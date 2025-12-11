use crate::components::file_upload::FileUpload;
use crate::{
	components::s3_object_table_rows::S3ObjectTableRows, dump_errors,
	graphql_queries::s3_objects::S3ObjectsQuery,
};
use leptos::prelude::*;
use thaw::*;

#[component]
pub fn Admin() -> impl IntoView {
	let s3_objects_resource = LocalResource::new(S3ObjectsQuery::run);

	view! {
		<ErrorBoundary fallback=dump_errors>
			<div class="relative w-dvw">
				<div class="container mx-auto">
					<h1>"Admin Page"</h1>
					<section>
						<h2>"Objects Table"</h2>
						<ConfigProvider>
							<Table>
								<TableHeader>
									<TableRow>
										<TableHeaderCell resizable=true>"ID"</TableHeaderCell>
										<TableHeaderCell resizable=true>"Name"</TableHeaderCell>
										<TableHeaderCell resizable=true>"Made On"</TableHeaderCell>
										<TableHeaderCell resizable=true>"Location"</TableHeaderCell>
										<TableHeaderCell resizable=true>"Link"</TableHeaderCell>
										<TableHeaderCell resizable=true>
											"Content Type"
										</TableHeaderCell>
										<TableHeaderCell resizable=true>"Actions"</TableHeaderCell>
									</TableRow>
								</TableHeader>
								<TableBody>
									{move || {
										s3_objects_resource
											.get()
											.map(|data| {
												Ok::<
													_,
													Error,
												>(view! { <S3ObjectTableRows s3_objects=data? /> })
											})
									}}
								</TableBody>
							</Table>
						</ConfigProvider>
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
