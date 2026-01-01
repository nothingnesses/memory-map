use crate::components::{
	edit_s3_object_form::EditS3ObjectForm, file_upload::FileUpload,
	s3_objects_table::S3ObjectsTable,
};
use crate::graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object;
use crate::{
	constants::{
		BUTTON_ADD_OBJECT, BUTTON_CLOSE, TITLE_ADD_OBJECT, TITLE_EDIT_OBJECT, TITLE_OBJECTS,
	},
	dump_errors,
	graphql_queries::s3_objects::S3ObjectsQuery,
};
use leptos::prelude::*;
use thaw::*;

/// The Objects page component.
///
/// Displays a table of S3 objects and provides functionality to add new objects
/// via a file upload dialog and edit existing objects via an edit dialog.
#[component]
pub fn Objects() -> impl IntoView {
	// Signal to trigger resource refetching
	let trigger = RwSignal::new(0usize);
	// Signal to control the add object dialog visibility
	let open_add_object = RwSignal::new(false);
	// Signal to control the edit object dialog visibility
	let open_edit_object = RwSignal::new(false);
	// Signal to store the ID of the object being edited
	let editing_object_id = RwSignal::new(0i64);
	// Signal to store the object being edited for optimistic UI
	let editing_object = RwSignal::new(None::<S3Object>);

	// Resource that fetches S3 objects, re-running whenever `trigger` changes
	let s3_objects_resource = LocalResource::new(move || {
		trigger.get();
		S3ObjectsQuery::run()
	});

	// Callback to update the trigger, effectively reloading the table
	let on_change = Callback::new(move |_| trigger.update(|n| *n = n.wrapping_add(1)));

	// Callback for successful upload: close dialog and refresh table
	let on_upload_success = Callback::new(move |_| {
		open_add_object.set(false);
		on_change.run(());
	});

	// Callback for edit button click
	let on_edit = Callback::new(move |s3_object: S3Object| {
		if let Ok(id) = s3_object.id.parse::<i64>() {
			editing_object_id.set(id);
			editing_object.set(Some(s3_object));
			open_edit_object.set(true);
		}
	});

	// Callback for successful edit
	let on_edit_success = Callback::new(move |_| {
		open_edit_object.set(false);
		on_change.run(());
	});

	view! {
		<ErrorBoundary fallback=dump_errors>
			<div class="relative w-dvw">
				<div class="container mx-auto grid gap-4">
					<div class="flex justify-between items-center">
						<h1 class="text-22px font-bold">{TITLE_OBJECTS}</h1>
						<Button
							on_click=move |_| open_add_object.set(true)
							class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
						>
							{BUTTON_ADD_OBJECT}
						</Button>
					</div>
					<div class="grid gap-6">
						<section class="grid gap-4">
							<S3ObjectsTable s3_objects_resource on_change on_edit />
						</section>
					</div>
				</div>
			</div>

			<Dialog open=open_add_object>
				<DialogSurface>
					<DialogBody>
						<DialogContent>
							<div class="grid gap-4">
								<div class="flex justify-between items-center">
									<h2 class="text-xl font-bold">{TITLE_ADD_OBJECT}</h2>
									<Button on_click=move |_| {
										open_add_object.set(false)
									}>{BUTTON_CLOSE}</Button>
								</div>
								<FileUpload
									on_success=on_upload_success
									on_cancel=move |_| open_add_object.set(false)
								/>
							</div>
						</DialogContent>
					</DialogBody>
				</DialogSurface>
			</Dialog>

			<Dialog open=open_edit_object>
				<DialogSurface>
					<DialogBody>
						<DialogContent>
							<div class="grid gap-4">
								<div class="flex justify-between items-center">
									<h2 class="text-xl font-bold">{TITLE_EDIT_OBJECT}</h2>
									<Button on_click=move |_| {
										open_edit_object.set(false)
									}>{BUTTON_CLOSE}</Button>
								</div>
								<EditS3ObjectForm
									id=editing_object_id
									initial_data=editing_object
									on_success=on_edit_success
									on_cancel=move |_| open_edit_object.set(false)
								/>
							</div>
						</DialogContent>
					</DialogBody>
				</DialogSurface>
			</Dialog>
		</ErrorBoundary>
	}
}
