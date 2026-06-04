use {
	crate::graphql_queries::GraphqlOp,
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/requestPasswordReset.graphql",
	response_derives = "Clone,Debug"
)]
pub struct RequestPasswordResetMutation;

impl GraphqlOp for RequestPasswordResetMutation {
	type Output = bool;

	fn extract(data: request_password_reset_mutation::ResponseData) -> Self::Output {
		data.request_password_reset
	}
}
