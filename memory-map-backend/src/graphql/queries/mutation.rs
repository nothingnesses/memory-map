use crate::graphql::{
	ContextWrapper,
	objects::{RowContext, location::Location, s3object::S3Object},
};
use async_graphql::{Context, Error as GraphQLError, Object};
use jiff::Timestamp;

pub struct Mutation;

#[Object]
impl Mutation {
	async fn add_s3_object(
		&self,
		ctx: &Context<'_>,
		name: String,
		made_on: Option<String>,
		location: Option<Location>,
	) -> Result<S3Object, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let parsed_made_on: Option<Timestamp> = match made_on {
			Some(ts) => Some(ts.parse()?),
			None => None,
		};
		match location {
			Some(location) => {
				let statement = client
			.prepare_cached(
				"INSERT INTO objects (name, made_on, location)
				VALUES ($1, $2, ST_SetSRID(ST_MakePoint($3, $4), 4326))
				RETURNING id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude;",
			)
			.await?;
				let row = client
					.query_one(
						&statement,
						&[&name, &parsed_made_on, &location.longitude, &location.latitude],
					)
					.await?;
				S3Object::try_from(RowContext(row, ctx.clone())).await
			}
			None => {
				let statement = client
			.prepare_cached(
				"INSERT INTO objects (name, made_on, location)
				VALUES ($1, $2, NULL)
				RETURNING id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude;",
			)
			.await?;
				let row = client.query_one(&statement, &[&name, &parsed_made_on]).await?;
				S3Object::try_from(RowContext(row, ctx.clone())).await
			}
		}
	}
}
