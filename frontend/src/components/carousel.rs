use crate::{render_s3_objects, s3_objects_query::S3ObjectsQueryS3Objects as S3Object};
use leptos::prelude::*;

#[component]
pub fn Carousel(#[prop(into)] s3_objects: Signal<Vec<S3Object>>) -> impl IntoView {
	view! { {move || render_s3_objects(s3_objects.get().clone())} }
}
