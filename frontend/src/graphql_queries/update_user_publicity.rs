use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		graphql_queries::{
			types::PublicityDefault,
			update_user_publicity::update_user_publicity_mutation::{
				UpdateUserPublicityMutationUpdateUserPublicity as User,
				Variables,
			},
		},
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/updateUserPublicity.graphql",
	extern_enums("PublicityDefault"),
	response_derives = "Clone,Debug"
)]
pub struct UpdateUserPublicityMutation;

impl UpdateUserPublicityMutation {
	pub async fn run(
		api_url: String,
		variables: Variables,
	) -> Result<User, AppError> {
		let response = post_graphql_with_auth::<UpdateUserPublicityMutation, _>(
			&reqwest::Client::new(),
			api_url,
			variables,
		)
		.await?;
		Ok(graphql_data(response)?.update_user_publicity)
	}
}
