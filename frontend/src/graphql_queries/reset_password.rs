use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		graphql_queries::reset_password::reset_password_mutation::Variables,
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/resetPassword.graphql",
	response_derives = "Clone,Debug"
)]
pub struct ResetPasswordMutation;

impl ResetPasswordMutation {
	pub async fn run(
		api_url: String,
		variables: Variables,
	) -> Result<bool, AppError> {
		let response = post_graphql_with_auth::<ResetPasswordMutation, _>(
			&reqwest::Client::new(),
			api_url,
			variables,
		)
		.await?;
		Ok(graphql_data(response)?.reset_password)
	}
}
