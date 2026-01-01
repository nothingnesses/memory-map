use crate::{graphql_queries::logout::logout_mutation::Variables, post_graphql_with_auth};
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
	pub async fn run(api_url: String) -> Result<bool, Error> {
		Ok(post_graphql_with_auth::<LogoutMutation, _>(
			&reqwest::Client::new(),
			api_url,
			Variables {},
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.logout)?)
	}
}
