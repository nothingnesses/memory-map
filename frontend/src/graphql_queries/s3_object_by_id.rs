use {
	crate::graphql_queries::{
		GraphqlOp,
		s3_object_by_id::s3_object_by_id_query::S3ObjectByIdQueryS3ObjectById as S3Object,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/s3ObjectById.graphql",
	extern_enums("PublicityOverride"),
	response_derives = "Clone,Debug,Serialize,Deserialize"
)]
pub struct S3ObjectByIdQuery;

pub use crate::graphql_queries::types::PublicityOverride;

impl GraphqlOp for S3ObjectByIdQuery {
	type Output = S3Object;

	fn extract(data: s3_object_by_id_query::ResponseData) -> Self::Output {
		data.s3_object_by_id
	}
}
