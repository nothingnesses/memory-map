use crate::{
	ContextWrapper,
	graphql::objects::{location::Location, s3_object::S3Object},
};
use async_graphql::{Context, Error as GraphQLError, Object};
use deadpool_postgres::Client;
use jiff::Timestamp;
use tracing;

const UPSERT_OBJECT_QUERY: &str = "INSERT INTO objects (name, made_on, location)
VALUES ($1, $2::timestamptz, ST_GeomFromEWKT($3))
ON CONFLICT (name) DO UPDATE
SET made_on = EXCLUDED.made_on, location = EXCLUDED.location
RETURNING id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude;";

pub struct Mutation;

impl Mutation {
	pub async fn upsert_s3_object_impl(
		client: &Client,
		name: String,
		made_on: Option<String>,
		location: Option<Location>,
	) -> Result<S3Object, GraphQLError> {
		let parsed_made_on: Option<Timestamp> = match made_on {
			Some(timestamp_string) => match timestamp_string.parse() {
				Ok(timestamp_string) => Some(timestamp_string),
				Err(error) => {
					tracing::error!("Failed to parse timestamp '{}': {}", timestamp_string, error);
					return Err(GraphQLError::new(format!(
						"Invalid timestamp format: {}",
						timestamp_string
					)));
				}
			},
			None => None,
		};
		let location_geometry = location.map(|location| {
			let location_geometry =
				format!("SRID=4326;POINT({} {})", location.longitude, location.latitude);
			tracing::debug!("Formatted location geometry: {}", location_geometry);
			location_geometry
		});
		tracing::debug!(
			"Executing upsert with: name={}, made_on={:?}, location={:?}",
			name,
			parsed_made_on.as_ref().map(|ts| ts.to_string()),
			location_geometry
		);
		S3Object::try_from(
			client
				.query_one(
					&client.prepare_cached(UPSERT_OBJECT_QUERY).await?,
					&[&name, &parsed_made_on, &location_geometry],
				)
				.await
				.map_err(|e| {
					tracing::error!("Database query failed: {}", e);
					GraphQLError::new(format!("Database error: {}", e))
				})?,
		)
		.await
	}
}

#[Object]
impl Mutation {
	async fn upsert_s3_object(
		&self,
		ctx: &Context<'_>,
		name: String,
		made_on: Option<String>,
		location: Option<Location>,
	) -> Result<S3Object, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		Self::upsert_s3_object_impl(&client, name, made_on, location).await
	}
}
