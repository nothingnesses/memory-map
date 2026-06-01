use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		graphql_queries::change_email::change_email_mutation::{
			ChangeEmailMutationChangeEmail as User,
			Variables,
		},
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/changeEmail.graphql",
	response_derives = "Clone,Debug"
)]
pub struct ChangeEmailMutation;

impl ChangeEmailMutation {
	pub async fn run(
		api_url: String,
		variables: Variables,
	) -> Result<User, AppError> {
		let response = post_graphql_with_auth::<ChangeEmailMutation, _>(
			&reqwest::Client::new(),
			api_url,
			variables,
		)
		.await?;
		Ok(graphql_data(response)?.change_email)
	}
}
