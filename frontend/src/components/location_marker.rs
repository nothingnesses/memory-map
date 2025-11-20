use crate::s3_objects_query::S3ObjectsQueryS3Objects as S3Object;
use leptos::prelude::*;
use leptos_leaflet::prelude::*;

fn render_s3_objects(s3_objects: Vec<S3Object>) -> impl IntoView {
	s3_objects
		.iter()
		.map(|s3_object| {
			view! { <div>{s3_object.id.clone()}</div> }
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
					{move || render_s3_objects(s3_objects.get().clone())}
					{move || format!("{latitude}, {longitude}")}
				</div>
			</Popup>
		</Marker>
	}
}
