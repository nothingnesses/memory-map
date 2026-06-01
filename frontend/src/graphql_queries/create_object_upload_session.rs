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
	query_path = "graphql/createObjectUploadSession.graphql",
	extern_enums("PublicityOverride"),
	response_derives = "Clone,Debug"
)]
pub struct CreateObjectUploadSessionMutation;

use self::create_object_upload_session_mutation::{
	CreateObjectUploadSessionInput,
	CreateObjectUploadSessionMutationCreateObjectUploadSession as CreatedObjectUploadSession,
	LocationInput,
	Variables,
};
pub use crate::graphql_queries::types::PublicityOverride;

impl CreateObjectUploadSessionMutation {
	pub async fn run(
		api_url: String,
		input: CreateObjectUploadSessionInput,
	) -> Result<CreatedObjectUploadSession, AppError> {
		let response = post_graphql_with_auth::<CreateObjectUploadSessionMutation, _>(
			&reqwest::Client::new(),
			api_url,
			Variables {
				input,
			},
		)
		.await?;
		Ok(graphql_data(response)?.create_object_upload_session)
	}
}

pub type CreateObjectUploadSessionInputVariables = CreateObjectUploadSessionInput;
pub type UploadLocationInput = LocationInput;
