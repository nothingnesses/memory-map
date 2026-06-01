use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		graphql_queries::request_password_reset::request_password_reset_mutation::Variables,
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/requestPasswordReset.graphql",
	response_derives = "Clone,Debug"
)]
pub struct RequestPasswordResetMutation;

impl RequestPasswordResetMutation {
	pub async fn run(
		api_url: String,
		variables: Variables,
	) -> Result<bool, AppError> {
		let response = post_graphql_with_auth::<RequestPasswordResetMutation, _>(
			&reqwest::Client::new(),
			api_url,
			variables,
		)
		.await?;
		Ok(graphql_data(response)?.request_password_reset)
	}
}
