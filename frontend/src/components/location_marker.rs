use crate::{components::gallery::Gallery, s3_objects_query::S3ObjectsQueryS3Objects as S3Object};
use leptos::prelude::*;
use leptos_leaflet::prelude::*;

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
					{move || view! { <Gallery s3_objects></Gallery> }}
					{move || format!("{latitude}, {longitude}")}
				</div>
			</Popup>
		</Marker>
	}
}
