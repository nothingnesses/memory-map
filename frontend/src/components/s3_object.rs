use crate::graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects;
use leptos::{either::either, prelude::*};
use mime::Mime;

#[component]
pub fn S3Object(
	#[prop(into)] s3_object: Signal<S3ObjectsQueryS3Objects>,
	#[prop(optional, into)] class: MaybeProp<String>,
) -> impl IntoView {
	either!(
		s3_object
			.get()
			.content_type
			.parse::<Mime>()
			.map(|m| m.type_().as_str().to_string())
			.unwrap_or_default().as_str(),
		"image" => view! {
			<img class=move || class.get() src=move || s3_object.get().url />
		},
		"video" => view! {
			<video class=move || class.get() src=move || s3_object.get().url controls autoplay />
		},
		_ => (),
	)
}
