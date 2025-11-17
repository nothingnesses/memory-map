use crate::{
	S3ObjectsQuery, components::counter_btn::Button as CounterButton, post_graphql,
	s3_objects_query,
};
use leptos::{
	logging::{debug_error, debug_log},
	prelude::*,
	task::spawn_local,
};
use leptos_leaflet::prelude::*;
use std::time;

/// Default Home Page
#[component]
pub fn Home() -> impl IntoView {
	let dump_errors = |errors: ArcRwSignal<Errors>| {
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
	};

	let make_graphql_request = move |_| {
		spawn_local(async move {
			let response = post_graphql::<S3ObjectsQuery, _>(
				&reqwest::Client::new(),
				"http://localhost:8000/",
				s3_objects_query::Variables {},
			)
			.await;
			match response {
				Ok(response) => debug_log!("{:?}", response),
				Err(error) => debug_error!("{:?}", error),
			}
		});
	};

	let (marker_position, set_marker_position) =
		JsRwSignal::new_local(Position::new(51.49, -0.08))
			.split();

	Effect::new(move |_| {
		set_interval_with_handle(
			move || {
				set_marker_position.update(|pos| {
					pos.lat += 0.001;
					pos.lng += 0.001;
				});
			},
			time::Duration::from_millis(200),
		)
		.ok()
	});

	view! {
		<ErrorBoundary fallback=dump_errors>

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

					<button on:click=make_graphql_request>"Make GraphQL Request"</button>
				</div>

				<MapContainer
					class="w-full"
					style="height: 400px"
					center=Position::new(51.505, -0.09)
					zoom=13.0
					set_view=true
				>
					<TileLayer
						url="https://tile.openstreetmap.org/{z}/{x}/{y}.png"
						attribution="&copy; <a href=\"https://www.openstreetmap.org/copyright\">OpenStreetMap</a> contributors"
					/>
					<Marker position=marker_position>
						<Popup>
							<strong>{"A pretty CSS3 popup"}</strong>
						</Popup>
					</Marker>
					<Marker position=position!(51.5, -0.065) draggable=true>
						<Popup>
							<strong>{"A pretty CSS3 popup"}</strong>
						</Popup>
					</Marker>
					<Tooltip position=position!(51.5, -0.06) permanent=true direction="top">
						<strong>{"And a tooltip"}</strong>
					</Tooltip>
					<Polyline positions=positions(
						&[(51.505, -0.09), (51.51, -0.1), (51.51, -0.12)],
					) />
					<Polygon
						color="purple"
						positions=positions(&[(51.515, -0.09), (51.52, -0.1), (51.52, -0.12)])
					>
						<Tooltip sticky=true direction="top">
							<strong>{"I'm a polygon"}</strong>
						</Tooltip>
					</Polygon>
					<Circle center=position!(51.505, -0.09) color="blue" radius=200.0>
						<Tooltip sticky=true>{"I'm a circle"}</Tooltip>
					</Circle>
				</MapContainer>

			</div>
		</ErrorBoundary>
	}
}
