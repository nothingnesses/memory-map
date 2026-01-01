use crate::{graphql_queries::logout::logout_mutation::Variables, post_graphql_with_auth, AppConfig};
use graphql_client::GraphQLQuery;
use leptos::{error::Error, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/logout.graphql",
	response_derives = "Clone,Debug"
)]
pub struct LogoutMutation;

impl LogoutMutation {
	pub async fn run() -> Result<bool, Error> {
		let config = use_context::<AppConfig>().expect("AppConfig missing");
		Ok(post_graphql_with_auth::<LogoutMutation, _>(
			&reqwest::Client::new(),
			config.api_url,
			Variables {},
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.logout)?)
	}
}
