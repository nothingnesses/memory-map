use crate::{
	ContextWrapper,
	graphql::objects::{location::Location, s3_object::S3Object},
};
use async_graphql::{Context, Error as GraphQLError, ID, Object};
use deadpool_postgres::Client;
use jiff::Timestamp;
use tracing;

const DELETE_OBJECTS_QUERY: &str = "DELETE FROM objects WHERE id = ANY($1) RETURNING id;";

const UPSERT_OBJECT_QUERY: &str = "INSERT INTO objects (name, made_on, location)
VALUES ($1, $2::timestamptz, ST_GeomFromEWKT($3))
ON CONFLICT (name) DO UPDATE
SET made_on = EXCLUDED.made_on, location = EXCLUDED.location
RETURNING id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude;";

pub struct Mutation;

impl Mutation {
	pub async fn delete_s3_objects_worker(
		client: &Client,
		ids: &[i64],
	) -> Result<Vec<i64>, GraphQLError> {
		let statement = client.prepare_cached(DELETE_OBJECTS_QUERY).await.map_err(|e| {
			tracing::error!("Failed to prepare query: {}", e);
			GraphQLError::new(format!("Database error: {}", e))
		})?;

		let rows = client.query(&statement, &[&ids]).await.map_err(|e| {
			tracing::error!("Database query failed: {}", e);
			GraphQLError::new(format!("Database error: {}", e))
		})?;

		Ok(rows.iter().map(|row| row.get("id")).collect())
	}

	pub async fn upsert_s3_object_worker(
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
	async fn delete_s3_objects(
		&self,
		ctx: &Context<'_>,
		ids: Vec<ID>,
	) -> Result<Vec<ID>, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let ids: Vec<i64> = ids
			.into_iter()
			.map(|id| {
				id.parse::<i64>()
					.map_err(|e| GraphQLError::new(format!("Invalid ID format: {}", e)))
			})
			.collect::<Result<Vec<i64>, _>>()?;

		Ok(Self::delete_s3_objects_worker(&client, &ids).await?.into_iter().map(ID::from).collect())
	}

	async fn upsert_s3_object(
		&self,
		ctx: &Context<'_>,
		name: String,
		made_on: Option<String>,
		location: Option<Location>,
	) -> Result<S3Object, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		Self::upsert_s3_object_worker(&client, name, made_on, location).await
	}
}
