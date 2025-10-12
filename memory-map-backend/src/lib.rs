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
		Ok(Location::from_row(
			ContextWrapper(ctx)
				.get_client()
				.await?
				.query_one(
					"SELECT id, ST_Y(location) AS latitude, ST_X(location) AS longitude
			FROM locations
			WHERE id = $1",
					&[&id],
				)
				.await
				.map_err(|e| GraphQLError::from(e))?,
		)?)
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
		todo!()
	}
}

struct Mutation;

#[Object]
impl Mutation {
	async fn add_location(
		&self,
		ctx: &Context<'_>,
		latitude: f64,
		longitude: f64,
	) -> Result<Location, GraphQLError> {
		Ok(Location::from_row(
			ContextWrapper(ctx)
				.get_client()
				.await?
				.query_one(
					"INSERT INTO locations (location)
				VALUES (ST_SetSRID(ST_MakePoint($1, $2), 4326))
				RETURNING id, ST_Y(location) AS latitude, ST_X(location) AS longitude",
					&[&longitude, &latitude],
				)
				.await
				.map_err(|e| GraphQLError::from(e))?,
		)?)
	}
}
