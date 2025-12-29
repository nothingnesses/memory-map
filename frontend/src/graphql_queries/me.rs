use crate::{
	graphql_queries::me::me_query::{MeQueryMe as User, Variables},
	post_graphql,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/me.graphql",
	response_derives = "Clone,Debug"
)]
pub struct MeQuery;

impl MeQuery {
	pub async fn run() -> Result<Option<User>, Error> {
		Ok(post_graphql::<MeQuery, _>(
			&reqwest::Client::new(),
			"http://localhost:8000/",
			Variables {},
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.me)?)
	}
}
