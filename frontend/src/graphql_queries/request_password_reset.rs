use crate::{
	graphql_queries::request_password_reset::request_password_reset_mutation::Variables,
	post_graphql_with_auth, AppConfig,
};
use graphql_client::GraphQLQuery;
use leptos::{error::Error, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/requestPasswordReset.graphql",
	response_derives = "Clone,Debug"
)]
pub struct RequestPasswordResetMutation;

impl RequestPasswordResetMutation {
	pub async fn run(variables: Variables) -> Result<bool, Error> {
		let config = use_context::<AppConfig>().expect("AppConfig missing");
		Ok(post_graphql_with_auth::<RequestPasswordResetMutation, _>(
			&reqwest::Client::new(),
			config.api_url,
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.request_password_reset)?)
	}
}
