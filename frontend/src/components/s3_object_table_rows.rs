use crate::graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object;
use leptos::{logging::debug_log, prelude::*};
use leptos_router::components::*;
use thaw::*;

#[component]
pub fn S3ObjectTableRows(#[prop(into)] s3_objects: Signal<Vec<S3Object>>) -> impl IntoView {
	view! {
		<ForEnumerate
			each=move || s3_objects.get()
			key=|s3_object| s3_object.id.clone()
			let(s3_object_index,
			s3_object)
		>
			<TableRow>
				<TableCell>{s3_object.id}</TableCell>
				<TableCell>{s3_object.name}</TableCell>
				<TableCell>{s3_object.made_on}</TableCell>
				<TableCell>
					{s3_object
						.location
						.map(|location| format!("{}, {}", location.latitude, location.longitude))}
				</TableCell>
				<TableCell>
					<A href=s3_object.url>"Click"</A>
				</TableCell>
				<TableCell>{s3_object.content_type}</TableCell>
				<TableCell>
					<Button on_click=move |_| {
						debug_log!("{}", s3_object_index.get())
					}>"Click"</Button>
				</TableCell>
			</TableRow>
		</ForEnumerate>
	}
}
