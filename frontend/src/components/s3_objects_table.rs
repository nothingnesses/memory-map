// @todo Add better error-handling to prevent errors from prevent table from displaying. Errors should just be logged in console.

use crate::{
	CallbackAnyView,
	components::s3_object_table_rows::S3ObjectTableRows,
	constants::{
		BUTTON_CLOSE, BUTTON_DELETE_SELECTED, BUTTON_NO, BUTTON_YES, HEADER_ACTIONS,
		HEADER_CONTENT_TYPE, HEADER_ID, HEADER_LOCATION, HEADER_MADE_ON, HEADER_NAME,
		HEADER_SELECT, HEADER_VIEW, MSG_CONFIRM_DELETE, MSG_DELETE_FAILED, MSG_DELETE_SUCCESS,
	},
	dump_errors,
	graphql_queries::{
		delete_s3_objects::DeleteS3ObjectsMutation,
		s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
	},
};
use leptos::{
	logging::{debug_error, debug_log},
	prelude::*,
	task::spawn_local,
};
use lucide_leptos::Trash;
use std::collections::HashSet;
use thaw::*;

#[component]
pub fn S3ObjectsTable(
	#[prop(into)] s3_objects_resource: Signal<LocalResource<Result<Vec<S3Object>, Error>>>,
	#[prop(into, default = Callback::new(|_| BUTTON_CLOSE.into_any()))]
	close_button_content: CallbackAnyView,
	// Callback to trigger a refresh of the data after deletion
	#[prop(into, default = Callback::new(|_| ()))] on_change: Callback<()>,
	#[prop(into)] on_edit: Callback<S3Object>,
	#[prop(into, default = Callback::new(|_|
		view! {
			<div class="relative grid grid-flow-col gap-4 place-items-center">
				<Trash />
				{BUTTON_DELETE_SELECTED}
			</div>
		}.into_any()
	))]
	delete_selected_button_content: CallbackAnyView,
) -> impl IntoView {
	let delete_objects = move |objects: Vec<S3Object>| {
		spawn_local(async move {
			let ids: Vec<String> = objects.iter().map(|o| o.id.clone()).collect();

			match DeleteS3ObjectsMutation::run(ids).await {
				Ok(_) => {
					debug_log!("{}", MSG_DELETE_SUCCESS);
					on_change.run(());
				}
				Err(e) => {
					debug_error!("{}: {:?}", MSG_DELETE_FAILED, e);
				}
			}
		});
	};
	let open_delete = RwSignal::new(false);
	let selected_objects = RwSignal::new(vec![]);
	let selected_ids = RwSignal::new(HashSet::<String>::new());

	let open_delete_object_dialog = move |s3_object: S3Object| {
		selected_objects.set(vec![s3_object]);
		open_delete.set(true);
	};

	let open_delete_selected_dialog = move |_| {
		let ids = selected_ids.get();
		if let Some(Ok(objects)) = s3_objects_resource.get().get() {
			let to_delete: Vec<S3Object> =
				objects.into_iter().filter(|o| ids.contains(&o.id)).collect();

			if !to_delete.is_empty() {
				selected_objects.set(to_delete);
				open_delete.set(true);
			}
		}
	};

	let toggle_id = move |id: String| {
		selected_ids.update(|ids| {
			if ids.contains(&id) {
				ids.remove(&id);
			} else {
				ids.insert(id);
			}
		});
	};

	let all_ids = Memo::new(move |_| {
		s3_objects_resource
			.get()
			.get()
			.and_then(|res| res.ok())
			.map(|objects| objects.iter().map(|o| o.id.clone()).collect::<HashSet<_>>())
			.unwrap_or_default()
	});

	let toggle_all = move |_| {
		let all = all_ids.get();
		let selected = selected_ids.get();
		let all_selected = !all.is_empty() && all.iter().all(|id| selected.contains(id));

		if all_selected {
			selected_ids.set(HashSet::new());
		} else {
			selected_ids.set(all);
		}
	};

	view! {
		<ErrorBoundary fallback=dump_errors>
			<Table>
				<TableHeader>
					<TableRow>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							<div class="flex gap-2 items-center">
								<input
									type="checkbox"
									prop:indeterminate=move || {
										let all = all_ids.get();
										let selected = selected_ids.get();
										let selected_count = all.intersection(&selected).count();
										selected_count > 0 && selected_count < all.len()
									}
									prop:checked=move || {
										let all = all_ids.get();
										let selected = selected_ids.get();
										!all.is_empty()
											&& all.iter().all(|id| selected.contains(id))
									}
									on:change=toggle_all
								/>
								<div>{HEADER_SELECT}</div>
							</div>
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							{HEADER_ID}
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							{HEADER_NAME}
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							{HEADER_MADE_ON}
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							{HEADER_LOCATION}
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							{HEADER_VIEW}
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							{HEADER_CONTENT_TYPE}
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							{HEADER_ACTIONS}
						</TableHeaderCell>
					</TableRow>
				</TableHeader>
				<TableBody>
					{move || {
						s3_objects_resource
							.get()
							.get()
							.map(|data| {
								Ok::<
									_,
									Error,
								>(
									view! {
										<S3ObjectTableRows
											s3_objects=data?
											selected_ids=selected_ids
											on_toggle=Callback::new(toggle_id)
											on_delete=Callback::new(open_delete_object_dialog)
											on_edit
										/>
									},
								)
							})
					}}
				</TableBody>
			</Table>
			<Show when=move || { !selected_ids.get().is_empty() } fallback=|| ()>
				<Button class="w-fit" on_click=open_delete_selected_dialog>
					{delete_selected_button_content.run(())}
				</Button>
			</Show>
			<Dialog open=open_delete>
				<DialogSurface>
					<DialogBody>
						<DialogContent>
							<div class="relative grid gap-4 group">
								<Button
									class="justify-self-end"
									on_click=move |_| {
										open_delete.set(false);
									}
								>
									{close_button_content.run(())}
								</Button>
								<div class="relative grid gap-4">
									<h2>
										{MSG_CONFIRM_DELETE}
										{move || {
											selected_objects
												.get()
												.iter()
												.map(|s3_object: &S3Object| {
													format!("\"{}\"", s3_object.name)
												})
												.collect::<Vec<_>>()
												.join(", ")
										}}"?"
									</h2>
									<div class="relative grid gap-4 grid-flow-col">
										<Button on_click=move |_| {
											delete_objects(selected_objects.get());
											open_delete.set(false);
										}>{BUTTON_YES}</Button>
										<Button on_click=move |_| {
											open_delete.set(false);
										}>{BUTTON_NO}</Button>
									</div>
								</div>
							</div>
						</DialogContent>
					</DialogBody>
				</DialogSurface>
			</Dialog>
		</ErrorBoundary>
	}
}
