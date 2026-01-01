use crate::{
	AppConfig, dump_errors, js_date_value_to_iso,
	constants::{
		BUTTON_CANCEL, BUTTON_SUBMIT, ERROR_NETWORK, ERROR_SELECT_FILE, ERROR_TITLE,
		LABEL_SELECT_FILES, LABEL_SET_DATE_TIME, LABEL_SET_LATITUDE, LABEL_SET_LONGITUDE,
		LATITUDE_MAX, LATITUDE_MIN, LONGITUDE_MAX, LONGITUDE_MIN,
	},
};
use leptos::{
	html::Input,
	logging::{debug_error, debug_log},
	prelude::*,
	task::spawn_local,
	wasm_bindgen::JsCast,
	web_sys::{self, FormData, HtmlFormElement, MouseEvent, Request, RequestInit, SubmitEvent},
};
use leptos_router::components::Form;
use shared::ALLOWED_MIME_TYPES;
use thaw::*;
use wasm_bindgen_futures::JsFuture;

#[component]
pub fn FileUpload(
	// Callback to trigger a refresh of the parent's data (e.g., table)
	#[prop(into, default = Callback::new(|_| ()))] on_success: Callback<()>,
	#[prop(into)] on_cancel: Callback<()>,
) -> impl IntoView {
	let config = use_context::<AppConfig>().expect("AppConfig missing");
	let toaster = ToasterInjection::expect_context();
	let file_input_ref = NodeRef::<Input>::new();
	let made_on_input_ref = NodeRef::<Input>::new();

	let on_submit = move |event: SubmitEvent| {
		event.prevent_default();
		let target = event.target().unwrap();
		let form = target.unchecked_into::<HtmlFormElement>();
		let form_data = FormData::new_with_form(&form).unwrap();

		if let Some(input) = made_on_input_ref.get() {
			let value = input.value();
			if let Some(iso_str) = js_date_value_to_iso(&value) {
				let _ = form_data.set_with_str("made_on", &iso_str);
			}
		}

		debug_log!("{:?}", form_data);

		// Client-side validation
		if let Some(input) = file_input_ref.get()
			&& let Some(files) = input.files()
		{
			let files_length = files.length();
			if files_length == 0 {
				toaster.dispatch_toast(
					move || {
						view! {
							<Toast>
								<ToastTitle>{ERROR_TITLE}</ToastTitle>
								<ToastBody>{ERROR_SELECT_FILE}</ToastBody>
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
									<ToastTitle>{ERROR_TITLE}</ToastTitle>
									<ToastBody>
										{format!(
											"Unsupported file type: {file_name} ({file_type})",
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

		let api_url = config.api_url.clone();
		spawn_local(async move {
			let options = RequestInit::new();
			options.set_method("POST");
			options.set_body(&form_data);

			let url = format!("{}/api/locations/", api_url);
			let request = Request::new_with_str_and_init(&url, &options).unwrap();

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
										<ToastTitle>{ERROR_TITLE}</ToastTitle>
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
									<ToastTitle>{ERROR_TITLE}</ToastTitle>
									<ToastBody>{ERROR_NETWORK}</ToastBody>
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
				<div class="relative grid gap-4">
					<label>
						<div class="font-bold">{LABEL_SET_LATITUDE}</div>
						<input type="number" name="latitude" min=LATITUDE_MIN max=LATITUDE_MAX step="any" />
					</label>
					<label>
						<div class="font-bold">{LABEL_SET_LONGITUDE}</div>
						<input type="number" name="longitude" min=LONGITUDE_MIN max=LONGITUDE_MAX step="any" />
					</label>
					<label>
						<div class="font-bold">{LABEL_SET_DATE_TIME}</div>
						<input type="datetime-local" node_ref=made_on_input_ref />
					</label>
					<label>
						<div class="font-bold">{LABEL_SELECT_FILES}</div>
						<input
							type="file"
							name="files"
							accept=ALLOWED_MIME_TYPES.join(",")
							multiple
							node_ref=file_input_ref
						/>
					</label>
					<div class="grid grid-flow-col justify-start gap-4">
						<Button class="w-fit">{BUTTON_SUBMIT}</Button>
						<Button
							class="w-fit"
							appearance=ButtonAppearance::Subtle
							on_click=move |e: MouseEvent| {
								e.prevent_default();
								on_cancel.run(());
							}
						>
							{BUTTON_CANCEL}
						</Button>
					</div>
				</div>
			</Form>
		</ErrorBoundary>
	}
}
