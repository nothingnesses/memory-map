use crate::{
	AppConfig,
	graphql_queries::admin_update_user::admin_update_user_mutation::{
		AdminUpdateUserMutationAdminUpdateUser as User, Variables,
	},
	post_graphql_with_auth,
};
use graphql_client::GraphQLQuery;
use leptos::{error::Error, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/adminUpdateUser.graphql",
	response_derives = "Clone,Debug"
)]
pub struct AdminUpdateUserMutation;

impl AdminUpdateUserMutation {
	pub async fn run(variables: Variables) -> Result<User, Error> {
		let config = use_context::<AppConfig>().expect("AppConfig missing");
		Ok(post_graphql_with_auth::<AdminUpdateUserMutation, _>(
			&reqwest::Client::new(),
			config.api_url,
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.admin_update_user)?)
	}
}
