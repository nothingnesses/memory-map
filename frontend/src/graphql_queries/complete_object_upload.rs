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
	query_path = "graphql/completeObjectUpload.graphql",
	extern_enums("PublicityOverride"),
	response_derives = "Clone,Debug"
)]
pub struct CompleteObjectUploadMutation;

use self::complete_object_upload_mutation::{
	CompleteObjectUploadMutationCompleteObjectUpload as CompletedObjectUpload,
	CompletedObjectUploadPartInput,
	Variables,
};
pub use crate::graphql_queries::types::PublicityOverride;

impl CompleteObjectUploadMutation {
	pub async fn run(
		api_url: String,
		object_id: String,
		parts: Vec<CompletedObjectUploadPartInput>,
	) -> Result<CompletedObjectUpload, AppError> {
		let response = post_graphql_with_auth::<CompleteObjectUploadMutation, _>(
			&reqwest::Client::new(),
			api_url,
			Variables {
				object_id,
				parts,
			},
		)
		.await?;
		Ok(graphql_data(response)?.complete_object_upload)
	}
}

pub type CompletedUploadPartInput = CompletedObjectUploadPartInput;
