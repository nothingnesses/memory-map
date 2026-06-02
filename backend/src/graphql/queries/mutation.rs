use {
	crate::{
		CasbinObject,
		ContextWrapper,
		GraphqlMutationCacheEffect,
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
			SELECT_USER_ID_BY_EMAIL_FOR_UPDATE_QUERY,
			SELECT_USER_PASSWORD_HASH_BY_ID_QUERY,
			SELECT_USER_WITH_PASSWORD_BY_EMAIL_QUERY,
			UPDATE_USER_EMAIL_QUERY,
			UPDATE_USER_PASSWORD_QUERY,
			UPDATE_USER_PUBLICITY_QUERY,
		},
		email_worker::enqueue_password_reset_email,
		errors::AppError,
		graphql::objects::{
			location::Location,
			s3_object::{
				PublicityOverride,
				S3Object,
			},
			upload_session::{
				AbortedObjectUpload,
				CreatedObjectUploadSession,
				PresignedObjectUploadPart,
			},
			user::{
				PublicityDefault,
				User,
			},
		},
		object_lifecycle::{
			ObjectLifecycleService,
			ObjectUploadSessionCreate,
		},
		storage::CompletedUploadPart,
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
	email_address::EmailAddress,
	jiff::Timestamp,
	rand::{
		RngExt,
		distr::Alphanumeric,
	},
	std::sync::Arc,
	time::Duration,
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

#[derive(InputObject)]
pub struct CreateObjectUploadSessionInput {
	pub name: String,
	pub content_type: String,
	pub file_size_bytes: i64,
	pub made_on: Option<String>,
	pub location: Option<Location>,
	pub publicity: PublicityOverride,
	pub allowed_users: Option<Vec<String>>,
}

#[derive(InputObject)]
pub struct CompletedObjectUploadPartInput {
	pub part_number: i32,
	pub e_tag: String,
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

fn presigned_url_expires_at(config: &crate::Config) -> Result<Timestamp, GraphQLError> {
	Timestamp::now()
		.checked_add(std::time::Duration::from_secs(config.storage.presigned_url_ttl_seconds))
		.context("Failed to calculate presigned URL expiry")
		.map_err(AppError::graphql)
}

pub struct Mutation;

#[Object]
impl Mutation {
	async fn create_object_upload_session(
		&self,
		ctx: &Context<'_>,
		input: CreateObjectUploadSessionInput,
	) -> Result<CreatedObjectUploadSession, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let user_id = wrapper.user_id()?;
		let storage = wrapper.storage_client();
		let mut client = wrapper.db_client().await?;
		let state = wrapper.shared_state();

		let session = ObjectLifecycleService::new(
			&mut client,
			storage,
			state.config.object_lifecycle.clone(),
		)
		.create_upload_session(ObjectUploadSessionCreate {
			name: input.name,
			content_type: input.content_type,
			file_size_bytes: input.file_size_bytes,
			made_on: input.made_on,
			location: input.location,
			user_id,
			publicity: input.publicity,
			allowed_users: input.allowed_users.unwrap_or_default(),
		})
		.await
		.map_err(AppError::graphql)?;

		Ok(session.into())
	}

	async fn presign_object_upload_parts(
		&self,
		ctx: &Context<'_>,
		object_id: ID,
		part_numbers: Vec<i32>,
	) -> Result<Vec<PresignedObjectUploadPart>, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let user_id = wrapper.user_id()?;
		let object_id =
			object_id.parse::<i64>().context("Invalid ID format").map_err(AppError::graphql)?;
		let storage = wrapper.storage_client();
		let mut client = wrapper.db_client().await?;
		let state = wrapper.shared_state();
		let url_expires_at = presigned_url_expires_at(&state.config)?;

		let parts = ObjectLifecycleService::new(
			&mut client,
			storage,
			state.config.object_lifecycle.clone(),
		)
		.presign_upload_parts(object_id, user_id, part_numbers)
		.await
		.map_err(AppError::graphql)?;

		ctx.data::<Arc<GraphqlMutationCacheEffect>>()
			.map_err(|e| anyhow::anyhow!(e.message).context("Mutation cache effect not found"))
			.map_err(AppError::graphql)?
			.mark_non_invalidating_field();

		Ok(parts
			.into_iter()
			.map(|part| PresignedObjectUploadPart::new(part, url_expires_at))
			.collect())
	}

	async fn complete_object_upload(
		&self,
		ctx: &Context<'_>,
		object_id: ID,
		parts: Vec<CompletedObjectUploadPartInput>,
	) -> Result<S3Object, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let user_id = wrapper.user_id()?;
		let object_id =
			object_id.parse::<i64>().context("Invalid ID format").map_err(AppError::graphql)?;
		let storage = wrapper.storage_client();
		let mut client = wrapper.db_client().await?;
		let state = wrapper.shared_state();
		let completed_parts = parts
			.into_iter()
			.map(|part| CompletedUploadPart {
				part_number: part.part_number,
				e_tag: part.e_tag,
			})
			.collect();

		ObjectLifecycleService::new(&mut client, storage, state.config.object_lifecycle.clone())
			.complete_upload(object_id, user_id, completed_parts)
			.await
			.map_err(AppError::graphql)
	}

	async fn abort_object_upload(
		&self,
		ctx: &Context<'_>,
		object_id: ID,
	) -> Result<AbortedObjectUpload, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let user_id = wrapper.user_id()?;
		let object_id =
			object_id.parse::<i64>().context("Invalid ID format").map_err(AppError::graphql)?;
		let storage = wrapper.storage_client();
		let mut client = wrapper.db_client().await?;
		let state = wrapper.shared_state();

		ObjectLifecycleService::new(&mut client, storage, state.config.object_lifecycle.clone())
			.abort_upload(object_id, user_id)
			.await
			.map_err(AppError::graphql)?;

		Ok(AbortedObjectUpload::new(object_id))
	}

	async fn delete_s3_objects(
		&self,
		ctx: &Context<'_>,
		ids: Vec<ID>,
	) -> Result<Vec<S3Object>, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let storage = wrapper.storage_client();
		let mut client = wrapper.db_client().await?;
		let state = wrapper.shared_state();
		let ids: Vec<i64> = ids
			.into_iter()
			.map(|id| id.parse::<i64>().context("Invalid ID format").map_err(AppError::graphql))
			.collect::<Result<Vec<i64>, _>>()?;

		let objects = S3Object::where_ids(ctx, &ids).await?;
		wrapper
			.require_permission_on_each(
				"delete",
				objects.iter().map(|obj| CasbinObject {
					user_id: obj.user_id.unwrap_or(0),
				}),
			)
			.await?;

		let result = ObjectLifecycleService::new(
			&mut client,
			storage,
			state.config.object_lifecycle.clone(),
		)
		.delete_objects(&ids)
		.await
		.map_err(AppError::graphql)?;

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
		let wrapper = ContextWrapper::new(ctx)?;
		let storage = wrapper.storage_client();
		let mut client = wrapper.db_client().await?;
		let state = wrapper.shared_state();
		let id_int =
			input.id.parse::<i64>().context("Invalid ID format").map_err(AppError::graphql)?;

		let obj = S3Object::where_id(ctx, id_int).await?;
		wrapper
			.require_permission(
				"update",
				CasbinObject {
					user_id: obj.user_id.unwrap_or(0),
				},
			)
			.await?;

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
		.map_err(AppError::graphql)?;

		Ok(result)
	}

	async fn update_user_publicity(
		&self,
		ctx: &Context<'_>,
		default_publicity: PublicityDefault,
	) -> Result<User, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let user_id = wrapper.user_id()?;
		wrapper
			.require_permission(
				"update",
				CasbinObject {
					user_id,
				},
			)
			.await?;
		let client = wrapper.db_client().await?;

		let row = client
			.query_one(UPDATE_USER_PUBLICITY_QUERY, &[&default_publicity, &user_id])
			.await
			.context("Failed to update user publicity in database")?;

		User::try_from(row)
			.map_err(|e| anyhow::anyhow!("Failed to convert database row to User: {}", e.message))
			.map_err(AppError::graphql)
	}

	async fn register(
		&self,
		ctx: &Context<'_>,
		email: String,
		password: String,
	) -> Result<User, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let client = wrapper.db_client().await?;
		let state = wrapper.shared_state();

		if !state.config.auth.enable_registration {
			return Err(AppError::Forbidden.extend_graphql());
		}

		validate_password(&password).map_err(AppError::graphql)?;

		if !EmailAddress::is_valid(&email) {
			return Err(AppError::Validation("Invalid email format".to_string()).extend_graphql());
		}

		// Check if email is taken
		let count: i64 = client
			.query_one(SELECT_USER_COUNT_BY_EMAIL_QUERY, &[&email])
			.await?
			.try_get(0)
			.context("Failed to get user count from database")?;

		if count > 0 {
			return Err(AppError::Validation("Email already in use".to_string()).extend_graphql());
		}

		let salt = SaltString::generate(&mut OsRng);
		let argon2 = Argon2::default();
		let password_hash = argon2
			.hash_password(password.as_bytes(), &salt)
			.map_err(|e| anyhow::anyhow!(e).context("Failed to hash password"))
			.map_err(AppError::graphql)?
			.to_string();

		let statement = client.prepare_cached(INSERT_USER_QUERY).await?;

		let row = client
			.query_one(&statement, &[&email, &password_hash])
			.await
			.context("Failed to insert user into database")?;

		User::try_from(row)
			.map_err(|e| anyhow::anyhow!("Failed to convert database row to User: {}", e.message))
			.map_err(AppError::graphql)
	}

	async fn login(
		&self,
		ctx: &Context<'_>,
		email: String,
		password: String,
	) -> Result<User, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let client = wrapper.db_client().await?;

		let statement = client.prepare_cached(SELECT_USER_WITH_PASSWORD_BY_EMAIL_QUERY).await?;

		let row = client
			.query_opt(&statement, &[&email])
			.await
			.context("Failed to query user from database")?
			.ok_or_else(|| AppError::Unauthorized.extend_graphql())?;

		let password_hash_str: String = row
			.try_get("password_hash")
			.context("Failed to get password hash from database row")?;
		let user = User::try_from(row).map_err(|e| {
			anyhow::anyhow!("Failed to convert database row to User: {}", e.message)
		})?;

		let parsed_hash = PasswordHash::new(&password_hash_str)
			.map_err(|e| anyhow::anyhow!(e).context("Failed to parse password hash from database"))
			.map_err(AppError::graphql)?;

		Argon2::default()
			.verify_password(password.as_bytes(), &parsed_hash)
			.map_err(|_| AppError::Unauthorized.extend_graphql())?;

		let state = wrapper.shared_state();

		let cookie = auth_cookie(user.id.to_string(), None, state.config.cookie_secure());

		let cookies = ctx
			.data::<Arc<parking_lot::Mutex<Vec<Cookie<'static>>>>>()
			.map_err(|e| anyhow::anyhow!(e.message).context("Cookies not found in context"))
			.map_err(AppError::graphql)?;
		cookies.lock().push(cookie);

		Ok(user)
	}

	async fn logout(
		&self,
		ctx: &Context<'_>,
	) -> Result<bool, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let state = wrapper.shared_state();
		let cookie =
			auth_cookie(String::new(), Some(Duration::seconds(0)), state.config.cookie_secure());

		let cookies = ctx
			.data::<Arc<parking_lot::Mutex<Vec<Cookie<'static>>>>>()
			.map_err(|e| anyhow::anyhow!(e.message).context("Cookies not found in context"))
			.map_err(AppError::graphql)?;
		cookies.lock().push(cookie);

		Ok(true)
	}

	async fn change_password(
		&self,
		ctx: &Context<'_>,
		old_password: String,
		new_password: String,
	) -> Result<bool, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let user_id = wrapper.user_id()?;
		wrapper
			.require_permission(
				"update",
				CasbinObject {
					user_id,
				},
			)
			.await?;
		let client = wrapper.db_client().await?;

		validate_password(&new_password).map_err(AppError::graphql)?;

		let password_hash_str: String = client
			.query_one(SELECT_USER_PASSWORD_HASH_BY_ID_QUERY, &[&user_id])
			.await?
			.try_get("password_hash")
			.context("Failed to get password hash from database row")?;

		let parsed_hash = PasswordHash::new(&password_hash_str)
			.map_err(|e| anyhow::anyhow!(e).context("Failed to parse password hash from database"))
			.map_err(AppError::graphql)?;

		Argon2::default()
			.verify_password(old_password.as_bytes(), &parsed_hash)
			.map_err(|_| AppError::Unauthorized.extend_graphql())?;

		let salt = SaltString::generate(&mut OsRng);
		let new_hash = Argon2::default()
			.hash_password(new_password.as_bytes(), &salt)
			.map_err(|e| anyhow::anyhow!(e).context("Failed to hash new password"))
			.map_err(AppError::graphql)?
			.to_string();

		client
			.execute(UPDATE_USER_PASSWORD_QUERY, &[&new_hash, &user_id])
			.await
			.context("Failed to update user password in database")?;

		Ok(true)
	}

	async fn change_email(
		&self,
		ctx: &Context<'_>,
		new_email: String,
	) -> Result<User, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let user_id = wrapper.user_id()?;
		wrapper
			.require_permission(
				"update",
				CasbinObject {
					user_id,
				},
			)
			.await?;
		let client = wrapper.db_client().await?;

		if !EmailAddress::is_valid(&new_email) {
			return Err(AppError::Validation("Invalid email format".to_string()).extend_graphql());
		}

		// Check if email is taken
		let count: i64 = client
			.query_one(SELECT_USER_COUNT_BY_EMAIL_EXCLUDING_ID_QUERY, &[&new_email, &user_id])
			.await?
			.try_get(0)
			.context("Failed to get user count from database")?;

		if count > 0 {
			return Err(AppError::Validation("Email already in use".to_string()).extend_graphql());
		}

		let row = client
			.query_one(UPDATE_USER_EMAIL_QUERY, &[&new_email, &user_id])
			.await
			.context("Failed to update user email in database")?;

		User::try_from(row)
			.map_err(|e| anyhow::anyhow!("Failed to convert database row to User: {}", e.message))
			.map_err(AppError::graphql)
	}

	async fn request_password_reset(
		&self,
		ctx: &Context<'_>,
		email: String,
	) -> Result<bool, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let mut client = wrapper.db_client().await?;
		let transaction = client.transaction().await?;
		let user_id_int: Option<i64> = transaction
			.query_opt(SELECT_USER_ID_BY_EMAIL_FOR_UPDATE_QUERY, &[&email])
			.await
			.context("Failed to lock password reset target user")?
			.map(|row| row.try_get::<_, i64>("id"))
			.transpose()
			.context("Failed to read password reset target user id")?;
		let Some(user_id_int) = user_id_int else {
			// Always succeed to avoid user enumeration.
			transaction.commit().await?;
			return Ok(true);
		};

		// Rate limit: skip silently if a recent token already exists. This check is
		// inside the user-row lock so concurrent requests cannot enqueue duplicates.
		let recent: bool = transaction
			.query_one(
				RECENT_PASSWORD_RESET_TOKEN_EXISTS_QUERY,
				&[&user_id_int, &PASSWORD_RESET_RATE_LIMIT_SECONDS],
			)
			.await
			.context("Failed to check recent password reset tokens")?
			.try_get(0)
			.context("Failed to read recent-token existence")?;
		if recent {
			transaction.commit().await?;
			return Ok(true);
		}

		let token: String =
			rand::rng().sample_iter(Alphanumeric).take(32).map(char::from).collect();
		let token_hash = blake3::hash(token.as_bytes()).to_string();

		// Invalidate sibling tokens, insert the new token, and enqueue the email in
		// one DB transaction. SMTP happens later in the worker, outside request locks.
		transaction
			.execute(DELETE_PASSWORD_RESET_TOKENS_BY_USER_QUERY, &[&user_id_int])
			.await
			.context("Failed to invalidate existing password reset tokens")?;
		transaction
			.execute(INSERT_PASSWORD_RESET_TOKEN_QUERY, &[&token_hash, &user_id_int])
			.await
			.context("Failed to insert password reset token into database")?;
		enqueue_password_reset_email(&transaction, &email, &token)
			.await
			.map_err(AppError::graphql)?;

		transaction.commit().await?;

		Ok(true)
	}

	async fn reset_password(
		&self,
		ctx: &Context<'_>,
		token: String,
		new_password: String,
	) -> Result<bool, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let client = wrapper.db_client().await?;

		validate_password(&new_password).map_err(AppError::graphql)?;

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
				.map_err(AppError::graphql)?
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
			Err(AppError::Validation("Invalid or expired token".to_string()).extend_graphql())
		}
	}

	async fn admin_update_user(
		&self,
		ctx: &Context<'_>,
		id: ID,
		role: Option<String>,
		email: Option<String>,
	) -> Result<User, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let client = wrapper.db_client().await?;

		let target_id =
			id.parse::<i64>().context("Invalid ID format").map_err(AppError::graphql)?;

		wrapper
			.require_permission(
				"manage_user",
				CasbinObject {
					user_id: target_id,
				},
			)
			.await?;

		let mut target_user = User::by_id(ctx, target_id)
			.await?
			.ok_or_else(|| AppError::NotFound("User not found".to_string()).extend_graphql())?;

		if let Some(new_email) = email {
			if !EmailAddress::is_valid(&new_email) {
				return Err(
					AppError::Validation("Invalid email format".to_string()).extend_graphql()
				);
			}

			// Check email uniqueness if changed
			let count: i64 = client
				.query_one(SELECT_USER_COUNT_BY_EMAIL_EXCLUDING_ID_QUERY, &[&new_email, &target_id])
				.await?
				.try_get(0)
				.context("Failed to get user count from database")?;

			if count > 0 {
				return Err(
					AppError::Validation("Email already in use".to_string()).extend_graphql()
				);
			}
			target_user.email = new_email;
		}

		if let Some(new_role_str) = role {
			let new_role = new_role_str
				.parse()
				.map_err(|_| AppError::Validation("Invalid role".to_string()).extend_graphql())?;
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
			.map_err(AppError::graphql)
	}
}
