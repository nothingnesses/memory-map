use {
	crate::graphql_queries::me::me_query::MeQueryMe,
	leptos::prelude::*,
};

#[derive(Clone, Copy)]
pub struct UserContext {
	pub user: LocalResource<Option<MeQueryMe>>,
	pub refetch: Callback<()>,
}
