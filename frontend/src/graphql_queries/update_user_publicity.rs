use crate::{
	graphql_queries::{
		types::PublicityDefault,
		update_user_publicity::update_user_publicity_mutation::{
			UpdateUserPublicityMutationUpdateUserPublicity as User, Variables,
		},
	},
	post_graphql_with_auth,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/updateUserPublicity.graphql",
	extern_enums("PublicityDefault"),
	response_derives = "Clone,Debug"
)]
pub struct UpdateUserPublicityMutation;

impl UpdateUserPublicityMutation {
	pub async fn run(variables: Variables) -> Result<User, Error> {
		Ok(post_graphql_with_auth::<UpdateUserPublicityMutation, _>(
			&reqwest::Client::new(),
			"http://127.0.0.1:8000/",
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.update_user_publicity)?)
	}
}
