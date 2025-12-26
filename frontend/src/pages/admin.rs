use crate::components::file_upload::FileUpload;
use crate::components::s3_objects_table::S3ObjectsTable;
use crate::{dump_errors, graphql_queries::s3_objects::S3ObjectsQuery};
use leptos::prelude::*;

#[component]
pub fn Admin() -> impl IntoView {
	// Signal to trigger resource refetching
	let trigger = RwSignal::new(false);
	// Resource that fetches S3 objects, re-running whenever `trigger` changes
	let s3_objects_resource = LocalResource::new(move || {
		trigger.get();
		S3ObjectsQuery::run()
	});
	// Callback to update the trigger, effectively reloading the table
	let on_change = Callback::new(move |_| trigger.update(|n| *n = !*n));

	view! {
		<ErrorBoundary fallback=dump_errors>
			<div class="relative w-dvw">
				<div class="container mx-auto">
					<h1>"Admin Page"</h1>
					<section>
						<h2>"Objects Table"</h2>
						<S3ObjectsTable
							s3_objects_resource=s3_objects_resource
							on_change=on_change
						/>
					</section>
					<section>
						<h2>"Add new entries"</h2>
						<FileUpload on_success=on_change />
					</section>
				</div>
			</div>
		</ErrorBoundary>
	}
}
