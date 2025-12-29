use crate::{
	graphql_queries::admin_update_user::admin_update_user_mutation::{
		AdminUpdateUserMutationAdminUpdateUser as User, Variables,
	},
	post_graphql,
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
	pub async fn run(variables: Variables) -> Result<User, Error> {
		Ok(post_graphql::<AdminUpdateUserMutation, _>(
			&reqwest::Client::new(),
			"http://127.0.0.1:8000/",
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.admin_update_user)?)
	}
}
