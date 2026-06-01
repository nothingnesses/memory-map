use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		graphql_queries::login::login_mutation::{
			LoginMutationLogin as User,
			Variables,
		},
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
};

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
	) -> Result<User, AppError> {
		let response =
			post_graphql_with_auth::<LoginMutation, _>(&reqwest::Client::new(), api_url, variables)
				.await?;
		Ok(graphql_data(response)?.login)
	}
}
