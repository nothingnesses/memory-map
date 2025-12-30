use crate::{
	SharedState, UserId,
	graphql::objects::{
		config::PublicConfig,
		s3_object::S3Object,
		user::{User, UserRole},
	},
};
use async_graphql::{Context, Error as GraphQLError, Object};
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
		// Check if user is admin
		if let Some(user_id) = ctx.data_opt::<UserId>()
			&& let Some(user) = User::by_id(ctx, user_id.0).await?
			&& user.role == UserRole::Admin
		{
			return User::all(ctx).await;
		}
		Err(GraphQLError::new("Unauthorized"))
	}

	async fn s3_object_by_id(
		&self,
		ctx: &Context<'_>,
		id: i64,
	) -> Result<S3Object, GraphQLError> {
		S3Object::where_id(ctx, id).await
	}

	async fn s3_object_by_name(
		&self,
		ctx: &Context<'_>,
		name: String,
	) -> Result<S3Object, GraphQLError> {
		S3Object::where_name(ctx, name).await
	}

	async fn s3_objects(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<S3Object>, GraphQLError> {
		S3Object::all(ctx).await
	}
}
