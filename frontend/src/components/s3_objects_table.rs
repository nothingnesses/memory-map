// @todo Add better error-handling to prevent errors from prevent table from displaying. Errors should just be logged in console.

use crate::{
	CallbackAnyView, components::s3_object_table_rows::S3ObjectTableRows, dump_errors,
	graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
};
use leptos::{
	logging::{debug_error, debug_log},
	prelude::*,
	task::spawn_local,
	web_sys::{self, Request, RequestInit},
};
use lucide_leptos::Trash;
use std::collections::HashSet;
use thaw::*;
use wasm_bindgen_futures::{
	JsFuture,
	wasm_bindgen::{JsCast, JsValue},
};

#[derive(serde::Serialize)]
struct DeleteRequest {
	s3_objects: Vec<i64>,
}

#[component]
pub fn S3ObjectsTable(
	#[prop(into)] s3_objects_resource: Signal<LocalResource<Result<Vec<S3Object>, Error>>>,
	#[prop(into, default = Callback::new(|_| "Close".into_any()))]
	close_button_content: CallbackAnyView,
	// Callback to trigger a refresh of the data after deletion
	#[prop(into, default = Callback::new(|_| ()))] on_change: Callback<()>,
	#[prop(into, default = Callback::new(|_|
		view! {
			<div class="relative grid grid-flow-col gap-4 place-items-center">
				<Trash />
				"Delete selected"
			</div>
		}.into_any()
	))]
	delete_selected_button_content: CallbackAnyView,
) -> impl IntoView {
	let delete_objects = move |objects: Vec<S3Object>| {
		spawn_local(async move {
			let ids: Vec<i64> = objects.iter().filter_map(|o| o.id.parse::<i64>().ok()).collect();

			let payload = DeleteRequest { s3_objects: ids };
			let json = match serde_json::to_string(&payload) {
				Ok(j) => j,
				Err(e) => {
					debug_error!("Failed to serialize payload: {:?}", e);
					return;
				}
			};

			debug_log!("Delete request JSON: {json}");

			let options = RequestInit::new();
			options.set_method("POST");
			let headers = web_sys::Headers::new().unwrap();
			headers.append("Content-Type", "application/json").unwrap();
			options.set_headers(&headers);
			options.set_body(&JsValue::from_str(&json));

			let request = Request::new_with_str_and_init(
				"http://localhost:8000/api/delete-s3-objects/",
				&options,
			)
			.unwrap();

			debug_log!("Delete request: {:?}", request);

			match JsFuture::from(web_sys::window().unwrap().fetch_with_request(&request)).await {
				Ok(resp_value) => {
					let resp: web_sys::Response = resp_value.unchecked_into();
					debug_log!("Response status: {} {}", resp.status(), resp.status_text());
					if resp.ok() {
						debug_log!("Deleted objects: {json}");
						// Trigger the refresh callback to update the table
						on_change.run(());
					} else {
						let text = JsFuture::from(resp.text().unwrap())
							.await
							.unwrap()
							.as_string()
							.unwrap_or_default();
						debug_error!(
							"Failed to delete objects. Status: {} {}, Body: {}",
							resp.status(),
							resp.status_text(),
							text
						);
					}
				}
				Err(e) => {
					debug_error!("Failed to delete objects (network error): {:?}", e);
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
								<div>"Select"</div>
							</div>
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							"ID"
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							"Name"
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							"Made On"
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							"Location"
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							"Link"
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							"Content Type"
						</TableHeaderCell>
						<TableHeaderCell class="wrap-anywhere font-bold" resizable=true>
							"Actions"
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
										/>
									},
								)
							})
					}}
				</TableBody>
			</Table>
			<Show when=move || { !selected_ids.get().is_empty() } fallback=|| view! {}>
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
										"Are you sure you want to delete "
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
										}>"Yes"</Button>
										<Button on_click=move |_| {
											open_delete.set(false);
										}>"No"</Button>
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
