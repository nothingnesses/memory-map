use crate::{
	graphql_queries::update_s3_object::update_s3_object_mutation::{
		UpdateS3ObjectMutationUpdateS3Object as S3Object, Variables,
	},
	post_graphql_with_auth,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/updateS3Object.graphql",
	extern_enums("PublicityOverride"),
	response_derives = "Clone,Debug"
)]
pub struct UpdateS3ObjectMutation;

pub use crate::graphql_queries::types::PublicityOverride;

impl UpdateS3ObjectMutation {
	// @todo Add better error-handling
	/// Executes the UpdateS3ObjectQuery against the GraphQL API.
	pub async fn run(variables: Variables) -> Result<S3Object, Error> {
		Ok(post_graphql_with_auth::<UpdateS3ObjectMutation, _>(
			&reqwest::Client::new(),
			"http://127.0.0.1:8000/",
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.update_s3_object)?)
	}
}
