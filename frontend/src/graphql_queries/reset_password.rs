use crate::{
	AppConfig, graphql_queries::reset_password::reset_password_mutation::Variables,
	post_graphql_with_auth,
};
use graphql_client::GraphQLQuery;
use leptos::{error::Error, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/resetPassword.graphql",
	response_derives = "Clone,Debug"
)]
pub struct ResetPasswordMutation;

impl ResetPasswordMutation {
	pub async fn run(variables: Variables) -> Result<bool, Error> {
		let config = use_context::<AppConfig>().expect("AppConfig missing");
		Ok(post_graphql_with_auth::<ResetPasswordMutation, _>(
			&reqwest::Client::new(),
			config.api_url,
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.reset_password)?)
	}
}
