use crate::{graphql_queries::logout::logout_mutation::Variables, post_graphql};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/logout.graphql",
	response_derives = "Clone,Debug"
)]
pub struct LogoutMutation;

impl LogoutMutation {
	pub async fn run() -> Result<bool, Error> {
		Ok(post_graphql::<LogoutMutation, _>(
			&reqwest::Client::new(),
			"http://localhost:8000/",
			Variables {},
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.logout)?)
	}
}
