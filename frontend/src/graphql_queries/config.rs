use {
	crate::graphql_queries::{
		GraphqlOp,
		config::config_query::ConfigQueryConfig as PublicConfig,
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

impl GraphqlOp for ConfigQuery {
	type Output = PublicConfig;

	fn extract(data: config_query::ResponseData) -> Self::Output {
		data.config
	}
}
