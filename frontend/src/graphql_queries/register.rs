use crate::{
	AppConfig,
	graphql_queries::register::register_mutation::{RegisterMutationRegister as User, Variables},
	post_graphql_with_auth,
};
use graphql_client::GraphQLQuery;
use leptos::{error::Error, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/register.graphql",
	response_derives = "Clone,Debug"
)]
pub struct RegisterMutation;

impl RegisterMutation {
	pub async fn run(variables: Variables) -> Result<User, Error> {
		let config = use_context::<AppConfig>().expect("AppConfig missing");
		Ok(post_graphql_with_auth::<RegisterMutation, _>(
			&reqwest::Client::new(),
			config.api_url,
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.register)?)
	}
}
