use {
	crate::graphql_queries::GraphqlOp,
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/logout.graphql",
	response_derives = "Clone,Debug"
)]
pub struct LogoutMutation;

impl GraphqlOp for LogoutMutation {
	type Output = bool;

	fn extract(data: logout_mutation::ResponseData) -> Self::Output {
		data.logout
	}
}
