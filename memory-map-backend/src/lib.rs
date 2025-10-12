use async_graphql::{Context, Error as GraphQLError, ID, Object, SimpleObject};
use deadpool::managed::{Manager as ManagedManager, Object, Pool, PoolError};
use deadpool_postgres::Manager;

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

impl Location {
	pub async fn get(
		ctx: &Context<'_>,
		id: i64,
	) -> Result<Self, GraphQLError> {
		let row = ContextWrapper(ctx)
			.get_client()
			.await?
			.query_one(
				"SELECT id, ST_Y(location) AS latitude, ST_X(location) AS longitude
				FROM locations
				WHERE id = $1",
				&[&id],
			)
			.await
			.map_err(|e| GraphQLError::from(e))?;

		Ok(Location {
			id: ID(row.get::<_, i64>(0).to_string()),
			latitude: row.get("latitude"),
			longitude: row.get("longitude"),
		})
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
		let client = ContextWrapper(ctx).get_client().await?;
		let id: i64 = client
			.query_one(
				"INSERT INTO locations (location)
				VALUES (ST_SetSRID(ST_MakePoint($1, $2), 4326))
				RETURNING id",
				&[&longitude, &latitude],
			)
			.await
			.map_err(|e| GraphQLError::from(e))?
			.get(0);

		Location::get(ctx, id).await
	}
}
