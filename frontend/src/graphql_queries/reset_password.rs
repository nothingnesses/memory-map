use {
	crate::graphql_queries::GraphqlOp,
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/resetPassword.graphql",
	response_derives = "Clone,Debug"
)]
pub struct ResetPasswordMutation;

impl GraphqlOp for ResetPasswordMutation {
	type Output = bool;

	fn extract(data: reset_password_mutation::ResponseData) -> Self::Output {
		data.reset_password
	}
}
