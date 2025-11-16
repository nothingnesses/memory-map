use crate::components::counter_btn::Button as CounterButton;
use graphql_client::{GraphQLQuery, Response};
use leptos::{logging::debug_log, prelude::*, task::spawn_local};

#[derive(GraphQLQuery)]
#[graphql(
	schema_path = "graphql/schema.json",
	query_path = "graphql/s3Objects.graphql",
	response_derives = "Debug"
)]
pub struct S3ObjectsQuery;

// https://github.com/leptos-rs/leptos/discussions/198#discussioncomment-4582094
async fn s3_objects_query_request(
	variables: s3_objects_query::Variables
) -> Result<String, String> {
	let request_body = S3ObjectsQuery::build_query(variables);
	let client = reqwest::Client::new();
	let res = client
		.post("http://localhost:8000/")
		.json(&request_body)
		.send()
		.await
		.map_err(|e| e.to_string())?;
	let response_body: Response<s3_objects_query::ResponseData> =
		res.json().await.map_err(|e| e.to_string())?;
	Ok(format!("{:?}", response_body))
}

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
							let res = s3_objects_query_request(s3_objects_query::Variables {}).await;
							debug_log!("{:?}", res)
						});
					}>
						"Make GraphQL Request"
					</button>
				</div>

			</div>
		</ErrorBoundary>
	}
}
