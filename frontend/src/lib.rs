use crate::pages::{admin::Admin, home::Home};
use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{components::*, path};
use std::ops::{Add, Rem, Sub};

// Modules
mod components;
pub mod graphql_queries;
mod pages;

/// An app router which renders the homepage and handles 404's
#[component]
pub fn App() -> impl IntoView {
	// Provides context that manages stylesheets, titles, meta tags, etc.
	provide_meta_context();

	view! {
		<Html attr:lang="en" attr:dir="ltr" attr:data-theme="light" />

		// sets the document title
		<Title text="Memory Map" />

		// injects metadata in the <head> of the page
		<Meta charset="UTF-8" />
		<Meta name="viewport" content="width=device-width, initial-scale=1.0" />

		<Router>
			<header>
				<nav class="relative container mx-auto grid gap-4 grid-flow-col justify-start">
					<A href="/">"Map"</A>
					<A href="/admin">"Admin"</A>
				</nav>
			</header>
			<main>
				<Routes fallback=|| view! { NotFound }>
					<Route path=path!("/") view=Home />
					<Route path=path!("/admin") view=Admin />
				</Routes>
			</main>
		</Router>
	}
}

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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LocationStrings {
	pub latitude: String,
	pub longitude: String,
}

pub fn dump_errors(errors: ArcRwSignal<Errors>) -> impl IntoView {
	view! {
		<h1>"Uh oh! Something went wrong!"</h1>

		<p>"Errors: "</p>
		// Render a list of errors as strings - good for development purposes
		<ul>
			{move || {
				errors
					.get()
					.into_iter()
					.map(|(_, e)| view! { <li>{e.to_string()}</li> })
					.collect_view()
			}}

		</ul>
	}
}

pub trait ModularAdd {
	fn modular_add(
		self,
		addend: Self,
		maximum: Self,
	) -> Self;
}

impl<T> ModularAdd for T
where
	T: Copy + PartialOrd + Add<Output = T> + Rem<Output = T>,
{
	fn modular_add(
		self,
		addend: Self,
		maximum: Self,
	) -> Self {
		(self + addend) % maximum
	}
}

pub trait ModularSubtract {
	fn modular_subtract(
		self,
		subtrahend: Self,
		maximum: Self,
	) -> Self;
}

impl<T> ModularSubtract for T
where
	T: Copy + PartialOrd + Sub<Output = T> + Rem<Output = T>,
{
	fn modular_subtract(
		self,
		subtrahend: Self,
		maximum: Self,
	) -> Self {
		let effective_subtrahend = subtrahend % maximum;

		if effective_subtrahend > self {
			maximum - (effective_subtrahend - self)
		} else {
			self - effective_subtrahend
		}
	}
}

pub type CallbackAnyView = Callback<(), AnyView>;
