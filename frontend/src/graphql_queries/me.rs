use {
	crate::graphql_queries::{
		GraphqlOp,
		me::me_query::MeQueryMe as User,
	},
	graphql_client::GraphQLQuery,
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

impl GraphqlOp for MeQuery {
	type Output = Option<User>;

	fn extract(data: me_query::ResponseData) -> Self::Output {
		data.me
	}
}
