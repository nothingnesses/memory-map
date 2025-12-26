use crate::dump_errors;
use leptos::{
	html::Input,
	logging::{debug_error, debug_log},
	prelude::*,
	task::spawn_local,
	wasm_bindgen::JsCast,
	web_sys::{self, FormData, HtmlFormElement, Request, RequestInit, SubmitEvent},
};
use leptos_router::components::Form;
use shared::ALLOWED_MIME_TYPES;
use thaw::*;
use wasm_bindgen_futures::JsFuture;

#[component]
pub fn FileUpload(
	// Callback to trigger a refresh of the parent's data (e.g., table)
	#[prop(into, default = Callback::new(|_| ()))] on_success: Callback<()>,
) -> impl IntoView {
	let toaster = ToasterInjection::expect_context();
	let file_input_ref = NodeRef::<Input>::new();

	let on_submit = move |event: SubmitEvent| {
		event.prevent_default();
		let target = event.target().unwrap();
		let form = target.unchecked_into::<HtmlFormElement>();
		let form_data = FormData::new_with_form(&form).unwrap();
		debug_log!("{:?}", form_data);

		// Client-side validation
		if let Some(input) = file_input_ref.get() {
			if let Some(files) = input.files() {
				let files_length = files.length();
				if files_length == 0 {
					toaster.dispatch_toast(
						move || {
							view! {
								<Toast>
									<ToastTitle>"Error"</ToastTitle>
									<ToastBody>
										"Please select at least one file to upload."
									</ToastBody>
								</Toast>
							}
						},
						ToastOptions::default().with_intent(ToastIntent::Error),
					);
					return;
				}
				for file in (0..files_length).map(|i| files.item(i).unwrap()) {
					let file_type = file.type_();
					if !ALLOWED_MIME_TYPES.contains(&file_type.as_str()) {
						let file_name = file.name();
						toaster.dispatch_toast(
							move || {
								view! {
									<Toast>
										<ToastTitle>"Error"</ToastTitle>
										<ToastBody>
											{format!(
												"Unsupported file type: {} ({})",
												file_name,
												file_type,
											)}
										</ToastBody>
									</Toast>
								}
							},
							ToastOptions::default().with_intent(ToastIntent::Error),
						);
						return;
					}
				}
			}
		}

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
						let text = JsFuture::from(resp.text().unwrap())
							.await
							.unwrap()
							.as_string()
							.unwrap_or_default();
						debug_error!(
							"Failed to upload files. Status: {} {}, Body: {}",
							resp.status(),
							resp.status_text(),
							text
						);
						toaster.dispatch_toast(
							move || {
								view! {
									<Toast>
										<ToastTitle>"Error"</ToastTitle>
										<ToastBody>{text}</ToastBody>
									</Toast>
								}
							},
							ToastOptions::default().with_intent(ToastIntent::Error),
						);
					}
				}
				Err(e) => {
					debug_error!("Failed to upload files (network error): {:?}", e);
					toaster.dispatch_toast(
						move || {
							view! {
								<Toast>
									<ToastTitle>"Error"</ToastTitle>
									<ToastBody>"Failed to upload files (network error)"</ToastBody>
								</Toast>
							}
						},
						ToastOptions::default().with_intent(ToastIntent::Error),
					);
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
						<input
							type="file"
							name="files"
							accept=ALLOWED_MIME_TYPES.join(",")
							multiple
							node_ref=file_input_ref
						/>
					</label>
					<Button class="w-fit">"Submit"</Button>
				</div>
			</Form>
		</ErrorBoundary>
	}
}
