use crate::{
	CasbinObject, CasbinUser, ContextWrapper, SharedState, UserId,
	email::send_password_reset_email,
	graphql::objects::{
		location::Location,
		s3_object::{PublicityOverride, S3Object},
		user::{PublicityDefault, User},
	},
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use async_graphql::{Context, Error as GraphQLError, ID, Object};
use axum_extra::extract::cookie::{Cookie, SameSite};
use casbin::CoreApi;
use deadpool_postgres::{Client, Manager};
use email_address::EmailAddress;
use futures::future::join_all;
use jiff::Timestamp;
use minio::s3::{Client as MinioClient, builders::ObjectToDelete, types::S3Api};
use rand::{Rng, distributions::Alphanumeric, rngs::OsRng};
use std::sync::{Arc, Mutex};
use time::Duration;
use tracing;

const DELETE_OBJECTS_QUERY: &str = "DELETE FROM objects WHERE id = ANY($1) RETURNING id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude, user_id, publicity;";

/// Query to update an existing object in the database.
/// It updates the name, made_on timestamp, and location based on the provided ID.
const UPDATE_OBJECT_QUERY: &str = "UPDATE objects
SET name = $2, made_on = $3::timestamptz, location = ST_GeomFromEWKT($4), publicity = $5
WHERE id = $1
RETURNING id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude, user_id, publicity;";

const UPSERT_OBJECT_QUERY: &str = "INSERT INTO objects (name, made_on, location, user_id, publicity)
VALUES ($1, $2::timestamptz, ST_GeomFromEWKT($3), $4, $5)
ON CONFLICT (name) DO UPDATE
SET made_on = EXCLUDED.made_on, location = EXCLUDED.location, publicity = EXCLUDED.publicity
RETURNING id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude, user_id, publicity;";

fn validate_password(password: &str) -> Result<(), GraphQLError> {
	if password.len() < 8 {
		return Err(GraphQLError::new("Password must be at least 8 characters long"));
	}
	Ok(())
}

pub struct Mutation;

impl Mutation {
	pub async fn delete_s3_objects_worker(
		db_client: &Client,
		minio_client: &MinioClient,
		bucket_name: &str,
		ids: &[i64],
	) -> Result<Vec<S3Object>, GraphQLError> {
		tracing::debug!("IDs to delete: {:?}", ids);
		let statement = db_client.prepare_cached(DELETE_OBJECTS_QUERY).await.map_err(|e| {
			tracing::error!("Failed to prepare query: {}", e);
			GraphQLError::new(format!("Database error: {e}"))
		})?;
		tracing::debug!("Delete DB query: {:?}", statement);

		let rows = db_client.query(&statement, &[&ids]).await.map_err(|e| {
			tracing::error!("Database query failed: {}", e);
			GraphQLError::new(format!("Database error: {e}"))
		})?;

		let objects = join_all(rows.into_iter().map(S3Object::try_from))
			.await
			.into_iter()
			.collect::<Result<Vec<_>, _>>()?;

		let objects_to_delete: Vec<ObjectToDelete> =
			objects.iter().map(|object| ObjectToDelete::from(&object.name)).collect();

		tracing::debug!("Objects to delete: {:?}", objects_to_delete);

		if !objects_to_delete.is_empty() {
			minio_client
				.delete_objects::<_, ObjectToDelete>(bucket_name, objects_to_delete)
				.send()
				.await
				.map_err(|e| {
					tracing::error!("Failed to delete objects from MinIO: {}", e);
					GraphQLError::new(format!("MinIO error: {e}"))
				})?;
		}

		Ok(objects)
	}

	/// Worker function to execute the update S3 object query.
	/// It parses the timestamp, formats the location geometry, and executes the SQL query.
	pub async fn update_s3_object_worker(
		client: &Client,
		id: i64,
		name: String,
		made_on: Option<String>,
		location: Option<Location>,
		publicity: PublicityOverride,
		allowed_users: Vec<String>,
	) -> Result<S3Object, GraphQLError> {
		let parsed_made_on: Option<Timestamp> = match made_on {
			Some(timestamp_string) => match timestamp_string.parse() {
				Ok(timestamp_string) => Some(timestamp_string),
				Err(error) => {
					tracing::error!("Failed to parse timestamp '{}': {}", timestamp_string, error);
					return Err(GraphQLError::new(format!(
						"Invalid timestamp format: {timestamp_string}"
					)));
				}
			},
			None => None,
		};
		let location_geometry = location.map(|location| {
			let location_geometry =
				format!("SRID=4326;POINT({} {})", location.longitude, location.latitude);
			tracing::debug!("Formatted location geometry: {}", location_geometry);
			location_geometry
		});
		tracing::debug!(
			"Executing update with: id={}, name={}, made_on={:?}, location={:?}, publicity={:?}",
			id,
			name,
			parsed_made_on.as_ref().map(|ts| ts.to_string()),
			location_geometry,
			publicity
		);
		let mut s3_object = S3Object::try_from(
			client
				.query_one(
					&client.prepare_cached(UPDATE_OBJECT_QUERY).await?,
					&[&id, &name, &parsed_made_on, &location_geometry, &publicity.to_string()],
				)
				.await
				.map_err(|e| {
					tracing::error!("Database query failed: {}", e);
					GraphQLError::new(format!("Database error: {e}"))
				})?,
		)
		.await?;

		// Update allowed users
		client
			.execute("DELETE FROM object_allowed_users WHERE object_id = $1", &[&id])
			.await
			.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;

		if !allowed_users.is_empty() {
			let rows = client
				.query("SELECT id FROM users WHERE email = ANY($1)", &[&allowed_users])
				.await
				.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;

			for row in rows {
				let user_id: i64 = row.get(0);
				client
					.execute(
						"INSERT INTO object_allowed_users (object_id, user_id) VALUES ($1, $2)",
						&[&id, &user_id],
					)
					.await
					.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;
			}
		}
		s3_object.allowed_users = allowed_users;
		Ok(s3_object)
	}

	pub async fn upsert_s3_object_worker(
		client: &Client,
		name: String,
		made_on: Option<String>,
		location: Option<Location>,
		user_id: i64,
		publicity: PublicityOverride,
		allowed_users: Vec<String>,
	) -> Result<S3Object, GraphQLError> {
		let parsed_made_on: Option<Timestamp> = match made_on {
			Some(timestamp_string) => match timestamp_string.parse() {
				Ok(timestamp_string) => Some(timestamp_string),
				Err(error) => {
					tracing::error!("Failed to parse timestamp '{}': {}", timestamp_string, error);
					return Err(GraphQLError::new(format!(
						"Invalid timestamp format: {timestamp_string}"
					)));
				}
			},
			None => None,
		};
		let location_geometry = location.map(|location| {
			let location_geometry =
				format!("SRID=4326;POINT({} {})", location.longitude, location.latitude);
			tracing::debug!("Formatted location geometry: {}", location_geometry);
			location_geometry
		});
		tracing::debug!(
			"Executing upsert with: name={}, made_on={:?}, location={:?}, user_id={}, publicity={:?}",
			name,
			parsed_made_on.as_ref().map(|ts| ts.to_string()),
			location_geometry,
			user_id,
			publicity
		);
		let mut s3_object = S3Object::try_from(
			client
				.query_one(
					&client.prepare_cached(UPSERT_OBJECT_QUERY).await?,
					&[&name, &parsed_made_on, &location_geometry, &user_id, &publicity.to_string()],
				)
				.await
				.map_err(|e| {
					tracing::error!("Database query failed: {}", e);
					GraphQLError::new(format!("Database error: {e}"))
				})?,
		)
		.await?;

		let id: i64 = s3_object.id.parse().unwrap();

		// Update allowed users
		client
			.execute("DELETE FROM object_allowed_users WHERE object_id = $1", &[&id])
			.await
			.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;

		if !allowed_users.is_empty() {
			let rows = client
				.query("SELECT id FROM users WHERE email = ANY($1)", &[&allowed_users])
				.await
				.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;

			for row in rows {
				let user_id: i64 = row.get(0);
				client
					.execute(
						"INSERT INTO object_allowed_users (object_id, user_id) VALUES ($1, $2)",
						&[&id, &user_id],
					)
					.await
					.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;
			}
		}
		s3_object.allowed_users = allowed_users;
		Ok(s3_object)
	}
}

#[Object]
impl Mutation {
	async fn delete_s3_objects(
		&self,
		ctx: &Context<'_>,
		ids: Vec<ID>,
	) -> Result<Vec<S3Object>, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?.0;
		let wrapper = ContextWrapper(ctx);
		let bucket_name = wrapper.get_bucket_name()?;
		let minio_client = wrapper.get_minio_client()?;
		let client = wrapper.get_db_client().await?;
		let ids: Vec<i64> = ids
			.into_iter()
			.map(|id| {
				id.parse::<i64>().map_err(|e| GraphQLError::new(format!("Invalid ID format: {e}")))
			})
			.collect::<Result<Vec<i64>, _>>()?;

		// Check permissions
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let enforcer = state.enforcer.read().await;
		let user =
			User::by_id(ctx, user_id).await?.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser { id: user_id, role: user.role.to_string() };

		let objects = S3Object::where_ids(ctx, &ids).await?;
		for obj in &objects {
			let casbin_obj = CasbinObject { user_id: obj.user_id.unwrap_or(0) };
			if !enforcer.enforce((casbin_user.clone(), casbin_obj, "delete"))? {
				return Err(GraphQLError::new("Forbidden"));
			}
		}

		let result =
			Self::delete_s3_objects_worker(&client, minio_client, bucket_name, &ids).await?;

		state.update_last_modified();

		Ok(result)
	}

	/// GraphQL mutation to update an S3 object.
	/// It retrieves the database client, parses the ID, calls the worker function,
	/// and updates the last modified state.
	async fn update_s3_object(
		&self,
		ctx: &Context<'_>,
		id: ID,
		name: String,
		made_on: Option<String>,
		location: Option<Location>,
		publicity: PublicityOverride,
		allowed_users: Option<Vec<String>>,
	) -> Result<S3Object, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?.0;
		let client = ContextWrapper(ctx).get_db_client().await?;
		let id_int =
			id.parse::<i64>().map_err(|e| GraphQLError::new(format!("Invalid ID format: {e}")))?;

		// Check permissions
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let enforcer = state.enforcer.read().await;
		let user =
			User::by_id(ctx, user_id).await?.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser { id: user_id, role: user.role.to_string() };

		let obj = S3Object::where_id(ctx, id_int).await?;
		let casbin_obj = CasbinObject { user_id: obj.user_id.unwrap_or(0) };
		if !enforcer.enforce((casbin_user, casbin_obj, "update"))? {
			return Err(GraphQLError::new("Forbidden"));
		}

		let allowed_users = allowed_users.unwrap_or_default();

		let result = Self::update_s3_object_worker(
			&client,
			id_int,
			name,
			made_on,
			location,
			publicity,
			allowed_users,
		)
		.await?;

		state.update_last_modified();

		Ok(result)
	}

	async fn upsert_s3_object(
		&self,
		ctx: &Context<'_>,
		name: String,
		made_on: Option<String>,
		location: Option<Location>,
		publicity: PublicityOverride,
		allowed_users: Option<Vec<String>>,
	) -> Result<S3Object, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?.0;
		let client = ContextWrapper(ctx).get_db_client().await?;

		// Check permissions
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let enforcer = state.enforcer.read().await;
		let user =
			User::by_id(ctx, user_id).await?.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser { id: user_id, role: user.role.to_string() };

		if let Ok(obj) = S3Object::where_name(ctx, name.clone()).await {
			let casbin_obj = CasbinObject { user_id: obj.user_id.unwrap_or(0) };
			if !enforcer.enforce((casbin_user.clone(), casbin_obj, "update"))? {
				return Err(GraphQLError::new("Forbidden"));
			}
		} else {
			let casbin_obj = CasbinObject { user_id };
			if !enforcer.enforce((casbin_user, casbin_obj, "create"))? {
				return Err(GraphQLError::new("Forbidden"));
			}
		}

		let allowed_users = allowed_users.unwrap_or_default();

		let result = Self::upsert_s3_object_worker(
			&client,
			name,
			made_on,
			location,
			user_id,
			publicity,
			allowed_users,
		)
		.await?;

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
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let enforcer = state.enforcer.read().await;
		let user =
			User::by_id(ctx, user_id).await?.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser { id: user_id, role: user.role.to_string() };
		let casbin_obj = CasbinObject { user_id };

		if !enforcer.enforce((casbin_user, casbin_obj, "update"))? {
			return Err(GraphQLError::new("Forbidden"));
		}

		let row = client
			.query_one(
				"UPDATE users SET default_publicity = $1, updated_at = now() WHERE id = $2 RETURNING *",
				&[&default_publicity.to_string(), &user_id],
			)
			.await
			.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;

		User::try_from(row)
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

		if !state.config.enable_registration {
			return Err(GraphQLError::new("Registration is disabled"));
		}

		validate_password(&password)?;

		if !EmailAddress::is_valid(&email) {
			return Err(GraphQLError::new("Invalid email format"));
		}

		// Check if email is taken
		let count: i64 = client
			.query_one("SELECT COUNT(*) FROM users WHERE email = $1", &[&email])
			.await
			.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?
			.get(0);

		if count > 0 {
			return Err(GraphQLError::new("Email already in use"));
		}

		let salt = SaltString::generate(&mut OsRng);
		let argon2 = Argon2::default();
		let password_hash = argon2
			.hash_password(password.as_bytes(), &salt)
			.map_err(|e| GraphQLError::new(format!("Hashing error: {e}")))?
			.to_string();

		let statement = client
			.prepare_cached("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING *")
			.await?;

		let row = client
			.query_one(&statement, &[&email, &password_hash])
			.await
			.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;

		User::try_from(row)
	}

	async fn login(
		&self,
		ctx: &Context<'_>,
		email: String,
		password: String,
	) -> Result<User, GraphQLError> {
		let wrapper = ContextWrapper(ctx);
		let client = wrapper.get_db_client().await?;

		let statement = client.prepare_cached("SELECT * FROM users WHERE email = $1").await?;

		let row = client
			.query_opt(&statement, &[&email])
			.await
			.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?
			.ok_or_else(|| GraphQLError::new("Invalid email or password"))?;

		let user = User::try_from(row)?;
		let password_hash_str: String = client
			.query_one(
				"SELECT password_hash FROM users WHERE id = $1",
				&[&user.id.parse::<i64>().unwrap()],
			)
			.await?
			.get("password_hash");

		let parsed_hash = PasswordHash::new(&password_hash_str)
			.map_err(|e| GraphQLError::new(format!("Hash parse error: {e}")))?;

		Argon2::default()
			.verify_password(password.as_bytes(), &parsed_hash)
			.map_err(|_| GraphQLError::new("Invalid email or password"))?;

		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let secure_cookie = state.config.frontend_url.starts_with("https");

		// Set cookie
		let cookie = Cookie::build(("auth_token", user.id.to_string()))
			.http_only(true)
			.secure(secure_cookie)
			.same_site(SameSite::Lax)
			.path("/")
			.build();

		let cookies = ctx.data::<Arc<Mutex<Vec<Cookie<'static>>>>>()?;
		cookies.lock().unwrap().push(cookie);

		Ok(user)
	}

	async fn logout(
		&self,
		ctx: &Context<'_>,
	) -> Result<bool, GraphQLError> {
		let cookie = Cookie::build(("auth_token", ""))
			.http_only(true)
			.secure(true)
			.same_site(SameSite::Lax)
			.path("/")
			.max_age(Duration::seconds(0))
			.build();

		let cookies = ctx.data::<Arc<Mutex<Vec<Cookie<'static>>>>>()?;
		cookies.lock().unwrap().push(cookie);

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
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let enforcer = state.enforcer.read().await;
		let user = User::by_id(ctx, user_id.0)
			.await?
			.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser { id: user_id.0, role: user.role.to_string() };
		let casbin_obj = CasbinObject { user_id: user_id.0 };

		if !enforcer.enforce((casbin_user, casbin_obj, "update"))? {
			return Err(GraphQLError::new("Forbidden"));
		}

		validate_password(&new_password)?;

		let password_hash_str: String = client
			.query_one("SELECT password_hash FROM users WHERE id = $1", &[&user_id.0])
			.await
			.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?
			.get("password_hash");

		let parsed_hash = PasswordHash::new(&password_hash_str)
			.map_err(|e| GraphQLError::new(format!("Hash parse error: {e}")))?;

		Argon2::default()
			.verify_password(old_password.as_bytes(), &parsed_hash)
			.map_err(|_| GraphQLError::new("Invalid old password"))?;

		let salt = SaltString::generate(&mut OsRng);
		let new_hash = Argon2::default()
			.hash_password(new_password.as_bytes(), &salt)
			.map_err(|e| GraphQLError::new(format!("Hashing error: {e}")))?
			.to_string();

		client
			.execute(
				"UPDATE users SET password_hash = $1, updated_at = now() WHERE id = $2",
				&[&new_hash, &user_id.0],
			)
			.await
			.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;

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
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let enforcer = state.enforcer.read().await;
		let user = User::by_id(ctx, user_id.0)
			.await?
			.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser { id: user_id.0, role: user.role.to_string() };
		let casbin_obj = CasbinObject { user_id: user_id.0 };

		if !enforcer.enforce((casbin_user, casbin_obj, "update"))? {
			return Err(GraphQLError::new("Forbidden"));
		}

		if !EmailAddress::is_valid(&new_email) {
			return Err(GraphQLError::new("Invalid email format"));
		}

		// Check if email is taken
		let count: i64 = client
			.query_one(
				"SELECT COUNT(*) FROM users WHERE email = $1 AND id != $2",
				&[&new_email, &user_id.0],
			)
			.await
			.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?
			.get(0);

		if count > 0 {
			return Err(GraphQLError::new("Email already in use"));
		}

		let row = client
			.query_one(
				"UPDATE users SET email = $1, updated_at = now() WHERE id = $2 RETURNING *",
				&[&new_email, &user_id.0],
			)
			.await
			.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;

		User::try_from(row)
	}

	async fn request_password_reset(
		&self,
		ctx: &Context<'_>,
		email: String,
	) -> Result<bool, GraphQLError> {
		let wrapper = ContextWrapper(ctx);
		let client = wrapper.get_db_client().await?;
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;

		let user_opt = User::by_email(ctx, &email).await?;
		if let Some(user) = user_opt {
			let token: String =
				rand::thread_rng().sample_iter(&Alphanumeric).take(32).map(char::from).collect();

			let token_hash = blake3::hash(token.as_bytes()).to_string();

			// Expires in 10 minutes
			client
				.execute(
					"INSERT INTO password_reset_tokens (token, user_id, expires_at) VALUES ($1, $2, now() + interval '10 minutes')",
					&[&token_hash, &user.id.parse::<i64>().unwrap()],
				)
				.await
				.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;

			// Send email
			// We spawn this to not block the request, but we need to handle errors.
			// For now, let's await it.
			send_password_reset_email(&state.config, &email, &token).await.map_err(|e| {
				tracing::error!("Failed to send email: {}", e);
				GraphQLError::new("Failed to send email")
			})?;
		}

		// Always return true to prevent user enumeration
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

		validate_password(&new_password)?;

		let token_hash = blake3::hash(token.as_bytes()).to_string();

		let row_opt = client
			.query_opt(
				"SELECT user_id FROM password_reset_tokens WHERE token = $1 AND expires_at > now()",
				&[&token_hash],
			)
			.await
			.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;

		if let Some(row) = row_opt {
			let user_id: i64 = row.get("user_id");

			let salt = SaltString::generate(&mut OsRng);
			let new_hash = Argon2::default()
				.hash_password(new_password.as_bytes(), &salt)
				.map_err(|e| GraphQLError::new(format!("Hashing error: {e}")))?
				.to_string();

			client
				.execute(
					"UPDATE users SET password_hash = $1, updated_at = now() WHERE id = $2",
					&[&new_hash, &user_id],
				)
				.await
				.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;

			// Delete token
			client
				.execute("DELETE FROM password_reset_tokens WHERE token = $1", &[&token_hash])
				.await?;

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

		let target_id = id.parse::<i64>().map_err(|_| GraphQLError::new("Invalid ID"))?;

		// Check permissions
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let enforcer = state.enforcer.read().await;
		let current_user = User::by_id(ctx, user_id.0)
			.await?
			.ok_or_else(|| GraphQLError::new("User not found"))?;
		let casbin_user = CasbinUser { id: user_id.0, role: current_user.role.to_string() };
		let casbin_obj = CasbinObject { user_id: target_id };

		if !enforcer.enforce((casbin_user, casbin_obj, "manage_user"))? {
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
				.query_one(
					"SELECT COUNT(*) FROM users WHERE email = $1 AND id != $2",
					&[&new_email, &target_id],
				)
				.await
				.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?
				.get(0);

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
				"UPDATE users SET role = $1, email = $2, updated_at = now() WHERE id = $3 RETURNING *",
				&[&target_user.role.to_string(), &target_user.email, &target_id],
			)
			.await
			.map_err(|e| GraphQLError::new(format!("Database error: {e}")))?;

		User::try_from(row)
	}
}
