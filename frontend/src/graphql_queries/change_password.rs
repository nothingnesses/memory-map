use crate::{
	AppConfig, graphql_queries::change_password::change_password_mutation::Variables,
	post_graphql_with_auth,
};
use graphql_client::GraphQLQuery;
use leptos::{error::Error, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/changePassword.graphql",
	response_derives = "Clone,Debug"
)]
pub struct ChangePasswordMutation;

impl ChangePasswordMutation {
	pub async fn run(variables: Variables) -> Result<bool, Error> {
		let config = use_context::<AppConfig>().expect("AppConfig missing");
		Ok(post_graphql_with_auth::<ChangePasswordMutation, _>(
			&reqwest::Client::new(),
			config.api_url,
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.change_password)?)
	}
}
