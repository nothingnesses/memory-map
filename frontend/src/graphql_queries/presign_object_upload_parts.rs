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
	query_path = "graphql/presignObjectUploadParts.graphql",
	response_derives = "Clone,Debug"
)]
pub struct PresignObjectUploadPartsMutation;

use self::presign_object_upload_parts_mutation::{
	PresignObjectUploadPartsMutationPresignObjectUploadParts as PresignedObjectUploadPart,
	Variables,
};

impl PresignObjectUploadPartsMutation {
	pub async fn run(
		api_url: String,
		object_id: String,
		part_numbers: Vec<i64>,
	) -> Result<Vec<PresignedObjectUploadPart>, AppError> {
		let response = post_graphql_with_auth::<PresignObjectUploadPartsMutation, _>(
			&reqwest::Client::new(),
			api_url,
			Variables {
				object_id,
				part_numbers,
			},
		)
		.await?;
		Ok(graphql_data(response)?.presign_object_upload_parts)
	}
}

pub type PresignedUploadPart = PresignedObjectUploadPart;
