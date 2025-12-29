use crate::{
	graphql_queries::s3_object_by_id::s3_object_by_id_query::{
		S3ObjectByIdQueryS3ObjectById as S3Object, Variables,
	},
	post_graphql,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/s3ObjectById.graphql",
	response_derives = "Clone,Debug,Serialize,Deserialize"
)]
pub struct S3ObjectByIdQuery;

impl S3ObjectByIdQuery {
	// @todo Add better error-handling
	pub async fn run(id: i64) -> Result<S3Object, Error> {
		Ok(post_graphql::<S3ObjectByIdQuery, _>(
			&reqwest::Client::new(),
			"http://127.0.0.1:8000/",
			Variables { id },
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.s3_object_by_id)?)
	}
}
