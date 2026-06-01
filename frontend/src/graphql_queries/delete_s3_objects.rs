use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/deleteS3Objects.graphql",
	response_derives = "Clone,Debug"
)]
pub struct DeleteS3ObjectsMutation;

use self::delete_s3_objects_mutation::{
	DeleteS3ObjectsMutationDeleteS3Objects as DeletedS3Object,
	Variables,
};

impl DeleteS3ObjectsMutation {
	pub async fn run(
		api_url: String,
		ids: Vec<String>,
	) -> Result<Vec<DeletedS3Object>, AppError> {
		let response = post_graphql_with_auth::<DeleteS3ObjectsMutation, _>(
			&reqwest::Client::new(),
			api_url,
			Variables {
				ids,
			},
		)
		.await?;
		Ok(graphql_data(response)?.delete_s3_objects)
	}
}
