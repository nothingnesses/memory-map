use crate::{
	graphql_queries::me::me_query::{MeQueryMe as User, Variables},
	post_graphql_with_auth, AppConfig,
};
use graphql_client::GraphQLQuery;
use leptos::{error::Error, prelude::*};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/me.graphql",
	extern_enums("PublicityDefault", "UserRole"),
	response_derives = "Clone,Debug,PartialEq"
)]
pub struct MeQuery;

pub use crate::graphql_queries::types::{PublicityDefault, UserRole};

impl MeQuery {
	pub async fn run() -> Result<Option<User>, Error> {
		let config = use_context::<AppConfig>().expect("AppConfig missing");
		Ok(post_graphql_with_auth::<MeQuery, _>(
			&reqwest::Client::new(),
			config.api_url,
			Variables {},
		)
		.await?
		.data
		.ok_or("Empty response".to_string())
		.map(|response| response.me)?)
	}
}
