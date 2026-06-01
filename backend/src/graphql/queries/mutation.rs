use {
	crate::{
		CasbinObject,
		CasbinUser,
		ContextWrapper,
		SharedState,
		UserId,
		constants::PASSWORD_RESET_RATE_LIMIT_SECONDS,
		db::queries::{
			ADMIN_UPDATE_USER_QUERY,
			DELETE_PASSWORD_RESET_TOKENS_BY_USER_QUERY,
			INSERT_PASSWORD_RESET_TOKEN_QUERY,
			INSERT_USER_QUERY,
			RECENT_PASSWORD_RESET_TOKEN_EXISTS_QUERY,
			SELECT_PASSWORD_RESET_TOKEN_QUERY,
			SELECT_USER_COUNT_BY_EMAIL_EXCLUDING_ID_QUERY,
			SELECT_USER_COUNT_BY_EMAIL_QUERY,
			SELECT_USER_PASSWORD_HASH_BY_ID_QUERY,
			SELECT_USER_WITH_PASSWORD_BY_EMAIL_QUERY,
			UPDATE_USER_EMAIL_QUERY,
			UPDATE_USER_PASSWORD_QUERY,
			UPDATE_USER_PUBLICITY_QUERY,
		},
		email::send_password_reset_email,
		errors::AppError,
		graphql::objects::{
			location::Location,
			s3_object::{
				PublicityOverride,
				S3Object,
			},
			user::{
				PublicityDefault,
				User,
			},
		},
		object_lifecycle::ObjectLifecycleService,
	},
	anyhow::Context as AnyhowContext,
	argon2::{
		Argon2,
		PasswordHash,
		PasswordHasher,
		PasswordVerifier,
		password_hash::{
			SaltString,
			rand_core::OsRng,
		},
	},
	async_graphql::{
		Context,
		Error as GraphQLError,
		ID,
		InputObject,
		Object,
	},
	axum_extra::extract::cookie::{
		Cookie,
		SameSite,
	},
	casbin::CoreApi,
	deadpool_postgres::{
		Client,
		Manager,
	},
	email_address::EmailAddress,
	rand::{
		RngExt,
		distr::Alphanumeric,
	},
	std::sync::{
		Arc,
		Mutex,
	},
	time::Duration,
	tracing,
};

#[derive(InputObject)]
pub struct UpdateS3ObjectInput {
	pub id: ID,
	pub name: String,
	pub made_on: Option<String>,
	pub location: Option<Location>,
	pub publicity: PublicityOverride,
	pub allowed_users: Option<Vec<String>>,
}

fn validate_password(password: &str) -> Result<(), AppError> {
	if password.len() < 8 {
		return Err(AppError::Validation(
			"Password must be at least 8 characters long".to_string(),
		));
	}
	Ok(())
}

/// Builds an auth cookie with attributes that must match between login and logout.
///
/// Browsers refuse to overwrite a cookie when the replacement uses different attributes,
/// so both the login (set token) and logout (expire token) paths must agree on
/// `Secure`, `HttpOnly`, `SameSite`, and `Path`.
fn auth_cookie(
	value: String,
	max_age: Option<Duration>,
	secure: bool,
) -> Cookie<'static> {
	let mut builder = Cookie::build(("auth_token", value))
		.http_only(true)
		.secure(secure)
		.same_site(SameSite::Lax)
		.path("/");
	if let Some(max_age) = max_age {
		builder = builder.max_age(max_age);
	}
	builder.build()
}

pub struct Mutation;

#[Object]
impl Mutation {
	async fn delete_s3_objects(
		&self,
		ctx: &Context<'_>,
		ids: Vec<ID>,
	) -> Result<Vec<S3Object>, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?.0;
		let wrapper = ContextWrapper(ctx);
		let storage = wrapper.get_storage_client()?;
		let mut client = wrapper.get_db_client().await?;
		let ids: Vec<i64> = ids
			.into_iter()
			.map(|id| id.parse::<i64>().context("Invalid ID format").map_err(GraphQLError::from))
			.collect::<Result<Vec<i64>, _>>()?;

		// Check permissions
		let state = ctx
			.data::<Arc<SharedState<Manager, Client>>>()
			.map_err(|e| anyhow::anyhow!(e.message).context("Shared state not found in context"))
			.map_err(GraphQLError::from)?;
		let enforcer = state.enforcer.read().await;
		let user =
			User::by_id(ctx, user_id).await?.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser {
			id: user_id,
			role: user.role.to_string(),
		};

		let objects = S3Object::where_ids(ctx, &ids).await?;
		for obj in &objects {
			let casbin_obj = CasbinObject {
				user_id: obj.user_id.unwrap_or(0),
			};
			enforcer
				.enforce((casbin_user.clone(), casbin_obj, "delete"))
				.map_err(AppError::from)?
				.then_some(())
				.ok_or_else(|| GraphQLError::new("Forbidden"))?;
		}

		let result = ObjectLifecycleService::new(
			&mut client,
			storage,
			state.config.object_lifecycle.clone(),
		)
		.delete_objects(&ids)
		.await
		.map_err(GraphQLError::from)?;

		state.update_last_modified();

		Ok(result)
	}

	/// GraphQL mutation to update an S3 object.
	/// It retrieves the database client, parses the ID, calls the worker function,
	/// and updates the last modified state.
	async fn update_s3_object(
		&self,
		ctx: &Context<'_>,
		input: UpdateS3ObjectInput,
	) -> Result<S3Object, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?.0;
		let wrapper = ContextWrapper(ctx);
		let storage = wrapper.get_storage_client()?;
		let mut client = wrapper.get_db_client().await?;
		let id_int =
			input.id.parse::<i64>().context("Invalid ID format").map_err(GraphQLError::from)?;

		// Check permissions
		let state = ctx
			.data::<Arc<SharedState<Manager, Client>>>()
			.map_err(|e| anyhow::anyhow!(e.message).context("Shared state not found in context"))
			.map_err(GraphQLError::from)?;
		let enforcer = state.enforcer.read().await;
		let user =
			User::by_id(ctx, user_id).await?.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser {
			id: user_id,
			role: user.role.to_string(),
		};

		let obj = S3Object::where_id(ctx, id_int).await?;
		let casbin_obj = CasbinObject {
			user_id: obj.user_id.unwrap_or(0),
		};
		if !enforcer.enforce((casbin_user, casbin_obj, "update")).map_err(GraphQLError::from)? {
			return Err(GraphQLError::new("Forbidden"));
		}

		let allowed_users = input.allowed_users.unwrap_or_default();

		let result = ObjectLifecycleService::new(
			&mut client,
			storage,
			state.config.object_lifecycle.clone(),
		)
		.update_object_metadata(
			id_int,
			input.name,
			input.made_on,
			input.location,
			input.publicity,
			allowed_users,
		)
		.await
		.map_err(GraphQLError::from)?;

		state.update_last_modified();

		Ok(result)
	}

	async fn update_user_publicity(
		&self,
		ctx: &Context<'_>,
		default_publicity: PublicityDefault,
	) -> Result<User, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?.0;
		let wrapper = ContextWrapper(ctx);
		let client = wrapper.get_db_client().await?;

		// Check permissions
		let state = ctx
			.data::<Arc<SharedState<Manager, Client>>>()
			.map_err(|e| anyhow::anyhow!(e.message).context("Shared state not found in context"))
			.map_err(GraphQLError::from)?;
		let enforcer = state.enforcer.read().await;
		let user =
			User::by_id(ctx, user_id).await?.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser {
			id: user_id,
			role: user.role.to_string(),
		};
		let casbin_obj = CasbinObject {
			user_id,
		};

		if !enforcer.enforce((casbin_user, casbin_obj, "update")).map_err(GraphQLError::from)? {
			return Err(GraphQLError::new("Forbidden"));
		}

		let row = client
			.query_one(UPDATE_USER_PUBLICITY_QUERY, &[&default_publicity, &user_id])
			.await
			.context("Failed to update user publicity in database")?;

		User::try_from(row)
			.map_err(|e| anyhow::anyhow!("Failed to convert database row to User: {}", e.message))
			.map_err(GraphQLError::from)
	}

	async fn register(
		&self,
		ctx: &Context<'_>,
		email: String,
		password: String,
	) -> Result<User, GraphQLError> {
		let wrapper = ContextWrapper(ctx);
		let client = wrapper.get_db_client().await?;
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;

		if !state.config.auth.enable_registration {
			return Err(GraphQLError::new("Registration is disabled"));
		}

		validate_password(&password).map_err(GraphQLError::from)?;

		if !EmailAddress::is_valid(&email) {
			return Err(GraphQLError::new("Invalid email format"));
		}

		// Check if email is taken
		let count: i64 = client
			.query_one(SELECT_USER_COUNT_BY_EMAIL_QUERY, &[&email])
			.await?
			.try_get(0)
			.context("Failed to get user count from database")?;

		if count > 0 {
			return Err(GraphQLError::new("Email already in use"));
		}

		let salt = SaltString::generate(&mut OsRng);
		let argon2 = Argon2::default();
		let password_hash = argon2
			.hash_password(password.as_bytes(), &salt)
			.map_err(|e| anyhow::anyhow!(e).context("Failed to hash password"))
			.map_err(GraphQLError::from)?
			.to_string();

		let statement = client.prepare_cached(INSERT_USER_QUERY).await?;

		let row = client
			.query_one(&statement, &[&email, &password_hash])
			.await
			.context("Failed to insert user into database")?;

		User::try_from(row)
			.map_err(|e| anyhow::anyhow!("Failed to convert database row to User: {}", e.message))
			.map_err(GraphQLError::from)
	}

	async fn login(
		&self,
		ctx: &Context<'_>,
		email: String,
		password: String,
	) -> Result<User, GraphQLError> {
		let wrapper = ContextWrapper(ctx);
		let client = wrapper.get_db_client().await?;

		let statement = client.prepare_cached(SELECT_USER_WITH_PASSWORD_BY_EMAIL_QUERY).await?;

		let row = client
			.query_opt(&statement, &[&email])
			.await
			.context("Failed to query user from database")?
			.ok_or_else(|| GraphQLError::new("Invalid email or password"))?;

		let password_hash_str: String = row
			.try_get("password_hash")
			.context("Failed to get password hash from database row")?;
		let user = User::try_from(row).map_err(|e| {
			anyhow::anyhow!("Failed to convert database row to User: {}", e.message)
		})?;

		let parsed_hash = PasswordHash::new(&password_hash_str)
			.map_err(|e| anyhow::anyhow!(e).context("Failed to parse password hash from database"))
			.map_err(GraphQLError::from)?;

		Argon2::default()
			.verify_password(password.as_bytes(), &parsed_hash)
			.map_err(|_| GraphQLError::new("Invalid email or password"))?;

		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;

		let cookie = auth_cookie(user.id.to_string(), None, state.config.cookie_secure());

		let cookies = ctx
			.data::<Arc<Mutex<Vec<Cookie<'static>>>>>()
			.map_err(|e| anyhow::anyhow!(e.message).context("Cookies not found in context"))
			.map_err(GraphQLError::from)?;
		cookies
			.lock()
			.map_err(|e| {
				tracing::error!("Mutex poisoned: {}", e);
				GraphQLError::new("Internal server error")
			})?
			.push(cookie);

		Ok(user)
	}

	async fn logout(
		&self,
		ctx: &Context<'_>,
	) -> Result<bool, GraphQLError> {
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let cookie =
			auth_cookie(String::new(), Some(Duration::seconds(0)), state.config.cookie_secure());

		let cookies = ctx
			.data::<Arc<Mutex<Vec<Cookie<'static>>>>>()
			.map_err(|e| anyhow::anyhow!(e.message).context("Cookies not found in context"))
			.map_err(GraphQLError::from)?;
		cookies
			.lock()
			.map_err(|e| {
				tracing::error!("Mutex poisoned: {}", e);
				GraphQLError::new("Internal server error")
			})?
			.push(cookie);

		Ok(true)
	}

	async fn change_password(
		&self,
		ctx: &Context<'_>,
		old_password: String,
		new_password: String,
	) -> Result<bool, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?;
		let wrapper = ContextWrapper(ctx);
		let client = wrapper.get_db_client().await?;

		// Check permissions
		let state = ctx
			.data::<Arc<SharedState<Manager, Client>>>()
			.map_err(|e| anyhow::anyhow!(e.message).context("Shared state not found in context"))
			.map_err(GraphQLError::from)?;
		let enforcer = state.enforcer.read().await;
		let user = User::by_id(ctx, user_id.0)
			.await?
			.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser {
			id: user_id.0,
			role: user.role.to_string(),
		};
		let casbin_obj = CasbinObject {
			user_id: user_id.0,
		};

		if !enforcer.enforce((casbin_user, casbin_obj, "update")).map_err(GraphQLError::from)? {
			return Err(GraphQLError::new("Forbidden"));
		}

		validate_password(&new_password).map_err(GraphQLError::from)?;

		let password_hash_str: String = client
			.query_one(SELECT_USER_PASSWORD_HASH_BY_ID_QUERY, &[&user_id.0])
			.await?
			.try_get("password_hash")
			.context("Failed to get password hash from database row")?;

		let parsed_hash = PasswordHash::new(&password_hash_str)
			.map_err(|e| anyhow::anyhow!(e).context("Failed to parse password hash from database"))
			.map_err(GraphQLError::from)?;

		Argon2::default()
			.verify_password(old_password.as_bytes(), &parsed_hash)
			.map_err(|_| GraphQLError::new("Invalid old password"))?;

		let salt = SaltString::generate(&mut OsRng);
		let new_hash = Argon2::default()
			.hash_password(new_password.as_bytes(), &salt)
			.map_err(|e| anyhow::anyhow!(e).context("Failed to hash new password"))
			.map_err(GraphQLError::from)?
			.to_string();

		client
			.execute(UPDATE_USER_PASSWORD_QUERY, &[&new_hash, &user_id.0])
			.await
			.context("Failed to update user password in database")?;

		Ok(true)
	}

	async fn change_email(
		&self,
		ctx: &Context<'_>,
		new_email: String,
	) -> Result<User, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?;
		let wrapper = ContextWrapper(ctx);
		let client = wrapper.get_db_client().await?;

		// Check permissions
		let state = ctx
			.data::<Arc<SharedState<Manager, Client>>>()
			.map_err(|e| anyhow::anyhow!(e.message).context("Shared state not found in context"))
			.map_err(GraphQLError::from)?;
		let enforcer = state.enforcer.read().await;
		let user = User::by_id(ctx, user_id.0)
			.await?
			.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser {
			id: user_id.0,
			role: user.role.to_string(),
		};
		let casbin_obj = CasbinObject {
			user_id: user_id.0,
		};

		if !enforcer.enforce((casbin_user, casbin_obj, "update")).map_err(GraphQLError::from)? {
			return Err(GraphQLError::new("Forbidden"));
		}

		if !EmailAddress::is_valid(&new_email) {
			return Err(GraphQLError::new("Invalid email format"));
		}

		// Check if email is taken
		let count: i64 = client
			.query_one(SELECT_USER_COUNT_BY_EMAIL_EXCLUDING_ID_QUERY, &[&new_email, &user_id.0])
			.await?
			.try_get(0)
			.context("Failed to get user count from database")?;

		if count > 0 {
			return Err(GraphQLError::new("Email already in use"));
		}

		let row = client
			.query_one(UPDATE_USER_EMAIL_QUERY, &[&new_email, &user_id.0])
			.await
			.context("Failed to update user email in database")?;

		User::try_from(row)
			.map_err(|e| anyhow::anyhow!("Failed to convert database row to User: {}", e.message))
			.map_err(GraphQLError::from)
	}

	async fn request_password_reset(
		&self,
		ctx: &Context<'_>,
		email: String,
	) -> Result<bool, GraphQLError> {
		let wrapper = ContextWrapper(ctx);
		let mut client = wrapper.get_db_client().await?;
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;

		let Some(user) = User::by_email(ctx, &email).await? else {
			// Always succeed to avoid user enumeration.
			return Ok(true);
		};

		let user_id_int = user
			.id
			.parse::<i64>()
			.context("Failed to parse user ID")
			.map_err(GraphQLError::from)?;

		// Rate limit: skip silently if a recent token already exists.
		// The throttle is per user, so an attacker cannot use timing to enumerate accounts.
		let recent: bool = client
			.query_one(
				RECENT_PASSWORD_RESET_TOKEN_EXISTS_QUERY,
				&[&user_id_int, &PASSWORD_RESET_RATE_LIMIT_SECONDS],
			)
			.await
			.context("Failed to check recent password reset tokens")?
			.try_get(0)
			.context("Failed to read recent-token existence")?;
		if recent {
			return Ok(true);
		}

		let token: String =
			rand::rng().sample_iter(Alphanumeric).take(32).map(char::from).collect();
		let token_hash = blake3::hash(token.as_bytes()).to_string();

		// Invalidate sibling tokens, then insert the new one, then send the email.
		// The transaction commits only after the email succeeds, so an SMTP failure
		// rolls back the new token and leaves the old sibling state intact.
		let transaction = client.transaction().await?;
		transaction
			.execute(DELETE_PASSWORD_RESET_TOKENS_BY_USER_QUERY, &[&user_id_int])
			.await
			.context("Failed to invalidate existing password reset tokens")?;
		transaction
			.execute(INSERT_PASSWORD_RESET_TOKEN_QUERY, &[&token_hash, &user_id_int])
			.await
			.context("Failed to insert password reset token into database")?;

		send_password_reset_email(&state.config, &email, &token)
			.await
			.context("Failed to send password reset email")
			.map_err(|e| AppError::from(e).extend_graphql())?;

		transaction.commit().await?;

		Ok(true)
	}

	async fn reset_password(
		&self,
		ctx: &Context<'_>,
		token: String,
		new_password: String,
	) -> Result<bool, GraphQLError> {
		let wrapper = ContextWrapper(ctx);
		let client = wrapper.get_db_client().await?;

		validate_password(&new_password).map_err(GraphQLError::from)?;

		let token_hash = blake3::hash(token.as_bytes()).to_string();

		let row_opt = client
			.query_opt(SELECT_PASSWORD_RESET_TOKEN_QUERY, &[&token_hash])
			.await
			.context("Failed to query password reset token from database")?;

		if let Some(row) = row_opt {
			let user_id: i64 =
				row.try_get("user_id").context("Failed to get user ID from database row")?;

			let salt = SaltString::generate(&mut OsRng);
			let new_hash = Argon2::default()
				.hash_password(new_password.as_bytes(), &salt)
				.map_err(|e| anyhow::anyhow!(e).context("Failed to hash new password"))
				.map_err(GraphQLError::from)?
				.to_string();

			client
				.execute(UPDATE_USER_PASSWORD_QUERY, &[&new_hash, &user_id])
				.await
				.context("Failed to update user password in database")?;

			// Invalidate all outstanding reset tokens for this user, not just the consumed one.
			// Prevents an attacker who triggered a second reset request from holding a working
			// bypass after the legitimate user changes their password.
			client
				.execute(DELETE_PASSWORD_RESET_TOKENS_BY_USER_QUERY, &[&user_id])
				.await
				.context("Failed to delete password reset tokens from database")?;

			Ok(true)
		} else {
			Err(GraphQLError::new("Invalid or expired token"))
		}
	}

	async fn admin_update_user(
		&self,
		ctx: &Context<'_>,
		id: ID,
		role: Option<String>,
		email: Option<String>,
	) -> Result<User, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?;
		let wrapper = ContextWrapper(ctx);
		let client = wrapper.get_db_client().await?;

		let target_id =
			id.parse::<i64>().context("Invalid ID format").map_err(GraphQLError::from)?;

		// Check permissions
		let state = ctx
			.data::<Arc<SharedState<Manager, Client>>>()
			.map_err(|e| anyhow::anyhow!(e.message).context("Shared state not found in context"))
			.map_err(GraphQLError::from)?;
		let enforcer = state.enforcer.read().await;
		let current_user = User::by_id(ctx, user_id.0)
			.await?
			.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser {
			id: user_id.0,
			role: current_user.role.to_string(),
		};
		let casbin_obj = CasbinObject {
			user_id: target_id,
		};

		if !enforcer
			.enforce((casbin_user, casbin_obj, "manage_user"))
			.map_err(GraphQLError::from)?
		{
			return Err(GraphQLError::new("Forbidden"));
		}

		let mut target_user = User::by_id(ctx, target_id)
			.await?
			.ok_or_else(|| GraphQLError::new("User not found"))?;

		if let Some(new_email) = email {
			if !EmailAddress::is_valid(&new_email) {
				return Err(GraphQLError::new("Invalid email format"));
			}

			// Check email uniqueness if changed
			let count: i64 = client
				.query_one(SELECT_USER_COUNT_BY_EMAIL_EXCLUDING_ID_QUERY, &[&new_email, &target_id])
				.await?
				.try_get(0)
				.context("Failed to get user count from database")?;

			if count > 0 {
				return Err(GraphQLError::new("Email already in use"));
			}
			target_user.email = new_email;
		}

		if let Some(new_role_str) = role {
			let new_role = new_role_str.parse().map_err(|_| GraphQLError::new("Invalid role"))?;
			target_user.role = new_role;
		}

		let row = client
			.query_one(
				ADMIN_UPDATE_USER_QUERY,
				&[&target_user.role.to_string(), &target_user.email, &target_id],
			)
			.await
			.context("Failed to update user in database")?;

		User::try_from(row)
			.map_err(|e| anyhow::anyhow!("Failed to convert database row to User: {}", e.message))
			.map_err(GraphQLError::from)
	}
}
