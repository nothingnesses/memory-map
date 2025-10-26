use crate::graphql::{
	ContextWrapper,
	objects::{RowContext, location::Location, s3object::S3Object},
};
use async_graphql::{Context, Error as GraphQLError, Object};
use jiff::Timestamp;

pub struct Mutation;

#[Object]
impl Mutation {
	async fn add_location(
		&self,
		ctx: &Context<'_>,
		longitude: f64,
		latitude: f64,
	) -> Result<Location, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"INSERT INTO locations (location)
				VALUES (ST_SetSRID(ST_MakePoint($1, $2), 4326))
				RETURNING id, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude;",
			)
			.await?;
		Ok(Location::try_from(client.query_one(&statement, &[&longitude, &latitude]).await?)?)
	}

	async fn add_s3_object(
		&self,
		ctx: &Context<'_>,
		name: String,
		made_on: Option<String>,
	) -> Result<S3Object, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let parsed_made_on: Option<Timestamp> = match made_on {
			Some(ts) => Some(ts.parse()?),
			None => None,
		};
		let statement = client
			.prepare_cached(
				"INSERT INTO objects (name, made_on)
				VALUES (1,2)
				RETURNING id, name, made_on;",
			)
			.await?;
		let row = client.query_one(&statement, &[&name, &parsed_made_on]).await?;
		S3Object::try_from(RowContext(row, ctx.clone())).await
	}
}
