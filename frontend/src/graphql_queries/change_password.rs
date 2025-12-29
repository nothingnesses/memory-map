use crate::{graphql_queries::change_password::change_password_mutation::Variables, post_graphql};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/changePassword.graphql",
	response_derives = "Clone,Debug"
)]
pub struct ChangePasswordMutation;

impl ChangePasswordMutation {
	pub async fn run(variables: Variables) -> Result<bool, Error> {
		Ok(post_graphql::<ChangePasswordMutation, _>(
			&reqwest::Client::new(),
			"http://127.0.0.1:8000/",
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.change_password)?)
	}
}
