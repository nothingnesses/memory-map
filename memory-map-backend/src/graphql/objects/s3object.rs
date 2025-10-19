use crate::graphql::ContextWrapper;
use async_graphql::{Context, Error as GraphQLError, ID, SimpleObject};
use tokio_postgres::{Error as TPError, Row};

#[derive(SimpleObject)]
pub struct S3Object {
	id: ID,
	name: String,
	time_stamp: String,
}

impl TryFrom<Row> for S3Object {
	type Error = TPError;

	fn try_from(value: Row) -> Result<Self, Self::Error> {
		Ok(S3Object {
			id: Row::try_get::<_, i64>(&value, "id")?.into(),
			name: value.try_get("name")?,
			time_stamp: value.try_get("time_stamp")?,
		})
	}
}

impl S3Object {
	pub async fn all(ctx: &Context<'_>) -> Result<Vec<Self>, GraphQLError> {
		let client = ContextWrapper(ctx).get_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT id, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude
				FROM locations",
			)
			.await?;
		Ok(client
			.query(&statement, &[])
			.await
			.map_err(|e| GraphQLError::from(e))?
			.into_iter()
			.map(Self::try_from)
			.collect::<Result<Vec<_>, _>>()?)
	}

	pub async fn where_id(
		ctx: &Context<'_>,
		id: i64,
	) -> Result<Self, GraphQLError> {
		let client = ContextWrapper(ctx).get_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT *
				FROM locations
				WHERE id = $1",
			)
			.await?;
		Ok(S3Object::try_from(client.query_one(&statement, &[&id]).await?)?)
	}
}
