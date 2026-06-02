use {
	crate::{
		CasbinObject,
		ContextWrapper,
		errors::AppError,
		graphql::objects::{
			config::PublicConfig,
			s3_object::S3Object,
			user::User,
		},
	},
	async_graphql::{
		Context,
		Error as GraphQLError,
		Object,
	},
};

pub struct Query;

#[Object]
impl Query {
	async fn config(
		&self,
		ctx: &Context<'_>,
	) -> Result<PublicConfig, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let state = wrapper.shared_state();
		Ok(PublicConfig {
			enable_registration: state.config.auth.enable_registration,
		})
	}

	async fn me(
		&self,
		ctx: &Context<'_>,
	) -> Result<Option<User>, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		if let Some(user_id) = wrapper.user_id_opt() {
			User::by_id(ctx, user_id).await.map_err(AppError::graphql)
		} else {
			Ok(None)
		}
	}

	async fn users(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<User>, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		wrapper
			.require_permission(
				"read_all_users",
				CasbinObject {
					user_id: 0,
				},
			)
			.await?;
		User::all(ctx).await.map_err(AppError::graphql)
	}

	async fn s3_object_by_id(
		&self,
		ctx: &Context<'_>,
		id: i64,
	) -> Result<S3Object, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let object = S3Object::where_id(ctx, id).await.map_err(AppError::graphql)?;
		wrapper
			.require_permission(
				"read",
				CasbinObject {
					user_id: object.user_id.unwrap_or(0),
				},
			)
			.await?;
		Ok(object)
	}

	async fn s3_object_by_name(
		&self,
		ctx: &Context<'_>,
		name: String,
	) -> Result<S3Object, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let object = S3Object::where_name(ctx, name).await.map_err(AppError::graphql)?;
		wrapper
			.require_permission(
				"read",
				CasbinObject {
					user_id: object.user_id.unwrap_or(0),
				},
			)
			.await?;
		Ok(object)
	}

	async fn s3_objects(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<S3Object>, GraphQLError> {
		let wrapper = ContextWrapper::new(ctx)?;
		let user_id_opt = wrapper.user_id_opt();

		if wrapper.caller_identity_opt().is_some() &&
			wrapper
				.has_permission(
					"read_all_s3_objects",
					CasbinObject {
						user_id: 0,
					},
				)
				.await?
		{
			return S3Object::all(ctx).await.map_err(AppError::graphql);
		}

		S3Object::visible_to_user(ctx, user_id_opt).await.map_err(AppError::graphql)
	}
}
