use crate::{
	graphql_queries::login::login_mutation::{LoginMutationLogin as User, Variables},
	post_graphql_with_auth,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/login.graphql",
	response_derives = "Clone,Debug"
)]
pub struct LoginMutation;

impl LoginMutation {
	pub async fn run(
		api_url: String,
		variables: Variables,
	) -> Result<User, Error> {
		Ok(post_graphql_with_auth::<LoginMutation, _>(&reqwest::Client::new(), api_url, variables)
			.await?
			.data
			.ok_or("Empty response".to_string())
			.map(|response| response.login)?)
	}
}
