use crate::{
	CasbinObject, CasbinUser, SharedState, UserId,
	graphql::objects::{
		config::PublicConfig,
		s3_object::S3Object,
		user::{User, UserRole},
	},
};
use async_graphql::{Context, Error as GraphQLError, Object};
use casbin::CoreApi;
use deadpool_postgres::{Client, Manager};
use std::sync::Arc;

pub struct Query;

#[Object]
impl Query {
	async fn config(
		&self,
		ctx: &Context<'_>,
	) -> Result<PublicConfig, GraphQLError> {
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		Ok(PublicConfig { enable_registration: state.config.enable_registration })
	}

	async fn me(
		&self,
		ctx: &Context<'_>,
	) -> Result<Option<User>, GraphQLError> {
		if let Some(user_id) = ctx.data_opt::<UserId>() {
			User::by_id(ctx, user_id.0).await
		} else {
			Ok(None)
		}
	}

	async fn users(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<User>, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?.0;

		// Check permissions
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let enforcer = state.enforcer.read().await;
		let user =
			User::by_id(ctx, user_id).await?.ok_or_else(|| GraphQLError::new("User not found"))?;

		let casbin_user = CasbinUser { id: user_id, role: user.role.to_string() };
		// Dummy object for system-level permission
		let casbin_obj = CasbinObject { user_id: 0 };

		if !enforcer.enforce((casbin_user, casbin_obj, "read_all_users"))? {
			return Err(GraphQLError::new("Forbidden"));
		}

		User::all(ctx).await
	}

	async fn s3_object_by_id(
		&self,
		ctx: &Context<'_>,
		id: i64,
	) -> Result<S3Object, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?.0;
		let user =
			User::by_id(ctx, user_id).await?.ok_or_else(|| GraphQLError::new("User not found"))?;
		let object = S3Object::where_id(ctx, id).await?;

		// Check permissions
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let enforcer = state.enforcer.read().await;
		let casbin_user = CasbinUser { id: user_id, role: user.role.to_string() };
		let casbin_obj = CasbinObject { user_id: object.user_id.unwrap_or(0) };

		if !enforcer.enforce((casbin_user, casbin_obj, "read"))? {
			return Err(GraphQLError::new("Forbidden"));
		}

		Ok(object)
	}

	async fn s3_object_by_name(
		&self,
		ctx: &Context<'_>,
		name: String,
	) -> Result<S3Object, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?.0;
		let user =
			User::by_id(ctx, user_id).await?.ok_or_else(|| GraphQLError::new("User not found"))?;
		let object = S3Object::where_name(ctx, name).await?;

		// Check permissions
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let enforcer = state.enforcer.read().await;
		let casbin_user = CasbinUser { id: user_id, role: user.role.to_string() };
		let casbin_obj = CasbinObject { user_id: object.user_id.unwrap_or(0) };

		if !enforcer.enforce((casbin_user, casbin_obj, "read"))? {
			return Err(GraphQLError::new("Forbidden"));
		}

		Ok(object)
	}

	async fn s3_objects(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<S3Object>, GraphQLError> {
		let user_id = ctx.data_opt::<UserId>().ok_or_else(|| GraphQLError::new("Unauthorized"))?.0;
		let user =
			User::by_id(ctx, user_id).await?.ok_or_else(|| GraphQLError::new("User not found"))?;

		// Check permissions
		let state = ctx.data::<Arc<SharedState<Manager, Client>>>()?;
		let enforcer = state.enforcer.read().await;
		let casbin_user = CasbinUser { id: user_id, role: user.role.to_string() };
		let casbin_obj = CasbinObject { user_id: 0 }; // System level object

		if enforcer.enforce((casbin_user, casbin_obj, "read_all_s3_objects"))? {
			S3Object::all(ctx).await
		} else {
			S3Object::where_user_id(ctx, user_id).await
		}
	}
}
