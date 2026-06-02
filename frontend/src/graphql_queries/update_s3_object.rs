use {
	crate::graphql_queries::{
		GraphqlOp,
		update_s3_object::update_s3_object_mutation::UpdateS3ObjectMutationUpdateS3Object as S3Object,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/updateS3Object.graphql",
	extern_enums("PublicityOverride"),
	response_derives = "Clone,Debug"
)]
pub struct UpdateS3ObjectMutation;

pub use crate::graphql_queries::types::PublicityOverride;

impl GraphqlOp for UpdateS3ObjectMutation {
	type Output = S3Object;

	fn extract(data: update_s3_object_mutation::ResponseData) -> Self::Output {
		data.update_s3_object
	}
}
