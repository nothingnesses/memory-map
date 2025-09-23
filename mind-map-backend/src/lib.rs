use async_graphql::{Object, Context};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
	async fn bool_true(
		&self,
		ctx: &Context<'_>,
	) -> bool {
		true
	}
}
