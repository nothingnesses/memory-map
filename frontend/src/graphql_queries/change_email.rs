use crate::{
	graphql_queries::change_email::change_email_mutation::{
		ChangeEmailMutationChangeEmail as User, Variables,
	},
	post_graphql_with_auth, AppConfig,
};
use graphql_client::GraphQLQuery;
use leptos::{error::Error, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/changeEmail.graphql",
	response_derives = "Clone,Debug"
)]
pub struct ChangeEmailMutation;

impl ChangeEmailMutation {
	pub async fn run(variables: Variables) -> Result<User, Error> {
		let config = use_context::<AppConfig>().expect("AppConfig missing");
		Ok(post_graphql_with_auth::<ChangeEmailMutation, _>(
			&reqwest::Client::new(),
			config.api_url,
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.change_email)?)
	}
}
