use crate::{
	graphql_queries::change_email::change_email_mutation::{
		ChangeEmailMutationChangeEmail as User, Variables,
	},
	post_graphql,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/changeEmail.graphql",
	response_derives = "Clone,Debug"
)]
pub struct ChangeEmailMutation;

impl ChangeEmailMutation {
	pub async fn run(variables: Variables) -> Result<User, Error> {
		Ok(post_graphql::<ChangeEmailMutation, _>(
			&reqwest::Client::new(),
			"http://localhost:8000/",
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.change_email)?)
	}
}
