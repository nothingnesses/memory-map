use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		graphql_queries::admin_update_user::admin_update_user_mutation::{
			AdminUpdateUserMutationAdminUpdateUser as User,
			Variables,
		},
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/adminUpdateUser.graphql",
	response_derives = "Clone,Debug"
)]
pub struct AdminUpdateUserMutation;

impl AdminUpdateUserMutation {
	pub async fn run(
		api_url: String,
		variables: Variables,
	) -> Result<User, AppError> {
		let response = post_graphql_with_auth::<AdminUpdateUserMutation, _>(
			&reqwest::Client::new(),
			api_url,
			variables,
		)
		.await?;
		Ok(graphql_data(response)?.admin_update_user)
	}
}
