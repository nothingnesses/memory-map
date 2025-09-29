use async_graphql::{Context, ID, InputObject, Object, SimpleObject};
use deadpool::managed::{Manager, Object, Pool};

#[derive(Clone)]
pub struct Location {
	id: ID,
	latitude: String,
	longitude: String,
}

#[Object]
impl Location {
	async fn id(&self) -> &ID {
		&self.id
	}

	async fn latitude(&self) -> &str {
		&self.latitude
	}

	async fn longitude(&self) -> &str {
		&self.longitude
	}
}

pub struct Query;

#[Object]
impl Query {
	async fn locations(
		&self,
		ctx: &Context<'_>,
	) -> Vec<Location> {
		vec![Location {
			id: ID("".to_string()),
			latitude: "".to_string(),
			longitude: "0.0".to_string(),
		}]
	}
}

pub struct SchemaData<M: Manager, W: From<Object<M>>> {
	pub pool: Pool<M, W>
}
