use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		graphql_queries::register::register_mutation::{
			RegisterMutationRegister as User,
			Variables,
		},
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/register.graphql",
	response_derives = "Clone,Debug"
)]
pub struct RegisterMutation;

impl RegisterMutation {
	pub async fn run(
		api_url: String,
		variables: Variables,
	) -> Result<User, AppError> {
		let response = post_graphql_with_auth::<RegisterMutation, _>(
			&reqwest::Client::new(),
			api_url,
			variables,
		)
		.await?;
		Ok(graphql_data(response)?.register)
	}
}
