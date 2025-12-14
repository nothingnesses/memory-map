use crate::components::file_upload::FileUpload;
use crate::components::s3_objects_table::S3ObjectsTable;
use crate::{dump_errors, graphql_queries::s3_objects::S3ObjectsQuery};
use leptos::prelude::*;

#[component]
pub fn Admin() -> impl IntoView {
	view! {
		<ErrorBoundary fallback=dump_errors>
			<div class="relative w-dvw">
				<div class="container mx-auto">
					<h1>"Admin Page"</h1>
					<section>
						<h2>"Objects Table"</h2>
						<S3ObjectsTable s3_objects_resource=LocalResource::new(
							S3ObjectsQuery::run,
						) />
					</section>
					<section>
						<h2>"Add new entries"</h2>
						<FileUpload />
					</section>
				</div>
			</div>
		</ErrorBoundary>
	}
}
