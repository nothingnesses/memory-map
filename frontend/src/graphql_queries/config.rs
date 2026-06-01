use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		graphql_queries::config::config_query::{
			ConfigQueryConfig as PublicConfig,
			Variables,
		},
		post_graphql,
	},
	graphql_client::GraphQLQuery,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/config.graphql",
	response_derives = "Clone,Debug,PartialEq"
)]
pub struct ConfigQuery;

impl ConfigQuery {
	pub async fn run(api_url: String) -> Result<PublicConfig, AppError> {
		let response =
			post_graphql::<ConfigQuery, _>(&reqwest::Client::new(), api_url, Variables {}).await?;
		Ok(graphql_data(response)?.config)
	}
}
