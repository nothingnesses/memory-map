use crate::{graphql_queries::reset_password::reset_password_mutation::Variables, post_graphql_with_auth};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/resetPassword.graphql",
	response_derives = "Clone,Debug"
)]
pub struct ResetPasswordMutation;

impl ResetPasswordMutation {
	pub async fn run(variables: Variables) -> Result<bool, Error> {
		Ok(post_graphql_with_auth::<ResetPasswordMutation, _>(
			&reqwest::Client::new(),
			"http://127.0.0.1:8000/",
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.reset_password)?)
	}
}
