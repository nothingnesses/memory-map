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
				<div class="container mx-auto grid gap-4">
					<h1 class="text-22px font-bold">"Admin"</h1>
					<div class="grid gap-6">
						<section class="grid gap-4">
							<h2 class="text-20px font-bold">"Objects"</h2>
							<S3ObjectsTable
								s3_objects_resource=s3_objects_resource
								on_change=on_change
							/>
						</section>
						<section class="grid gap-4">
							<h2 class="text-20px font-bold">"Add/update entries"</h2>
							<FileUpload on_success=on_change />
						</section>
					</div>
				</div>
			</div>
		</ErrorBoundary>
	}
}
