use crate::graphql::{
	ContextWrapper,
	objects::{RowContext, location::Location, s3object::S3Object},
};
use async_graphql::{Context, Error as GraphQLError, Object};
use jiff::Timestamp;

const UPSERT_OBJECT_QUERY: &str = "INSERT INTO objects (name, made_on, location)
VALUES ($1, $2, $3::geometry)
ON CONFLICT (name) DO UPDATE
SET made_on = EXCLUDED.made_on, location = EXCLUDED.location
RETURNING id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude;";

pub struct Mutation;

#[Object]
impl Mutation {
	async fn merge_s3_object(
		&self,
		ctx: &Context<'_>,
		name: String,
		made_on: Option<String>,
		location: Option<Location>,
	) -> Result<S3Object, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let parsed_made_on: Option<Timestamp> = made_on.map(|ts| ts.parse()).transpose()?;
		let location_geometry = location.map(|location| {
			format!("SRID=4326;POINT({} {})", location.longitude, location.latitude)
		});
		S3Object::try_from(RowContext(
			client
				.query_one(
					&client.prepare_cached(UPSERT_OBJECT_QUERY).await?,
					&[&name, &parsed_made_on, &location_geometry],
				)
				.await?,
			ctx.clone(),
		))
		.await
	}
}
