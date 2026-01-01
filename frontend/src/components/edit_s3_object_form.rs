use crate::{
	AppConfig,
	constants::{
		BUTTON_CANCEL, BUTTON_SUBMIT, LABEL_ALLOWED_USERS, LABEL_NAME, LABEL_PUBLICITY,
		LABEL_SET_DATE_TIME, LABEL_SET_LATITUDE, LABEL_SET_LONGITUDE, LATITUDE_MAX, LATITUDE_MIN,
		LONGITUDE_MAX, LONGITUDE_MIN, MSG_ERROR_LOADING_OBJECT, MSG_INVALID_EMAILS,
		MSG_MISSING_USERS, MSG_OBJECT_UPDATED, MSG_UPDATE_FAILED, OPTION_DEFAULT, OPTION_PRIVATE,
		OPTION_PUBLIC, OPTION_SELECTED_USERS, PLACEHOLDER_ALLOWED_USERS, TITLE_INVALID_EMAILS,
		TITLE_SUCCESS, TITLE_WARNING,
	},
	dump_errors,
	graphql_queries::{
		s3_object_by_id::S3ObjectByIdQuery,
		s3_objects::s3_objects_query::S3ObjectsQueryS3Objects,
		types::PublicityOverride,
		update_s3_object::{
			UpdateS3ObjectMutation,
			update_s3_object_mutation::{LocationInput, Variables},
		},
	},
	iso_to_local_datetime_value, js_date_value_to_iso,
};
use email_address::EmailAddress;
use leptos::{
	logging::debug_error,
	prelude::*,
	task::spawn_local,
	web_sys::{MouseEvent, SubmitEvent},
};
use std::{collections::HashSet, str::FromStr};
use thaw::*;

/// Component for editing an existing S3 object.
///
/// Fetches the object data by ID and provides a form to update its properties.
/// Uses `thaw` components for UI and `leptos` signals for state management.
#[component]
pub fn EditS3ObjectForm(
	/// The ID of the S3 object to edit.
	#[prop(into)]
	id: Signal<i64>,
	/// Initial data to populate the form (Optimistic UI).
	#[prop(into)]
	initial_data: Signal<Option<S3ObjectsQueryS3Objects>>,
	/// Callback invoked when the update is successful.
	#[prop(into)]
	on_success: Callback<()>,
	/// Callback invoked when the user cancels the operation.
	#[prop(into)]
	on_cancel: Callback<()>,
) -> impl IntoView {
	// Resource to fetch the S3 object data when the ID changes.
	let config = use_context::<AppConfig>().expect(crate::constants::ERR_APP_CONFIG_MISSING);
	let config = StoredValue::new(config);
	let s3_object_resource = LocalResource::new(move || {
		let id = id.get();
		let api_url = config.with_value(|c| c.api_url.clone());
		async move {
			if id == 0 {
				return Err("Invalid ID".to_string());
			}
			// Fetch the S3 object data by ID
			S3ObjectByIdQuery::run(api_url, id).await.map_err(|e| e.to_string())
		}
	});

	let toaster = ToasterInjection::expect_context();

	// Form state signals
	let (name, set_name) = signal(String::new());
	let (latitude, set_latitude) = signal(None::<f64>);
	let (longitude, set_longitude) = signal(None::<f64>);
	let (made_on, set_made_on) = signal(String::new());
	let (publicity, set_publicity) = signal(PublicityOverride::Default);
	let (allowed_users, set_allowed_users) = signal(String::new());

	// Populate form from initial data (Optimistic UI)
	Effect::new(move |_| {
		if let Some(s3_object) = initial_data.get() {
			set_name.set(s3_object.name);
			if let Some(loc) = s3_object.location {
				set_latitude.set(Some(loc.latitude));
				set_longitude.set(Some(loc.longitude));
			}
			if let Some(iso_str) = s3_object.made_on
				&& let Some(local_str) = iso_to_local_datetime_value(&iso_str)
			{
				set_made_on.set(local_str);
			}
			set_publicity.set(s3_object.publicity);
			if !s3_object.allowed_users.is_empty() {
				set_allowed_users.set(s3_object.allowed_users.join(", "));
			}
		}
	});

	// Populate form when data is loaded from server
	Effect::new(move |_| {
		if let Some(Ok(s3_object)) = s3_object_resource.get() {
			set_name.set(s3_object.name);
			if let Some(loc) = s3_object.location {
				set_latitude.set(Some(loc.latitude));
				set_longitude.set(Some(loc.longitude));
			}
			if let Some(iso_str) = s3_object.made_on
				&& let Some(local_str) = iso_to_local_datetime_value(&iso_str)
			{
				set_made_on.set(local_str);
			}
			set_publicity.set(s3_object.publicity);
			if !s3_object.allowed_users.is_empty() {
				set_allowed_users.set(s3_object.allowed_users.join(", "));
			}
		}
	});

	let on_submit = move |event: SubmitEvent| {
		event.prevent_default();

		let name_val = name.get();
		let lat_val = latitude.get();
		let lon_val = longitude.get();
		let made_on_val = made_on.get();
		let publicity_val = publicity.get();
		let allowed_users_val = allowed_users.get();

		let location = if let (Some(lat), Some(lon)) = (lat_val, lon_val) {
			Some(LocationInput { latitude: lat, longitude: lon })
		} else {
			None
		};

		let made_on_iso = js_date_value_to_iso(&made_on_val);

		let allowed_users_vec: Vec<String> = allowed_users_val
			.split(',')
			.map(|s| s.trim().to_string())
			.filter(|s| !s.is_empty())
			.collect();

		// Validate emails
		let invalid_emails: Vec<String> = allowed_users_vec
			.iter()
			.filter(|email| EmailAddress::from_str(email).is_err())
			.map(|s| s.to_string())
			.collect();

		if !invalid_emails.is_empty() {
			toaster.dispatch_toast(
				move || {
					view! {
						<Toast>
							<ToastTitle>{TITLE_INVALID_EMAILS}</ToastTitle>
							<ToastBody>
								{format!("{MSG_INVALID_EMAILS}{}", invalid_emails.join(", "))}
							</ToastBody>
						</Toast>
					}
				},
				ToastOptions::default().with_intent(ToastIntent::Error),
			);
			return;
		}

		let api_url = config.with_value(|c| c.api_url.clone());
		spawn_local(async move {
			let variables = Variables {
				id: id.get().to_string(),
				name: name_val,
				made_on: made_on_iso,
				location,
				publicity: publicity_val,
				allowed_users: Some(allowed_users_vec.clone()),
			};

			match UpdateS3ObjectMutation::run(api_url, variables).await {
				Ok(updated_obj) => {
					toaster.dispatch_toast(
						move || {
							view! {
								<Toast>
									<ToastTitle>{TITLE_SUCCESS}</ToastTitle>
									<ToastBody>{MSG_OBJECT_UPDATED}</ToastBody>
								</Toast>
							}
						},
						ToastOptions::default().with_intent(ToastIntent::Success),
					);

					// Check for missing users
					let returned_users: HashSet<String> =
						updated_obj.allowed_users.into_iter().collect();
					let missing_users: Vec<String> = allowed_users_vec
						.into_iter()
						.filter(|u| !returned_users.contains(u))
						.collect();

					if !missing_users.is_empty() {
						toaster.dispatch_toast(
							move || {
								view! {
									<Toast>
										<ToastTitle>{TITLE_WARNING}</ToastTitle>
										<ToastBody>
											{format!("{MSG_MISSING_USERS}{}", missing_users.join(", "))}
										</ToastBody>
									</Toast>
								}
							},
							ToastOptions::default().with_intent(ToastIntent::Warning),
						);
					}

					on_success.run(());
				}
				Err(e) => {
					debug_error!("Failed to update object: {:?}", e);
					toaster.dispatch_toast(
						move || {
							view! {
								<Toast>
									<ToastTitle>"Error"</ToastTitle>
									<ToastBody>
										{format!("{MSG_UPDATE_FAILED}{e}")}
									</ToastBody>
								</Toast>
							}
						},
						ToastOptions::default().with_intent(ToastIntent::Error),
					);
				}
			}
		});
	};
	let on_submit = StoredValue::new(on_submit);

	view! {
		<ErrorBoundary fallback=dump_errors>
			<Suspense fallback=move || {
				view! { <p>"Loading..."</p> }
			}>
				{move || {
					s3_object_resource
						.get()
						.map(|result| {
							match result {
								Ok(_) => {
									view! {
										<form on:submit=move |ev| on_submit.with_value(|f| f(ev))>
											<div class="relative grid gap-4">
												<label>
													<div class="font-bold">{LABEL_NAME}</div>
													<input
														type="text"
														name="name"
														prop:value=name
														readonly
														class="bg-gray-200 cursor-not-allowed"
													/>
												</label>
												<label>
													<div class="font-bold">{LABEL_PUBLICITY}</div>
													<select
														class="p-2 border rounded bg-white"
														on:change=move |ev| {
															let val = event_target_value(&ev);
															if let Ok(new_publicity) = val.parse() {
																set_publicity.set(new_publicity);
															}
														}
														prop:value=move || publicity.get().to_string()
													>
														<option value="Default">{OPTION_DEFAULT}</option>
														<option value="Public">{OPTION_PUBLIC}</option>
														<option value="Private">{OPTION_PRIVATE}</option>
														<option value="Selected Users">{OPTION_SELECTED_USERS}</option>
													</select>
												</label>
												<Show when=move || {
													publicity.get() == PublicityOverride::SelectedUsers
												}>
													<label>
														<div class="font-bold">{LABEL_ALLOWED_USERS}</div>
														<input
															type="text"
															name="allowed_users"
															prop:value=allowed_users
															on:input=move |ev| {
																set_allowed_users.set(event_target_value(&ev))
															}
															placeholder=PLACEHOLDER_ALLOWED_USERS
														/>
													</label>
												</Show>
												<label>
													<div class="font-bold">{LABEL_SET_LATITUDE}</div>
													<input
														type="number"
														name="latitude"
														min=LATITUDE_MIN
														max=LATITUDE_MAX
														step="any"
														on:input=move |ev| {
															set_latitude
																.set(event_target_value(&ev).parse::<f64>().ok());
														}
														prop:value=move || {
															latitude.get().map(|f| f.to_string()).unwrap_or_default()
														}
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
														on:input=move |ev| {
															set_longitude
																.set(event_target_value(&ev).parse::<f64>().ok());
														}
														prop:value=move || {
															longitude.get().map(|f| f.to_string()).unwrap_or_default()
														}
													/>
												</label>
												<label>
													<div class="font-bold">{LABEL_SET_DATE_TIME}</div>
													<input
														type="datetime-local"
														on:input=move |ev| set_made_on.set(event_target_value(&ev))
														prop:value=made_on
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
										</form>
									}
										.into_any()
								}
								Err(e) => {
									view! { <p>{MSG_ERROR_LOADING_OBJECT} {e.to_string()}</p> }
										.into_any()
								}
							}
						})
				}}
			</Suspense>
		</ErrorBoundary>
	}
}
