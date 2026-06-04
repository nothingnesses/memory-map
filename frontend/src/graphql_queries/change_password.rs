use {
	crate::graphql_queries::GraphqlOp,
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/changePassword.graphql",
	response_derives = "Clone,Debug"
)]
pub struct ChangePasswordMutation;

impl GraphqlOp for ChangePasswordMutation {
	type Output = bool;

	fn extract(data: change_password_mutation::ResponseData) -> Self::Output {
		data.change_password
	}
}
