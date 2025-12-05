use crate::{
	pages::home::Home,
	s3_objects_query::{S3ObjectsQueryS3Objects as S3Object, Variables},
};
use graphql_client::GraphQLQuery;
use leptos::{either::either, prelude::*};
use leptos_meta::*;
use leptos_router::{components::*, path};
use mime::Mime;
use std::ops::{Add, Rem, Sub};

// Modules
mod components;
mod pages;

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
	response_derives = "Clone,Debug"
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

pub async fn fetch_s3_objects() -> Result<Vec<S3Object>, Error> {
	Ok(post_graphql::<S3ObjectsQuery, _>(
		&reqwest::Client::new(),
		"http://localhost:8000/",
		Variables {},
	)
	.await?
	.data
	.ok_or("Empty response".to_string())
	.map(|response| response.s3_objects)?)
}

pub fn render_s3_object(s3_object: S3Object) -> impl IntoView {
	let mime_type = s3_object
		.content_type
		.parse::<Mime>()
		.map(|m| m.type_().as_str().to_string())
		.unwrap_or_default();
	either!(
		mime_type.as_str(),
		"image" => view! {
			<img src=s3_object.url />
		},
		"video" => view! {
			<video src=s3_object.url controls=true />
		},
		_ => (),
	)
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
