use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		graphql_queries::change_password::change_password_mutation::Variables,
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/changePassword.graphql",
	response_derives = "Clone,Debug"
)]
pub struct ChangePasswordMutation;

impl ChangePasswordMutation {
	pub async fn run(
		api_url: String,
		variables: Variables,
	) -> Result<bool, AppError> {
		let response = post_graphql_with_auth::<ChangePasswordMutation, _>(
			&reqwest::Client::new(),
			api_url,
			variables,
		)
		.await?;
		Ok(graphql_data(response)?.change_password)
	}
}
