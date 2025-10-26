use crate::graphql::{
	ContextWrapper,
	objects::{location::Location, s3object::S3Object},
};
use async_graphql::{Context, Error as GraphQLError, Object};

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
				RETURNING id, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude",
			)
			.await?;
		Ok(Location::try_from(client.query_one(&statement, &[&longitude, &latitude]).await?)?)
	}

	async fn add_s3_object(
		&self,
		ctx: &Context<'_>,
		name: String,
		time_stamp: Option<String>,
	) -> Result<S3Object, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		todo!()
	}
}
