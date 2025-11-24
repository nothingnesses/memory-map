use crate::{components::gallery::Gallery, s3_objects_query::S3ObjectsQueryS3Objects as S3Object};
use leptos::{either::either, prelude::*};
use leptos_leaflet::prelude::*;
use mime::Mime;

fn render_s3_objects(s3_objects: Vec<S3Object>) -> impl IntoView {
	s3_objects
		.into_iter()
		.map(|s3_object| {
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
				_ => view! { },
			)
		})
		.collect_view()
}

#[component]
pub fn LocationMarker(
	#[prop(into)] latitude: Signal<f64>,
	#[prop(into)] longitude: Signal<f64>,
	#[prop(into)] s3_objects: Signal<Vec<S3Object>>,
) -> impl IntoView {
	let (latitude, longitude) = (latitude.get(), longitude.get());
	view! {
		<Marker position=position!(latitude, longitude) draggable=true>
			<Popup>
				<div class="grid">
					{move || view! { <Gallery></Gallery> }}
					{move || render_s3_objects(s3_objects.get().clone())}
					{move || format!("{latitude}, {longitude}")}
				</div>
			</Popup>
		</Marker>
	}
}
