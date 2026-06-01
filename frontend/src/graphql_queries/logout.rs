use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		graphql_queries::logout::logout_mutation::Variables,
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/logout.graphql",
	response_derives = "Clone,Debug"
)]
pub struct LogoutMutation;

impl LogoutMutation {
	pub async fn run(api_url: String) -> Result<bool, AppError> {
		let response = post_graphql_with_auth::<LogoutMutation, _>(
			&reqwest::Client::new(),
			api_url,
			Variables {},
		)
		.await?;
		Ok(graphql_data(response)?.logout)
	}
}
