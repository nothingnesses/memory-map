// @todo Add better error-handling to prevent errors from prevent table from displaying. Errors should just be logged in console.

use crate::{
	components::s3_object_table_rows::S3ObjectTableRows, dump_errors,
	graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects,
};
use leptos::prelude::*;
use thaw::*;

#[component]
pub fn S3ObjectsTable(
	#[prop(into)] s3_objects_resource: Signal<
		LocalResource<Result<Vec<S3ObjectsQueryS3Objects>, Error>>,
	>
) -> impl IntoView {
	view! {
		<ErrorBoundary fallback=dump_errors>
			<ConfigProvider>
				<Table>
					<TableHeader>
						<TableRow>
							<TableHeaderCell resizable=true>"ID"</TableHeaderCell>
							<TableHeaderCell resizable=true>"Name"</TableHeaderCell>
							<TableHeaderCell resizable=true>"Made On"</TableHeaderCell>
							<TableHeaderCell resizable=true>"Location"</TableHeaderCell>
							<TableHeaderCell resizable=true>"Link"</TableHeaderCell>
							<TableHeaderCell resizable=true>"Content Type"</TableHeaderCell>
							<TableHeaderCell resizable=true>"Actions"</TableHeaderCell>
						</TableRow>
					</TableHeader>
					<TableBody>
						{move || {
							s3_objects_resource
								.get()
								.get()
								.map(|data| {
									Ok::<_, Error>(view! { <S3ObjectTableRows s3_objects=data? /> })
								})
						}}
					</TableBody>
				</Table>
			</ConfigProvider>
		</ErrorBoundary>
	}
}
