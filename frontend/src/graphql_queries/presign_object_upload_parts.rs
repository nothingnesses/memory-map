use {
	crate::graphql_queries::GraphqlOp,
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/presignObjectUploadParts.graphql",
	response_derives = "Clone,Debug"
)]
pub struct PresignObjectUploadPartsMutation;

use self::presign_object_upload_parts_mutation::PresignObjectUploadPartsMutationPresignObjectUploadParts as PresignedObjectUploadPart;

impl GraphqlOp for PresignObjectUploadPartsMutation {
	type Output = Vec<PresignedObjectUploadPart>;

	fn extract(data: presign_object_upload_parts_mutation::ResponseData) -> Self::Output {
		data.presign_object_upload_parts
	}
}

pub type PresignedUploadPart = PresignedObjectUploadPart;
