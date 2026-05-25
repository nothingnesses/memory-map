use {
	crate::{
		AppConfig,
		constants::{
			BUTTON_CANCEL,
			BUTTON_SUBMIT,
			ERR_NETWORK_UPLOAD_FAILED,
			ERR_SYSTEM_NO_WINDOW,
			ERR_SYSTEM_REQUEST_FAILED,
			ERR_SYSTEM_RESPONSE_CAST,
			ERR_UNSUPPORTED_FILE_TYPE,
			ERROR_SELECT_FILE,
			LABEL_SELECT_FILES,
			LABEL_SET_DATE_TIME,
			LABEL_SET_LATITUDE,
			LABEL_SET_LONGITUDE,
			LATITUDE_MAX,
			LATITUDE_MIN,
			LONGITUDE_MAX,
			LONGITUDE_MIN,
		},
		dump_errors,
		errors::{
			AppError,
			use_context_safe,
			use_error_context,
		},
		js_date_value_to_iso,
	},
	leptos::{
		html::Input,
		prelude::*,
		task::spawn_local,
		wasm_bindgen::JsCast,
		web_sys::{
			self,
			FormData,
			HtmlFormElement,
			MouseEvent,
			Request,
			RequestInit,
			SubmitEvent,
		},
	},
	leptos_router::components::Form,
	shared::ALLOWED_MIME_TYPES,
	thaw::*,
	wasm_bindgen_futures::JsFuture,
};

#[component]
pub fn FileUpload(
	// Callback to trigger a refresh of the parent's data (e.g., table)
	#[prop(into, default = Callback::new(|_| ()))] on_success: Callback<()>,
	#[prop(into)] on_cancel: Callback<()>,
) -> impl IntoView {
	let config = match use_context_safe::<AppConfig>("AppConfig") {
		Some(c) => c,
		None => return view! { <p>"System Error: Configuration missing"</p> }.into_any(),
	};
	let error_ctx = use_error_context();
	let file_input_ref = NodeRef::<Input>::new();
	let made_on_input_ref = NodeRef::<Input>::new();

	let on_submit = move |event: SubmitEvent| {
		event.prevent_default();
		let Some(target) = event.target() else {
			error_ctx.report(AppError::System("Submit event has no target".to_string()));
			return;
		};
		let Ok(form) = target.dyn_into::<HtmlFormElement>() else {
			error_ctx
				.report(AppError::System("Failed to cast target to HtmlFormElement".to_string()));
			return;
		};
		let Ok(form_data) = FormData::new_with_form(&form) else {
			error_ctx.report(AppError::System("Failed to create FormData from form".to_string()));
			return;
		};

		if let Some(input) = made_on_input_ref.get() {
			let value = input.value();
			if let Some(iso_str) = js_date_value_to_iso(&value) {
				let _ = form_data.set_with_str("made_on", &iso_str);
			}
		}

		// Client-side validation
		if let Some(input) = file_input_ref.get() &&
			let Some(files) = input.files()
		{
			let files_length = files.length();
			if files_length == 0 {
				error_ctx.report(AppError::Validation(ERROR_SELECT_FILE.to_string()));
				return;
			}
			for i in 0 .. files_length {
				let Some(file) = files.item(i) else {
					continue;
				};
				let file_type = file.type_();
				if !ALLOWED_MIME_TYPES.contains(&file_type.as_str()) {
					let file_name = file.name();
					error_ctx.report(AppError::Validation(format!(
						"{}{file_name} ({file_type})",
						ERR_UNSUPPORTED_FILE_TYPE
					)));
					return;
				}
			}
		}

		let api_url = config.api_url.clone();
		spawn_local(async move {
			let options = RequestInit::new();
			options.set_method("POST");
			options.set_body(&form_data);

			let url = format!("{api_url}/api/locations/");
			let Ok(request) = Request::new_with_str_and_init(&url, &options) else {
				error_ctx.report(AppError::System(ERR_SYSTEM_REQUEST_FAILED.to_string()));
				return;
			};

			let Some(window) = web_sys::window() else {
				error_ctx.report(AppError::System(ERR_SYSTEM_NO_WINDOW.to_string()));
				return;
			};

			match JsFuture::from(window.fetch_with_request(&request)).await {
				Ok(resp_value) => {
					let Ok(resp) = resp_value.dyn_into::<web_sys::Response>() else {
						error_ctx.report(AppError::System(ERR_SYSTEM_RESPONSE_CAST.to_string()));
						return;
					};
					if resp.ok() {
						// Trigger the parent's refresh callback instead of reloading the page
						on_success.run(());
					} else {
						let text = if let Ok(text_promise) = resp.text() {
							JsFuture::from(text_promise)
								.await
								.ok()
								.and_then(|v| v.as_string())
								.unwrap_or_default()
						} else {
							Default::default()
						};
						error_ctx.report(AppError::Network(format!(
							"{}{} {}, Body: {}",
							ERR_NETWORK_UPLOAD_FAILED,
							resp.status(),
							resp.status_text(),
							text
						)));
					}
				}
				Err(e) => {
					error_ctx.report(AppError::from(e));
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
						<input
							type="number"
							name="latitude"
							min=LATITUDE_MIN
							max=LATITUDE_MAX
							step="any"
						/>
					</label>
					<label>
						<div class="font-bold">{LABEL_SET_LONGITUDE}</div>
						<input
							type="number"
							name="longitude"
							min=LONGITUDE_MIN
							max=LONGITUDE_MAX
							step="any"
						/>
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
	.into_any()
}
