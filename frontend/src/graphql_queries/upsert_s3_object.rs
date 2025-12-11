use crate::{
	graphql_queries::upsert_s3_object::upsert_s3_object_query::{
		UpsertS3ObjectQueryUpsertS3Object as S3Object, Variables,
	},
	post_graphql,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/upsertS3Object.graphql",
	response_derives = "Clone,Debug"
)]
pub struct UpsertS3ObjectQuery;

impl UpsertS3ObjectQuery {
	pub async fn run(variables: Variables) -> Result<S3Object, Error> {
		Ok(post_graphql::<UpsertS3ObjectQuery, _>(
			&reqwest::Client::new(),
			"http://localhost:8000/",
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.upsert_s3_object)?)
	}
}
