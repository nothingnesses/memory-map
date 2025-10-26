use crate::graphql::objects::s3object::S3Object;
use async_graphql::{Context, Error as GraphQLError, Object};

pub struct Query;

#[Object]
impl Query {
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
