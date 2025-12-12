use crate::AxumState;
use async_graphql::{Context, Error as GraphQLError};
use deadpool::managed::{Manager as ManagedManager, Object, Pool};
use deadpool_postgres::Manager;
use std::sync::Arc;

pub mod objects;
pub mod queries;

pub struct SchemaData<M: ManagedManager, W: From<Object<M>>> {
	pub pool: Pool<M, W>,
	pub axum_state: Arc<AxumState>,
}

struct ContextWrapper<'a>(&'a Context<'a>);

impl<'a> ContextWrapper<'a> {
	async fn get_db_client(&self) -> Result<Object<Manager>, GraphQLError> {
		let pool: &Pool<Manager> = &self.0.data::<SchemaData<Manager, _>>()?.pool;
		Ok(pool.get().await?)
	}
}
