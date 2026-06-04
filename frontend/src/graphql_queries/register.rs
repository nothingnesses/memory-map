use {
	crate::graphql_queries::{
		GraphqlOp,
		register::register_mutation::RegisterMutationRegister as User,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/register.graphql",
	response_derives = "Clone,Debug"
)]
pub struct RegisterMutation;

impl GraphqlOp for RegisterMutation {
	type Output = User;

	fn extract(data: register_mutation::ResponseData) -> Self::Output {
		data.register
	}
}
