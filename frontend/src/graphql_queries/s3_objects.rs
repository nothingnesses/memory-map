use crate::{
	graphql_queries::s3_objects::s3_objects_query::{
		S3ObjectsQueryS3Objects as S3Object, Variables,
	},
	post_graphql_with_auth,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

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
	// @todo Add better error-handling
	pub async fn run() -> Result<Vec<S3Object>, Error> {
		Ok(post_graphql_with_auth::<S3ObjectsQuery, _>(
			&reqwest::Client::new(),
			"http://127.0.0.1:8000/",
			Variables {},
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.s3_objects)?)
	}
}
