use async_graphql::{Context, ID, InputObject, Object, SimpleObject};

#[derive(Clone, SimpleObject, InputObject)]
pub struct Location {
	id: ID,
	latitude: f64,
	longitude: f64,
}

pub struct Query;

#[Object]
impl Query {
	async fn locations(
		&self,
		ctx: &Context<'_>,
	) -> Vec<Location> {
		vec![Location { id: ID("".to_string()), latitude: 0.0, longitude: 0.0 }]
	}
}
