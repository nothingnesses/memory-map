use crate::pages::{admin::Admin, edit_s3_object::EditS3Object, home::Home};
use leptos::{prelude::*, wasm_bindgen::JsValue, web_sys::js_sys};
use leptos_meta::*;
use leptos_router::{components::*, path};
use std::ops::{Add, Rem, Sub};
use thaw::{ConfigProvider, ToasterProvider};

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
		<ConfigProvider>
			<ToasterProvider>
				<Html attr:lang="en" attr:dir="ltr" attr:data-theme="light" />

				// sets the document title
				<Title text="Memory Map" />

				// injects metadata in the <head> of the page
				<Meta charset="UTF-8" />
				<Meta name="viewport" content="width=device-width, initial-scale=1.0" />

				<Router>
					<header>
						<nav class="relative container mx-auto grid gap-4 grid-flow-col justify-start py-4">
							<A href="/">"Map"</A>
							<A href="/admin">"Admin"</A>
						</nav>
					</header>
					<main>
						<Routes fallback=|| view! { NotFound }>
							<Route path=path!("/") view=Home />
							<Route path=path!("/admin") view=Admin />
							<Route path=path!("/admin/s3-objects/:id/edit") view=EditS3Object />
						</Routes>
					</main>
				</Router>
			</ToasterProvider>
		</ConfigProvider>
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

/// Converts a date string from an HTML input (e.g. "2023-12-26T14:30")
/// to an ISO 8601 UTC string (e.g. "2023-12-26T14:30:00.000Z").
pub fn js_date_value_to_iso(value: &str) -> Option<String> {
	if value.is_empty() {
		return None;
	}
	let date = js_sys::Date::new(&JsValue::from_str(value));
	// Check for invalid date (NaN timestamp) to prevent panic in to_iso_string
	if date.get_time().is_nan() {
		return None;
	}
	date.to_iso_string().as_string()
}

/// Converts an ISO 8601 UTC string (e.g. "2023-12-26T14:30:00.000Z")
/// to a local datetime string suitable for an HTML input (e.g. "2023-12-26T14:30").
pub fn iso_to_local_datetime_value(iso: &str) -> Option<String> {
	let date = js_sys::Date::new(&JsValue::from_str(iso));
	if date.get_time().is_nan() {
		return None;
	}
	let offset = date.get_timezone_offset() * 60000.0;
	let local_date = js_sys::Date::new(&JsValue::from_f64(date.get_time() - offset));
	local_date.to_iso_string().as_string().map(|s| s.chars().take(16).collect())
}
