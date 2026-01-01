use crate::{
	graphql_queries::config::config_query::{ConfigQueryConfig as PublicConfig, Variables},
	post_graphql,
};
use graphql_client::GraphQLQuery;
use leptos::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/config.graphql",
	response_derives = "Clone,Debug,PartialEq"
)]
pub struct ConfigQuery;

impl ConfigQuery {
	pub async fn run(api_url: String) -> Result<PublicConfig, Error> {
		Ok(post_graphql::<ConfigQuery, _>(&reqwest::Client::new(), api_url, Variables {})
			.await?
			.data
			.ok_or("Empty response".to_string())
			.map(|response| response.config)?)
	}
}
