use crate::{
	graphql_queries::s3_object_by_id::s3_object_by_id_query::{
		S3ObjectByIdQueryS3ObjectById as S3Object, Variables,
	},
	post_graphql_with_auth,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/s3ObjectById.graphql",
	extern_enums("PublicityOverride"),
	response_derives = "Clone,Debug,Serialize,Deserialize"
)]
pub struct S3ObjectByIdQuery;

pub use crate::graphql_queries::types::PublicityOverride;

impl S3ObjectByIdQuery {
	// @todo Add better error-handling
	pub async fn run(
		api_url: String,
		id: i64,
	) -> Result<S3Object, Error> {
		Ok(post_graphql_with_auth::<S3ObjectByIdQuery, _>(
			&reqwest::Client::new(),
			api_url,
			Variables { id },
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.s3_object_by_id)?)
	}
}
