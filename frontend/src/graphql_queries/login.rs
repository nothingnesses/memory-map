use crate::{
	graphql_queries::login::login_mutation::{LoginMutationLogin as User, Variables},
	post_graphql_with_auth, AppConfig,
};
use graphql_client::GraphQLQuery;
use leptos::{error::Error, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/login.graphql",
	response_derives = "Clone,Debug"
)]
pub struct LoginMutation;

impl LoginMutation {
	pub async fn run(variables: Variables) -> Result<User, Error> {
		let config = use_context::<AppConfig>().expect("AppConfig missing");
		Ok(post_graphql_with_auth::<LoginMutation, _>(
			&reqwest::Client::new(),
			config.api_url,
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.login)?)
	}
}
