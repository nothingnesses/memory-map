use {
	crate::graphql_queries::{
		GraphqlOp,
		change_email::change_email_mutation::ChangeEmailMutationChangeEmail as User,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/changeEmail.graphql",
	response_derives = "Clone,Debug"
)]
pub struct ChangeEmailMutation;

impl GraphqlOp for ChangeEmailMutation {
	type Output = User;

	fn extract(data: change_email_mutation::ResponseData) -> Self::Output {
		data.change_email
	}
}
