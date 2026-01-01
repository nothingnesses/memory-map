use crate::{
	AppConfig,
	graphql_queries::upsert_s3_object::upsert_s3_object_mutation::{
		UpsertS3ObjectMutationUpsertS3Object as S3Object, Variables,
	},
	post_graphql_with_auth,
};
use graphql_client::GraphQLQuery;
use leptos::{error::Error, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/upsertS3Object.graphql",
	extern_enums("PublicityOverride"),
	response_derives = "Clone,Debug"
)]
pub struct UpsertS3ObjectMutation;

pub use crate::graphql_queries::types::PublicityOverride;

impl UpsertS3ObjectMutation {
	// @todo Add better error-handling
	pub async fn run(variables: Variables) -> Result<S3Object, Error> {
		let config = use_context::<AppConfig>().expect("AppConfig missing");
		Ok(post_graphql_with_auth::<UpsertS3ObjectMutation, _>(
			&reqwest::Client::new(),
			config.api_url,
			variables,
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.upsert_s3_object)?)
	}
}
