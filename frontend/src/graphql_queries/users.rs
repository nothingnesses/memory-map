use crate::{
	graphql_queries::users::users_query::{UsersQueryUsers as User, Variables},
	post_graphql_with_auth,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/users.graphql",
	response_derives = "Clone,Debug"
)]
pub struct UsersQuery;

impl UsersQuery {
	pub async fn run() -> Result<Vec<User>, Error> {
		Ok(post_graphql_with_auth::<UsersQuery, _>(
			&reqwest::Client::new(),
			"http://127.0.0.1:8000/",
			Variables {},
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.users)?)
	}
}
