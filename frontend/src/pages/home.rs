use crate::{components::counter_btn::Button as CounterButton, post_graphql};
use graphql_client::GraphQLQuery;
use leptos::{
	logging::{debug_error, debug_log},
	prelude::*,
	task::spawn_local,
};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/s3Objects.graphql",
	response_derives = "Debug"
)]
pub struct S3ObjectsQuery;

/// Default Home Page
#[component]
pub fn Home() -> impl IntoView {
	view! {
		<ErrorBoundary fallback=|errors| {
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
		}>

			<div class="container">

				<picture>
					<source
						srcset="https://raw.githubusercontent.com/leptos-rs/leptos/main/docs/logos/Leptos_logo_pref_dark_RGB.svg"
						media="(prefers-color-scheme: dark)"
					/>
					<img
						src="https://raw.githubusercontent.com/leptos-rs/leptos/main/docs/logos/Leptos_logo_RGB.svg"
						alt="Leptos Logo"
						height="200"
						width="400"
					/>
				</picture>

				<h1>"Welcome to Leptos"</h1>

				<div class="buttons">
					<CounterButton />
					<CounterButton increment=5 />

					<button on:click=move |_| {
						spawn_local(async move {
							let response = post_graphql::<S3ObjectsQuery, _>(&reqwest::Client::new(), "http://localhost:8000/", s3_objects_query::Variables {}).await;
							match response {
								Ok(response) => debug_log!("{:?}", response),
								Err(error) => debug_error!("{:?}", error),
							}
						});
					}>
						"Make GraphQL Request"
					</button>
				</div>

			</div>
		</ErrorBoundary>
	}
}
