use crate::{AppConfig, post_graphql_with_auth};
use graphql_client::GraphQLQuery;
use leptos::{error::Error, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/deleteS3Objects.graphql",
	response_derives = "Clone,Debug"
)]
pub struct DeleteS3ObjectsMutation;

use self::delete_s3_objects_mutation::{
	DeleteS3ObjectsMutationDeleteS3Objects as DeletedS3Object, Variables,
};

impl DeleteS3ObjectsMutation {
	pub async fn run(ids: Vec<String>) -> Result<Vec<DeletedS3Object>, Error> {
		let config = use_context::<AppConfig>().expect("AppConfig missing");
		Ok(post_graphql_with_auth::<DeleteS3ObjectsMutation, _>(
			&reqwest::Client::new(),
			config.api_url,
			Variables { ids },
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.delete_s3_objects)?)
	}
}
