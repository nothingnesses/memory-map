use crate::{
	CallbackAnyView,
	graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
};
use leptos::{
	logging::{debug_error, debug_log},
	prelude::*,
	task::spawn_local,
	web_sys::{self, Request, RequestInit},
};
use leptos_router::components::*;
use std::collections::HashSet;
use thaw::*;
use wasm_bindgen_futures::{JsFuture, wasm_bindgen::JsValue};

#[derive(serde::Serialize)]
struct DeleteRequest {
	s3_objects: Vec<i64>,
}

fn delete_objects(objects: Vec<S3Object>) {
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

		let options = RequestInit::new();
		options.set_method("POST");
		let headers = web_sys::Headers::new().unwrap();
		headers.append("Content-Type", "application/json").unwrap();
		options.set_headers(&headers);
		options.set_body(&JsValue::from_str(&json));

		let request = Request::new_with_str_and_init("/api/delete-s3-objects/", &options).unwrap();

		match JsFuture::from(web_sys::window().unwrap().fetch_with_request(&request)).await {
			Ok(_) => {
				debug_log!("Deleted objects");
				let _ = web_sys::window().unwrap().location().reload();
			}
			Err(e) => {
				debug_error!("Failed to delete objects: {:?}", e);
			}
		}
	});
}

#[component]
pub fn S3ObjectTableRows(
	#[prop(into)] s3_objects: Signal<Vec<S3Object>>,
	#[prop(into, default = Callback::new(|_| "Close".into_any()))]
	close_button_content: CallbackAnyView,
) -> impl IntoView {
	let open_delete = RwSignal::new(false);
	let selected_objects = RwSignal::new(vec![]);
	let checked_ids = RwSignal::new(HashSet::<String>::new());

	let open_delete_object_dialog = move |s3_object: S3Object| {
		selected_objects.set(vec![s3_object]);
		open_delete.set(true);
	};

	let open_delete_selected_dialog = move |_| {
		let ids = checked_ids.get();
		let objects = s3_objects.get();
		let to_delete: Vec<S3Object> =
			objects.into_iter().filter(|o| ids.contains(&o.id)).collect();

		if !to_delete.is_empty() {
			selected_objects.set(to_delete);
			open_delete.set(true);
		}
	};

	let toggle_id = move |id: String| {
		checked_ids.update(|ids| {
			if ids.contains(&id) {
				ids.remove(&id);
			} else {
				ids.insert(id);
			}
		});
	};

	view! {
		<ConfigProvider>
			<Button on_click=open_delete_selected_dialog>"Delete selected"</Button>
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
							<TableCell>
								<input
									type="checkbox"
									checked=move || {
										checked_ids.get().contains(&s3_object_for_checkbox.id)
									}
									on:change=move |_| toggle_id(s3_object_for_toggle.id.clone())
								/>
							</TableCell>
							<TableCell>{s3_object.id.clone()}</TableCell>
							<TableCell>{s3_object.name.clone()}</TableCell>
							<TableCell>{s3_object.made_on.clone()}</TableCell>
							<TableCell>
								{s3_object
									.location
									.clone()
									.map(|location| {
										format!("{}, {}", location.latitude, location.longitude)
									})}
							</TableCell>
							<TableCell>
								<A href=s3_object.url.clone()>"Click"</A>
							</TableCell>
							<TableCell>{s3_object.content_type.clone()}</TableCell>
							<TableCell>
								<div>
									<Button on_click=move |_| open_delete_object_dialog(
										s3_object_for_delete.clone(),
									)>"Delete"</Button>
								</div>
							</TableCell>
						</TableRow>
					}
				}
			</ForEnumerate>
			<Dialog open=open_delete>
				<DialogSurface>
					<DialogBody>
						<DialogContent>
							<div class="relative grid justify-items-center group">
								<Button on_click=move |_| {
									open_delete.set(false);
								}>{close_button_content.run(())}</Button>
								<div>
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
									<div>
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
		</ConfigProvider>
	}
}
