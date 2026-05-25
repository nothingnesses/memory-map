use {
	crate::{
		graphql_queries::me::me_query::{
			MeQueryMe as User,
			Variables,
		},
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
	leptos::error::Error,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/me.graphql",
	extern_enums("PublicityDefault", "UserRole"),
	response_derives = "Clone,Debug,PartialEq"
)]
pub struct MeQuery;

pub use crate::graphql_queries::types::{
	PublicityDefault,
	UserRole,
};

impl MeQuery {
	pub async fn run(api_url: String) -> Result<Option<User>, Error> {
		Ok(post_graphql_with_auth::<MeQuery, _>(&reqwest::Client::new(), api_url, Variables {})
			.await?
			.data
			.ok_or("Empty response".to_string())
			.map(|response| response.me)?)
	}
}
