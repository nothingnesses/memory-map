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
		graphql_queries::{
			abort_object_upload::AbortObjectUploadMutation,
			complete_object_upload::{
				CompleteObjectUploadMutation,
				CompletedUploadPartInput,
			},
			create_object_upload_session::{
				CreateObjectUploadSessionInputVariables,
				CreateObjectUploadSessionMutation,
				PublicityOverride,
				UploadLocationInput,
			},
			presign_object_upload_parts::{
				PresignObjectUploadPartsMutation,
				PresignedUploadPart,
			},
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
			File,
			FormData,
			Headers,
			HtmlFormElement,
			MouseEvent,
			Request,
			RequestCredentials,
			RequestInit,
			RequestMode,
			Response,
			SubmitEvent,
		},
	},
	shared::ALLOWED_MIME_TYPES,
	thaw::*,
	wasm_bindgen_futures::JsFuture,
};

const MAX_PRESIGN_PARTS_PER_REQUEST: i64 = 100;

#[derive(Clone, Debug)]
struct UploadMetadata {
	made_on: Option<String>,
	location: Option<(f64, f64)>,
}

impl UploadMetadata {
	fn location_input(&self) -> Option<UploadLocationInput> {
		self.location.map(|(latitude, longitude)| UploadLocationInput {
			latitude,
			longitude,
		})
	}
}

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
	let (uploading, set_uploading) = signal(false);

	let on_submit = move |event: SubmitEvent| {
		event.prevent_default();
		if uploading.get_untracked() {
			return;
		}

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

		let made_on = made_on_input_ref.get().and_then(|input| {
			let value = input.value();
			js_date_value_to_iso(&value)
		});
		let metadata = match upload_metadata(&form_data, made_on) {
			Ok(metadata) => metadata,
			Err(error) => {
				error_ctx.report(error);
				return;
			}
		};

		let files = match selected_files(file_input_ref) {
			Ok(files) => files,
			Err(error) => {
				error_ctx.report(error);
				return;
			}
		};

		let api_url = config.api_url.clone();
		set_uploading.set(true);
		spawn_local(async move {
			let result = upload_files(api_url, files, metadata).await;
			set_uploading.set(false);
			match result {
				Ok(()) => on_success.run(()),
				Err(error) => error_ctx.report(error),
			}
		});
	};
	view! {
		<ErrorBoundary fallback=dump_errors>
			<form on:submit=on_submit>
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
						<Button attr:r#type="submit" attr:disabled=move || uploading.get() class="w-fit">
							{move || if uploading.get() { "Uploading..." } else { BUTTON_SUBMIT }}
						</Button>
						<Button
							attr:r#type="button"
							attr:disabled=move || uploading.get()
							class="w-fit"
							appearance=ButtonAppearance::Subtle
							on_click=move |e: MouseEvent| {
								e.prevent_default();
								if !uploading.get_untracked() {
									on_cancel.run(());
								}
							}
						>
							{BUTTON_CANCEL}
						</Button>
					</div>
				</div>
			</form>
		</ErrorBoundary>
	}
	.into_any()
}

fn selected_files(file_input_ref: NodeRef<Input>) -> Result<Vec<File>, AppError> {
	let input =
		file_input_ref.get().ok_or_else(|| AppError::Validation(ERROR_SELECT_FILE.to_string()))?;
	let files = input.files().ok_or_else(|| AppError::Validation(ERROR_SELECT_FILE.to_string()))?;
	let files_length = files.length();
	if files_length == 0 {
		return Err(AppError::Validation(ERROR_SELECT_FILE.to_string()));
	}

	let mut selected = Vec::with_capacity(files_length as usize);
	for index in 0 .. files_length {
		let Some(file) = files.item(index) else {
			continue;
		};
		let file_type = file.type_();
		if !ALLOWED_MIME_TYPES.contains(&file_type.as_str()) {
			return Err(AppError::Validation(format!(
				"{}{} ({})",
				ERR_UNSUPPORTED_FILE_TYPE,
				file.name(),
				file_type
			)));
		}
		if file.size() <= 0.0 {
			return Err(AppError::Validation(format!("{} is empty", file.name())));
		}
		selected.push(file);
	}

	if selected.is_empty() {
		return Err(AppError::Validation(ERROR_SELECT_FILE.to_string()));
	}
	Ok(selected)
}

fn upload_metadata(
	form_data: &FormData,
	made_on: Option<String>,
) -> Result<UploadMetadata, AppError> {
	let latitude = form_value(form_data, "latitude");
	let longitude = form_value(form_data, "longitude");
	let location = match (latitude.as_deref(), longitude.as_deref()) {
		(None, None) => None,
		(Some(_), None) | (None, Some(_)) =>
			return Err(AppError::Validation(
				"Latitude and longitude must both be provided or both be left blank".to_string(),
			)),
		(Some(latitude), Some(longitude)) => Some((
			parse_coordinate(latitude, "latitude", -90.0, 90.0)?,
			parse_coordinate(longitude, "longitude", -180.0, 180.0)?,
		)),
	};

	Ok(UploadMetadata {
		made_on,
		location,
	})
}

fn form_value(
	form_data: &FormData,
	name: &str,
) -> Option<String> {
	let value = form_data.get(name).as_string()?;
	let value = value.trim();
	if value.is_empty() { None } else { Some(value.to_string()) }
}

fn parse_coordinate(
	value: &str,
	name: &str,
	min: f64,
	max: f64,
) -> Result<f64, AppError> {
	let coordinate =
		value.parse::<f64>().map_err(|_| AppError::Validation(format!("Invalid {name}")))?;
	if !(min ..= max).contains(&coordinate) {
		return Err(AppError::Validation(format!("{name} must be between {min} and {max}")));
	}
	Ok(coordinate)
}

async fn upload_files(
	api_url: String,
	files: Vec<File>,
	metadata: UploadMetadata,
) -> Result<(), AppError> {
	for file in files {
		upload_file(api_url.clone(), file, &metadata).await?;
	}
	Ok(())
}

async fn upload_file(
	api_url: String,
	file: File,
	metadata: &UploadMetadata,
) -> Result<(), AppError> {
	let content_type = file.type_();
	let file_size_bytes = file_size_bytes(&file)?;
	let session = CreateObjectUploadSessionMutation::run(
		api_url.clone(),
		CreateObjectUploadSessionInputVariables {
			name: file.name(),
			content_type,
			file_size_bytes,
			made_on: metadata.made_on.clone(),
			location: metadata.location_input(),
			publicity: PublicityOverride::Default,
			allowed_users: Some(Vec::new()),
		},
	)
	.await?;

	let completed_parts = match upload_file_parts(
		api_url.clone(),
		&file,
		&session.object_id,
		session.part_size_bytes,
		session.total_parts,
	)
	.await
	{
		Ok(completed_parts) => completed_parts,
		Err(error) => {
			let _ = AbortObjectUploadMutation::run(api_url, session.object_id).await;
			return Err(error);
		}
	};

	CompleteObjectUploadMutation::run(api_url, session.object_id, completed_parts).await?;
	Ok(())
}

fn file_size_bytes(file: &File) -> Result<i64, AppError> {
	let size = file.size();
	if !size.is_finite() || size <= 0.0 || size > i64::MAX as f64 {
		return Err(AppError::Validation(format!("Invalid file size for {}", file.name())));
	}
	Ok(size as i64)
}

async fn upload_file_parts(
	api_url: String,
	file: &File,
	object_id: &str,
	part_size_bytes: i64,
	total_parts: i64,
) -> Result<Vec<CompletedUploadPartInput>, AppError> {
	let mut completed_parts = Vec::with_capacity(total_parts as usize);
	let mut next_part_number = 1;
	while next_part_number <= total_parts {
		let last_part_number =
			(next_part_number + MAX_PRESIGN_PARTS_PER_REQUEST - 1).min(total_parts);
		let part_numbers = (next_part_number ..= last_part_number).collect::<Vec<_>>();
		let mut presigned_parts = PresignObjectUploadPartsMutation::run(
			api_url.clone(),
			object_id.to_string(),
			part_numbers,
		)
		.await?;
		presigned_parts.sort_by_key(|part| part.part_number);

		for part in presigned_parts {
			completed_parts.push(upload_presigned_part(file, part_size_bytes, part).await?);
		}
		next_part_number = last_part_number + 1;
	}

	Ok(completed_parts)
}

async fn upload_presigned_part(
	file: &File,
	part_size_bytes: i64,
	part: PresignedUploadPart,
) -> Result<CompletedUploadPartInput, AppError> {
	let part_start = (part.part_number - 1)
		.checked_mul(part_size_bytes)
		.ok_or_else(|| AppError::Validation("Upload part offset overflowed".to_string()))?;
	let part_end = part_start
		.checked_add(part.expected_content_length)
		.ok_or_else(|| AppError::Validation("Upload part end offset overflowed".to_string()))?;
	let chunk =
		file.slice_with_f64_and_f64(part_start as f64, part_end as f64).map_err(AppError::from)?;
	if chunk.size() as i64 != part.expected_content_length {
		return Err(AppError::Validation(format!(
			"Upload part {} size did not match signed content length",
			part.part_number
		)));
	}

	let options = RequestInit::new();
	options.set_method(&part.method);
	options.set_mode(RequestMode::Cors);
	options.set_credentials(RequestCredentials::Omit);
	options.set_body(&chunk);

	let headers = Headers::new().map_err(AppError::from)?;
	for header in part.headers {
		headers.set(&header.name, &header.value).map_err(AppError::from)?;
	}
	options.set_headers(&headers);

	let request = Request::new_with_str_and_init(&part.url, &options)
		.map_err(|_| AppError::System(ERR_SYSTEM_REQUEST_FAILED.to_string()))?;
	let window =
		web_sys::window().ok_or_else(|| AppError::System(ERR_SYSTEM_NO_WINDOW.to_string()))?;
	let response_value = JsFuture::from(window.fetch_with_request(&request)).await?;
	let response = response_value
		.dyn_into::<Response>()
		.map_err(|_| AppError::System(ERR_SYSTEM_RESPONSE_CAST.to_string()))?;
	if !response.ok() {
		return Err(AppError::Network(format!(
			"{}{} {}",
			ERR_NETWORK_UPLOAD_FAILED,
			response.status(),
			response.status_text()
		)));
	}
	let e_tag = response
		.headers()
		.get("ETag")
		.map_err(AppError::from)?
		.ok_or_else(|| AppError::Network("Upload response did not include ETag".to_string()))?;

	Ok(CompletedUploadPartInput {
		part_number: part.part_number,
		e_tag,
	})
}
