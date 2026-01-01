use crate::{
	graphql_queries::admin_update_user::admin_update_user_mutation::{
		AdminUpdateUserMutationAdminUpdateUser as User, Variables,
	},
	post_graphql_with_auth,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

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
	) -> Result<User, Error> {
		Ok(post_graphql_with_auth::<AdminUpdateUserMutation, _>(
			&reqwest::Client::new(),
			api_url,
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.admin_update_user)?)
	}
}
