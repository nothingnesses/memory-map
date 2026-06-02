use {
	crate::graphql_queries::GraphqlOp,
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/deleteS3Objects.graphql",
	response_derives = "Clone,Debug"
)]
pub struct DeleteS3ObjectsMutation;

use self::delete_s3_objects_mutation::DeleteS3ObjectsMutationDeleteS3Objects as DeletedS3Object;

impl GraphqlOp for DeleteS3ObjectsMutation {
	type Output = Vec<DeletedS3Object>;

	fn extract(data: delete_s3_objects_mutation::ResponseData) -> Self::Output {
		data.delete_s3_objects
	}
}
