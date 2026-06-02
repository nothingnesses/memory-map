use {
	crate::graphql_queries::{
		GraphqlOp,
		types::PublicityDefault,
		update_user_publicity::update_user_publicity_mutation::UpdateUserPublicityMutationUpdateUserPublicity as User,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/updateUserPublicity.graphql",
	extern_enums("PublicityDefault"),
	response_derives = "Clone,Debug"
)]
pub struct UpdateUserPublicityMutation;

impl GraphqlOp for UpdateUserPublicityMutation {
	type Output = User;

	fn extract(data: update_user_publicity_mutation::ResponseData) -> Self::Output {
		data.update_user_publicity
	}
}
