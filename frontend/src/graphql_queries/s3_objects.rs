use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		graphql_queries::s3_objects::s3_objects_query::{
			S3ObjectsQueryS3Objects as S3Object,
			Variables,
		},
		post_graphql_with_auth,
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

impl S3ObjectsQuery {
	pub async fn run(api_url: String) -> Result<Vec<S3Object>, AppError> {
		let response = post_graphql_with_auth::<S3ObjectsQuery, _>(
			&reqwest::Client::new(),
			api_url,
			Variables {},
		)
		.await?;
		Ok(graphql_data(response)?.s3_objects)
	}
}
