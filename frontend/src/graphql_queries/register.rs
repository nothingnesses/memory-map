use crate::{
	graphql_queries::register::register_mutation::{RegisterMutationRegister as User, Variables},
	post_graphql,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/register.graphql",
	response_derives = "Clone,Debug"
)]
pub struct RegisterMutation;

impl RegisterMutation {
	pub async fn run(variables: Variables) -> Result<User, Error> {
		Ok(post_graphql::<RegisterMutation, _>(
			&reqwest::Client::new(),
			"http://127.0.0.1:8000/",
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.register)?)
	}
}
