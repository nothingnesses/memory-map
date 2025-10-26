use crate::graphql::ContextWrapper;
use async_graphql::{Context, Error as GraphQLError, ID, SimpleObject};
use tokio_postgres::{Error as TPError, Row};

#[derive(SimpleObject)]
pub struct Location {
	pub id: ID,
	pub latitude: f64,
	pub longitude: f64,
}

impl TryFrom<Row> for Location {
	type Error = TPError;

	fn try_from(value: Row) -> Result<Self, Self::Error> {
		Ok(Location {
			id: Row::try_get::<_, i64>(&value, "id")?.into(),
			latitude: value.try_get("latitude")?,
			longitude: value.try_get("longitude")?,
		})
	}
}

impl Location {
	pub async fn all(ctx: &Context<'_>) -> Result<Vec<Self>, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT id, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude
				FROM locations",
			)
			.await?;
		Ok(client
			.query(&statement, &[])
			.await
			.map_err(GraphQLError::from)?
			.into_iter()
			.map(Self::try_from)
			.collect::<Result<Vec<_>, _>>()?)
	}

	pub async fn where_id(
		ctx: &Context<'_>,
		id: i64,
	) -> Result<Self, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT id, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude
				FROM locations
				WHERE id = $1",
			)
			.await?;
		Ok(Self::try_from(client.query_one(&statement, &[&id]).await?)?)
	}
}
