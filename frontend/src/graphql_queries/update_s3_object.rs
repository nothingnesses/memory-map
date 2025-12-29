use crate::{
	graphql_queries::update_s3_object::update_s3_object_mutation::{
		UpdateS3ObjectMutationUpdateS3Object as S3Object, Variables,
	},
	post_graphql,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/updateS3Object.graphql",
	response_derives = "Clone,Debug"
)]
pub struct UpdateS3ObjectMutation;

impl UpdateS3ObjectMutation {
	// @todo Add better error-handling
	/// Executes the UpdateS3ObjectQuery against the GraphQL API.
	pub async fn run(variables: Variables) -> Result<S3Object, Error> {
		Ok(post_graphql::<UpdateS3ObjectMutation, _>(
			&reqwest::Client::new(),
			"http://localhost:8000/",
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.update_s3_object)?)
	}
}
