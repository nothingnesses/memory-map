use {
	crate::graphql_queries::{
		GraphqlOp,
		login::login_mutation::LoginMutationLogin as User,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/login.graphql",
	response_derives = "Clone,Debug"
)]
pub struct LoginMutation;

impl GraphqlOp for LoginMutation {
	type Output = User;

	fn extract(data: login_mutation::ResponseData) -> Self::Output {
		data.login
	}
}
