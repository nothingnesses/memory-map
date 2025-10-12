use async_graphql::{Context, Error as GraphQLError, ID, Object, SimpleObject};
use deadpool::managed::{Manager as ManagedManager, Object, Pool};
use deadpool_postgres::Manager;
use tokio_postgres::{Error as TPError, Row};

pub struct SchemaData<M: ManagedManager, W: From<Object<M>>> {
	pub pool: Pool<M, W>,
}

#[derive(SimpleObject)]
pub struct Location {
	id: ID,
	latitude: f64,
	longitude: f64,
}

struct ContextWrapper<'a>(&'a Context<'a>);

impl<'a> ContextWrapper<'a> {
	async fn get_client(&self) -> Result<Object<Manager>, GraphQLError> {
		let pool: &Pool<Manager> = &self.0.data::<SchemaData<Manager, _>>()?.pool;
		Ok(pool.get().await?)
	}
}

struct IDWrapper;

impl IDWrapper {
	pub fn from_i64(id: i64) -> ID {
		ID(id.to_string())
	}
}

impl Location {
	pub fn from_row(row: Row) -> Result<Self, TPError> {
		Ok(Location {
			id: IDWrapper::from_i64(row.try_get("id")?),
			latitude: row.try_get("latitude")?,
			longitude: row.try_get("longitude")?,
		})
	}

	pub async fn get(
		ctx: &Context<'_>,
		id: i64,
	) -> Result<Self, GraphQLError> {
		let client = ContextWrapper(ctx).get_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT id, ST_Y(location) AS latitude, ST_X(location) AS longitude
				FROM locations
				WHERE id = $1",
			)
			.await?;
		Ok(Location::from_row(client.query_one(&statement, &[&id]).await?)?)
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
		Location::get(ctx, id).await
	}

	async fn locations(
		&self,
		ctx: &Context<'_>,
	) -> Result<Vec<Location>, GraphQLError> {
		let client = ContextWrapper(ctx).get_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT id, ST_Y(location) AS latitude, ST_X(location) AS longitude
				FROM locations",
			)
			.await?;
		client
			.query(&statement, &[])
			.await
			.map_err(|e| GraphQLError::from(e))?
			.into_iter()
			.map(Location::from_row)
			.collect::<Result<Vec<_>, _>>()
			.map_err(|e| GraphQLError::from(e))
	}
}

struct Mutation;

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
			RETURNING id, ST_Y(location) AS latitude, ST_X(location) AS longitude",
			)
			.await?;
		Ok(Location::from_row(client.query_one(&statement, &[&longitude, &latitude]).await?)?)
	}
}
