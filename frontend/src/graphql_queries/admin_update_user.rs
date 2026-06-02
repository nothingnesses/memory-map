use {
	crate::graphql_queries::{
		GraphqlOp,
		admin_update_user::admin_update_user_mutation::AdminUpdateUserMutationAdminUpdateUser as User,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/adminUpdateUser.graphql",
	response_derives = "Clone,Debug"
)]
pub struct AdminUpdateUserMutation;

impl GraphqlOp for AdminUpdateUserMutation {
	type Output = User;

	fn extract(data: admin_update_user_mutation::ResponseData) -> Self::Output {
		data.admin_update_user
	}
}
