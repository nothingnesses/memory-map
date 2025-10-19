use crate::graphql::objects::{location::Location, s3object::S3Object};
use async_graphql::{Context, Error as GraphQLError, Object};

pub struct Query;

#[Object]
impl Query {
	async fn location(
		&self,
		ctx: &Context<'_>,
		id: i64,
	) -> Result<Location, GraphQLError> {
		Location::where_id(ctx, id).await
	}

	async fn locations(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<Location>, GraphQLError> {
		Location::all(ctx).await
	}

	async fn object(
		&self,
		ctx: &Context<'_>,
		id: i64,
	) -> Result<S3Object, GraphQLError> {
		S3Object::where_id(ctx, id).await
	}

	async fn objects(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<S3Object>, GraphQLError> {
		S3Object::all(ctx).await
	}
}
