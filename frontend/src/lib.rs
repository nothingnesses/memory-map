use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{components::*, path};
use graphql_client::GraphQLQuery;

// Modules
mod components;
mod pages;

// Top-Level pages
use crate::pages::home::Home;

/// An app router which renders the homepage and handles 404's
#[component]
pub fn App() -> impl IntoView {
	// Provides context that manages stylesheets, titles, meta tags, etc.
	provide_meta_context();

	view! {
		<Html attr:lang="en" attr:dir="ltr" attr:data-theme="light" />

		// sets the document title
		<Title text="Welcome to Leptos CSR" />

		// injects metadata in the <head> of the page
		<Meta charset="UTF-8" />
		<Meta name="viewport" content="width=device-width, initial-scale=1.0" />

		<Router>
			<Routes fallback=|| view! { NotFound }>
				<Route path=path!("/") view=Home />
			</Routes>
		</Router>
	}
}

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/s3Objects.graphql",
	response_derives = "Debug"
)]
pub struct S3ObjectsQuery;

/// [Copied from here](https://docs.rs/graphql_client/0.14.0/src/graphql_client/reqwest.rs.html#8-17),
/// since we can't initialise a `reqwest::Client` to use with the original version,
/// since `graphql_client` didn't `pub use` their version of `reqwest` for us to `use`.
pub async fn post_graphql<Q: graphql_client::GraphQLQuery, U: reqwest::IntoUrl>(
	client: &reqwest::Client,
	url: U,
	variables: Q::Variables,
) -> Result<graphql_client::Response<Q::ResponseData>, reqwest::Error> {
	let body = Q::build_query(variables);
	let reqwest_response = client.post(url).json(&body).send().await?;

	reqwest_response.json().await
}
