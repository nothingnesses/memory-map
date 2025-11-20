use crate::{LocationStrings, S3ObjectsQuery, dump_errors, post_graphql, s3_objects_query};
use leptos::{logging::debug_error, prelude::*};
use leptos_leaflet::prelude::*;
use std::collections;

async fn fetch_s3_objects() -> Result<Vec<s3_objects_query::S3ObjectsQueryS3Objects>, Error> {
	Ok(post_graphql::<S3ObjectsQuery, _>(
		&reqwest::Client::new(),
		"http://localhost:8000/",
		s3_objects_query::Variables {},
	)
	.await?
	.data
	.ok_or("Empty response".to_string())
	.map(|response| response.s3_objects)?)
}

fn render_s3_objects(s3_objects: Vec<s3_objects_query::S3ObjectsQueryS3Objects>) -> impl IntoView {
	s3_objects
		.iter()
		.map(|s3_object: &s3_objects_query::S3ObjectsQueryS3Objects| {
			(
				s3_object.location.as_ref().map(|location| LocationStrings {
					latitude: location.latitude.to_string(),
					longitude: location.longitude.to_string(),
				}),
				s3_object,
			)
		})
		.fold(
			collections::HashMap::<
				Option<LocationStrings>,
				Vec<&s3_objects_query::S3ObjectsQueryS3Objects>,
			>::new(),
			|mut carry, (key, item)| {
				carry.entry(key).or_default().push(item);
				carry
			},
		)
		.iter()
		.map(|(location, _s3_objects)| {
			location.as_ref().map(|location_strings| {
				match (
					location_strings.latitude.parse::<f64>(),
					location_strings.longitude.parse::<f64>(),
				) {
					(Ok(latitude), Ok(longitude)) => Some(view! {
						<Marker position=position!(latitude, longitude) draggable=true>
							<Popup>
								<strong>{"Found Objects Here"}</strong>
							</Popup>
						</Marker>
					}),
					_ => None,
				}
			})
		})
		.collect_view()
}

/// Location markers to add to the map.
#[component]
fn LocationMarkers() -> impl IntoView {
	let s3_objects_resource = LocalResource::new(move || fetch_s3_objects());
	view! {
		<ErrorBoundary fallback=|errors| {
			debug_error!("Failed to load markers: {:?}", errors.get());
			view! {}
		}>
			<Suspense fallback=move || {
				view! { <p>"Loading map data..."</p> }
			}>
				{move || {
					s3_objects_resource
						.get()
						.map(|data| { Ok::<_, Error>(view! { {render_s3_objects(data?)} }) })
				}}
			</Suspense>
		</ErrorBoundary>
	}
}

/// Default Home Page
#[component]
pub fn Home() -> impl IntoView {
	view! {
		<ErrorBoundary fallback=dump_errors>
			<div class="container">
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
					<LocationMarkers />
				</MapContainer>
			</div>
		</ErrorBoundary>
	}
}
