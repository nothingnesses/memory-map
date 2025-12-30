use crate::{
	LocationStrings,
	components::location_marker::LocationMarker,
	dump_errors,
	graphql_queries::s3_objects::{
		S3ObjectsQuery, s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
	},
};
use leptos::{logging::debug_error, prelude::*};
use std::collections::HashMap;

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
				// This should just work, since the location strings were serialised from actual f64 values.
				match (
					location_strings.latitude.parse::<f64>(),
					location_strings.longitude.parse::<f64>(),
				) {
					(Ok(latitude), Ok(longitude)) => {
						Some(view! { <LocationMarker latitude longitude s3_objects /> })
					}
					_ => None,
				}
			})
		})
		.collect_view()
}

/// Location markers to add to the map.
#[component]
pub fn LocationMarkers() -> impl IntoView {
	let s3_objects_resource = LocalResource::new(S3ObjectsQuery::run);
	view! {
		<ErrorBoundary fallback=|errors| {
			debug_error!("Failed to load markers: {:?}", errors.get());
			dump_errors(errors)
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
