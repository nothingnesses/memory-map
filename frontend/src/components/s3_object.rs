use {
	crate::graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects,
	leptos::{
		either::either,
		prelude::*,
	},
	mime::Mime,
};

#[component]
pub fn S3Object(
	#[prop(into)] s3_object: Signal<Option<S3ObjectsQueryS3Objects>>,
	#[prop(optional, into)] class: MaybeProp<String>,
) -> impl IntoView {
	move || {
		if let Some(s3_object) = s3_object.get() {
			either!(
				s3_object
					.content_type
					.parse::<Mime>()
					.map(|m| m.type_().as_str().to_string())
					.unwrap_or_default()
					.as_str(),
				"image" => view! { <img class=move || class.get() src=move || s3_object.url.clone() /> },
				"video" | "audio" => view! {
					<video
						class=move || class.get()
						src=move || s3_object.url.clone()
						controls
						autoplay
					/>
				},
				_ => (),
			)
			.into_any()
		} else {
			().into_any()
		}
	}
}
