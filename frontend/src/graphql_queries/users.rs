use {
	crate::graphql_queries::{
		GraphqlOp,
		users::users_query::UsersQueryUsers as User,
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

impl GraphqlOp for UsersQuery {
	type Output = Vec<User>;

	fn extract(data: users_query::ResponseData) -> Self::Output {
		data.users
	}
}
