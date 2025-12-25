use crate::dump_errors;
use leptos::{
	logging::{debug_error, debug_log},
	prelude::*,
	task::spawn_local,
	wasm_bindgen::JsCast,
	web_sys::{self, FormData, HtmlFormElement, Request, RequestInit, SubmitEvent},
};
use leptos_router::components::Form;
use thaw::*;
use wasm_bindgen_futures::JsFuture;

#[component]
pub fn FileUpload(
	// Callback to trigger a refresh of the parent's data (e.g., table)
	#[prop(into, default = Callback::new(|_| ()))] on_success: Callback<()>,
) -> impl IntoView {
	let on_submit = move |event: SubmitEvent| {
		event.prevent_default();
		let target = event.target().unwrap();
		let form = target.unchecked_into::<HtmlFormElement>();
		let form_data = FormData::new_with_form(&form).unwrap();
		debug_log!("{:?}", form_data);

		spawn_local(async move {
			let options = RequestInit::new();
			options.set_method("POST");
			options.set_body(&form_data);

			let request =
				Request::new_with_str_and_init("http://localhost:8000/api/locations/", &options)
					.unwrap();

			match JsFuture::from(web_sys::window().unwrap().fetch_with_request(&request)).await {
				Ok(resp_value) => {
					let resp: web_sys::Response = resp_value.unchecked_into();
					if resp.ok() {
						// Trigger the parent's refresh callback instead of reloading the page
						on_success.run(());
					} else {
						debug_error!(
							"Failed to upload files. Status: {} {}",
							resp.status(),
							resp.status_text()
						);
					}
				}
				Err(e) => {
					debug_error!("Failed to upload files (network error): {:?}", e);
				}
			}
		});
	};
	view! {
		<ErrorBoundary fallback=dump_errors>
			<Form action="" on:submit=on_submit>
				<div class="relative grid">
					<label>
						<div>"Set latitude"</div>
						<input type="number" name="latitude" min="-90" max="90" step="any" />
					</label>
					<label>
						<div>"Set longitude"</div>
						<input type="number" name="longitude" min="-180" max="180" step="any" />
					</label>
					<label>
						<div>"Select files to upload"</div>
						<input type="file" name="files" accept="image/*,video/*" multiple />
					</label>
					<Button class="w-fit">"Submit"</Button>
				</div>
			</Form>
		</ErrorBoundary>
	}
}
