use {
	crate::graphql_queries::GraphqlOp,
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/completeObjectUpload.graphql",
	extern_enums("PublicityOverride"),
	response_derives = "Clone,Debug"
)]
pub struct CompleteObjectUploadMutation;

pub use {
	self::complete_object_upload_mutation::CompletedObjectUploadPartInput as CompletedUploadPartInput,
	crate::graphql_queries::types::PublicityOverride,
};

use self::complete_object_upload_mutation::CompleteObjectUploadMutationCompleteObjectUpload as CompletedObjectUpload;

impl GraphqlOp for CompleteObjectUploadMutation {
	type Output = CompletedObjectUpload;

	fn extract(data: complete_object_upload_mutation::ResponseData) -> Self::Output {
		data.complete_object_upload
	}
}
