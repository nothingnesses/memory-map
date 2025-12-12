use crate::dump_errors;
use leptos::wasm_bindgen::JsCast;
use leptos::web_sys::{FormData, HtmlFormElement, Request, RequestInit, SubmitEvent};
use leptos::{
	logging::{debug_error, debug_log},
	prelude::*,
	task::spawn_local,
};
use leptos_router::components::Form;
use thaw::*;
use wasm_bindgen_futures::JsFuture;

#[component]
pub fn FileUpload() -> impl IntoView {
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

			let _ = JsFuture::from(leptos::web_sys::window().unwrap().fetch_with_request(&request))
				.await;
		});
	};
	view! {
		<ErrorBoundary fallback=|errors| {
			debug_error!("Failed to upload files: {:?}", errors.get());
			return dump_errors(errors);
		}>
			<Form action="" on:submit=on_submit>
				<div class="relative grid">
					<label>
						<div>"Set latitude"</div>
						<input type="number" name="latitude" min="-90" max="90" />
					</label>
					<label>
						<div>"Set longitude"</div>
						<input type="number" name="longitude" min="-180" max="180" />
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
