use crate::{
	LocationStrings, S3ObjectsQuery,
	components::location_marker::LocationMarker,
	post_graphql,
	s3_objects_query::{S3ObjectsQueryS3Objects as S3Object, Variables},
};
use leptos::{logging::debug_error, prelude::*};
use std::collections::HashMap;

async fn fetch_s3_objects() -> Result<Vec<S3Object>, Error> {
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

fn render_markers(s3_objects: Vec<S3Object>) -> impl IntoView {
	s3_objects
		.into_iter()
		.map(|s3_object: S3Object| {
			(
				s3_object.location.as_ref().map(|location| LocationStrings {
					latitude: location.latitude.to_string(),
					longitude: location.longitude.to_string(),
				}),
				s3_object,
			)
		})
		.fold(HashMap::<Option<LocationStrings>, Vec<S3Object>>::new(), |mut carry, (key, item)| {
			carry.entry(key).or_default().push(item);
			carry
		})
		.into_iter()
		.map(|(location, s3_objects)| {
			location.as_ref().map(|location_strings| {
				match (
					location_strings.latitude.parse::<f64>(),
					location_strings.longitude.parse::<f64>(),
				) {
					(Ok(latitude), Ok(longitude)) => Some(
						view! { <LocationMarker latitude=latitude longitude=longitude s3_objects=s3_objects /> },
					),
					_ => None,
				}
			})
		})
		.collect_view()
}

/// Location markers to add to the map.
#[component]
pub fn LocationMarkers() -> impl IntoView {
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
						.map(|data| { Ok::<_, Error>(view! { {render_markers(data?)} }) })
				}}
			</Suspense>
		</ErrorBoundary>
	}
}
