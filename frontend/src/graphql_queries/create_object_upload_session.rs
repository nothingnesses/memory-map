use {
	crate::graphql_queries::GraphqlOp,
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

pub use {
	self::create_object_upload_session_mutation::{
		CreateObjectUploadSessionInput,
		LocationInput as UploadLocationInput,
	},
	crate::graphql_queries::types::PublicityOverride,
};

use self::create_object_upload_session_mutation::CreateObjectUploadSessionMutationCreateObjectUploadSession as CreatedObjectUploadSession;

impl GraphqlOp for CreateObjectUploadSessionMutation {
	type Output = CreatedObjectUploadSession;

	fn extract(data: create_object_upload_session_mutation::ResponseData) -> Self::Output {
		data.create_object_upload_session
	}
}
