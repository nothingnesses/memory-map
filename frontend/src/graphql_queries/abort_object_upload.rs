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
	query_path = "graphql/abortObjectUpload.graphql",
	response_derives = "Clone,Debug"
)]
pub struct AbortObjectUploadMutation;

use self::abort_object_upload_mutation::{
	AbortObjectUploadMutationAbortObjectUpload as AbortedObjectUpload,
	Variables,
};

impl AbortObjectUploadMutation {
	pub async fn run(
		api_url: String,
		object_id: String,
	) -> Result<AbortedObjectUpload, AppError> {
		let response = post_graphql_with_auth::<AbortObjectUploadMutation, _>(
			&reqwest::Client::new(),
			api_url,
			Variables {
				object_id,
			},
		)
		.await?;
		Ok(graphql_data(response)?.abort_object_upload)
	}
}
