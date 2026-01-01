use crate::{
	graphql_queries::users::users_query::{UsersQueryUsers as User, Variables},
	post_graphql_with_auth, AppConfig,
};
use graphql_client::GraphQLQuery;
use leptos::{error::Error, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/users.graphql",
	response_derives = "Clone,Debug,PartialEq"
)]
pub struct UsersQuery;

pub use users_query::UserRole;

impl UsersQuery {
	pub async fn run() -> Result<Vec<User>, Error> {
		let config = use_context::<AppConfig>().expect("AppConfig missing");
		Ok(post_graphql_with_auth::<UsersQuery, _>(
			&reqwest::Client::new(),
			config.api_url,
			Variables {},
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.users)?)
	}
}
