use crate::graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object;
use leptos::prelude::*;
use leptos_router::components::*;
use std::collections::HashSet;
use thaw::*;

#[component]
pub fn S3ObjectTableRows(
	#[prop(into)] s3_objects: Signal<Vec<S3Object>>,
	#[prop(into)] selected_ids: Signal<HashSet<String>>,
	#[prop(into)] on_toggle: Callback<String>,
	#[prop(into)] on_delete: Callback<S3Object>,
) -> impl IntoView {
	view! {
		<ForEnumerate
			each=move || s3_objects.get()
			key=|s3_object| s3_object.id.clone()
			let(_s3_object_index,
			s3_object)
		>
			{
				let s3_object_for_checkbox = s3_object.clone();
				let s3_object_for_toggle = s3_object.clone();
				let s3_object_for_delete = s3_object.clone();
				view! {
					<TableRow>
						<TableCell class="wrap-anywhere">
							<input
								type="checkbox"
								checked=move || {
									selected_ids.get().contains(&s3_object_for_checkbox.id)
								}
								on:change=move |_| on_toggle.run(s3_object_for_toggle.id.clone())
							/>
						</TableCell>
						<TableCell class="wrap-anywhere">{s3_object.id.clone()}</TableCell>
						<TableCell class="wrap-anywhere">{s3_object.name.clone()}</TableCell>
						<TableCell class="wrap-anywhere">{s3_object.made_on.clone()}</TableCell>
						<TableCell class="wrap-anywhere">
							{s3_object
								.location
								.clone()
								.map(|location| {
									format!("{}, {}", location.latitude, location.longitude)
								})}
						</TableCell>
						<TableCell class="wrap-anywhere">
							<A href=s3_object.url.clone()>"Click me"</A>
						</TableCell>
						<TableCell class="wrap-anywhere">{s3_object.content_type.clone()}</TableCell>
						<TableCell class="wrap-anywhere">
							<div>
								<Button on_click=move |_| {
									on_delete.run(s3_object_for_delete.clone())
								}>"Delete"</Button>
							</div>
						</TableCell>
					</TableRow>
				}
			}
		</ForEnumerate>
	}
}
