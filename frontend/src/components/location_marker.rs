use crate::{
	components::gallery::Gallery,
	graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
};
use leptos::prelude::*;
use leptos_leaflet::prelude::*;

#[component]
pub fn LocationMarker(
	#[prop(into)] latitude: Signal<f64>,
	#[prop(into)] longitude: Signal<f64>,
	#[prop(into)] s3_objects: Signal<Vec<S3Object>>,
) -> impl IntoView {
	view! {
		<Marker position=position!(latitude.get(), longitude.get())>
			<Popup>
				<div class="grid gap-4">
					<h2>{latitude}," "{longitude}</h2>
					<Gallery
						s3_objects
						dialog_title_content=Callback::new(move |_| {
							view! { { latitude },
								" "
								{longitude}
							}
								.into_any()
						})
					></Gallery>
				</div>
			</Popup>
		</Marker>
	}
}
