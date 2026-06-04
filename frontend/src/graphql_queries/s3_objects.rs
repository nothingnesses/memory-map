use {
	crate::graphql_queries::{
		GraphqlOp,
		s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/s3Objects.graphql",
	extern_enums("PublicityOverride"),
	response_derives = "Clone,Debug"
)]
pub struct S3ObjectsQuery;

pub use crate::graphql_queries::types::PublicityOverride;

impl GraphqlOp for S3ObjectsQuery {
	type Output = Vec<S3Object>;

	fn extract(data: s3_objects_query::ResponseData) -> Self::Output {
		data.s3_objects
	}
}
