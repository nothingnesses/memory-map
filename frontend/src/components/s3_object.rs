use crate::s3_objects_query::S3ObjectsQueryS3Objects;
use leptos::{either::either, prelude::*};
use mime::Mime;

#[component]
pub fn S3Object(#[prop(into)] s3_object: Signal<S3ObjectsQueryS3Objects>) -> impl IntoView {
	either!(
		s3_object
			.get()
			.content_type
			.parse::<Mime>()
			.map(|m| m.type_().as_str().to_string())
			.unwrap_or_default().as_str(),
		"image" => view! {
			<img src=move || s3_object.get().url />
		},
		"video" => view! {
			<video src=move || s3_object.get().url controls=true />
		},
		_ => (),
	)
}
