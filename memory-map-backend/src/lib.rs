use async_graphql::{
	Context, Error as GraphQLError, ID, Object, SimpleObject, http::GraphiQLSource,
};
use axum::response::{self, IntoResponse};
use deadpool::managed::{Manager as ManagedManager, Object, Pool};
use deadpool_postgres::Manager;
use tokio_postgres::{Error as TPError, Row};

#[derive(Debug, serde::Deserialize)]
pub struct Config {
	pub pg: deadpool_postgres::Config,
}

impl Config {
	pub fn from_env() -> Result<Self, config::ConfigError> {
		config::Config::builder()
			.add_source(config::Environment::default().separator("__"))
			.build()?
			.try_deserialize()
	}
}

refinery::embed_migrations!("migrations");

pub struct SchemaData<M: ManagedManager, W: From<Object<M>>> {
	pub pool: Pool<M, W>,
}

struct ContextWrapper<'a>(&'a Context<'a>);

impl<'a> ContextWrapper<'a> {
	async fn get_client(&self) -> Result<Object<Manager>, GraphQLError> {
		let pool: &Pool<Manager> = &self.0.data::<SchemaData<Manager, _>>()?.pool;
		Ok(pool.get().await?)
	}
}

#[derive(SimpleObject)]
pub struct Location {
	id: ID,
	latitude: f64,
	longitude: f64,
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
				"SELECT id, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude
				FROM locations
				WHERE id = $1",
			)
			.await?;
		Ok(Location::try_from(client.query_one(&statement, &[&id]).await?)?)
	}
}

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

pub struct Mutation;

#[Object]
impl Mutation {
	async fn add_location(
		&self,
		ctx: &Context<'_>,
		longitude: f64,
		latitude: f64,
	) -> Result<Location, GraphQLError> {
		let client = ContextWrapper(ctx).get_client().await?;
		let statement = client
			.prepare_cached(
				"INSERT INTO locations (location)
				VALUES (ST_SetSRID(ST_MakePoint($1, $2), 4326))
				RETURNING id, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude",
			)
			.await?;
		Ok(Location::try_from(client.query_one(&statement, &[&longitude, &latitude]).await?)?)
	}
}

pub async fn graphiql() -> impl IntoResponse {
	response::Html(GraphiQLSource::build().endpoint("/").finish())
}
