use {
	crate::graphql_queries::GraphqlOp,
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/abortObjectUpload.graphql",
	response_derives = "Clone,Debug"
)]
pub struct AbortObjectUploadMutation;

use self::abort_object_upload_mutation::AbortObjectUploadMutationAbortObjectUpload as AbortedObjectUpload;

impl GraphqlOp for AbortObjectUploadMutation {
	type Output = AbortedObjectUpload;

	fn extract(data: abort_object_upload_mutation::ResponseData) -> Self::Output {
		data.abort_object_upload
	}
}
