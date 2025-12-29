use crate::{
	graphql_queries::request_password_reset::request_password_reset_mutation::Variables,
	post_graphql,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/requestPasswordReset.graphql",
	response_derives = "Clone,Debug"
)]
pub struct RequestPasswordResetMutation;

impl RequestPasswordResetMutation {
	pub async fn run(variables: Variables) -> Result<bool, Error> {
		Ok(post_graphql::<RequestPasswordResetMutation, _>(
			&reqwest::Client::new(),
			"http://127.0.0.1:8000/",
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.request_password_reset)?)
	}
}
