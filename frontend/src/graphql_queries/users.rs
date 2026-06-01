use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		graphql_queries::users::users_query::{
			UsersQueryUsers as User,
			Variables,
		},
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/users.graphql",
	response_derives = "Clone,Debug,PartialEq"
)]
pub struct UsersQuery;

pub use users_query::UserRole;

impl UsersQuery {
	pub async fn run(api_url: String) -> Result<Vec<User>, AppError> {
		let response =
			post_graphql_with_auth::<UsersQuery, _>(&reqwest::Client::new(), api_url, Variables {})
				.await?;
		Ok(graphql_data(response)?.users)
	}
}
