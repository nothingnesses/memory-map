use crate::{
	dump_errors,
	graphql_queries::{
		s3_object_by_id::S3ObjectByIdQuery,
		update_s3_object::{
			UpdateS3ObjectQuery,
			update_s3_object_query::{LocationInput, Variables},
		},
	},
	iso_to_local_datetime_value, js_date_value_to_iso,
};
use leptos::{logging::debug_error, prelude::*, task::spawn_local, web_sys::SubmitEvent};
use leptos_router::{
	components::{A, Form},
	hooks::use_params_map,
};
use thaw::*;

#[component]
pub fn EditS3Object() -> impl IntoView {
	let params = use_params_map();
	let id =
		move || params.get().get("id").and_then(|id| id.parse::<i64>().ok()).unwrap_or_default();

	let s3_object_resource = LocalResource::new(move || {
		let id = id();
		async move {
			if id == 0 {
				return Err("Invalid ID".to_string());
			}
			// Fetch the S3 object data by ID
			S3ObjectByIdQuery::run(id).await.map_err(|e| e.to_string())
		}
	});

	let toaster = ToasterInjection::expect_context();

	// Form state signals
	// Use signals for controlled inputs to manage form state explicitly
	let (name, set_name) = signal(String::new());
	let (latitude, set_latitude) = signal(None::<f64>);
	let (longitude, set_longitude) = signal(None::<f64>);
	let (made_on, set_made_on) = signal(String::new());

	// Populate form when data is loaded
	// Effect to populate form state from the resource data.
	// This runs once when the resource data becomes available, avoiding
	// the "Effect inside View" anti-pattern.
	Effect::new(move |_| {
		if let Some(Ok(s3_object)) = s3_object_resource.get() {
			set_name.set(s3_object.name);
			if let Some(loc) = s3_object.location {
				set_latitude.set(Some(loc.latitude));
				set_longitude.set(Some(loc.longitude));
			}
			if let Some(iso_str) = s3_object.made_on {
				if let Some(local_str) = iso_to_local_datetime_value(&iso_str) {
					set_made_on.set(local_str);
				}
			}
		}
	});

	// Submit handler uses signal values directly, avoiding manual DOM/FormData extraction
	let on_submit = move |event: SubmitEvent| {
		event.prevent_default();

		let name_val = name.get();
		let lat_val = latitude.get();
		let lon_val = longitude.get();
		let made_on_val = made_on.get();

		let location = if let (Some(lat), Some(lon)) = (lat_val, lon_val) {
			Some(LocationInput { latitude: lat, longitude: lon })
		} else {
			None
		};

		let made_on_iso = js_date_value_to_iso(&made_on_val);

		spawn_local(async move {
			let variables =
				Variables { id: id().to_string(), name: name_val, made_on: made_on_iso, location };

			// Send the mutation to update the S3 object using the specific update query
			match UpdateS3ObjectQuery::run(variables).await {
				Ok(_) => {
					toaster.dispatch_toast(
						move || {
							view! {
								<Toast>
									<ToastTitle>"Success"</ToastTitle>
									<ToastBody>"Object updated successfully"</ToastBody>
								</Toast>
							}
						},
						ToastOptions::default().with_intent(ToastIntent::Success),
					);
				}
				Err(e) => {
					debug_error!("Failed to update object: {:?}", e);
					toaster.dispatch_toast(
						move || {
							view! {
								<Toast>
									<ToastTitle>"Error"</ToastTitle>
									<ToastBody>
										{format!("Failed to update object: {}", e)}
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

	view! {
		<ErrorBoundary fallback=dump_errors>
			<div class="relative w-dvw">
				<div class="container mx-auto grid gap-4">
					<h1 class="text-22px font-bold">"Edit object"</h1>
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
												<Form action="" on:submit=on_submit>
													<div class="relative grid gap-4">
														<label>
															<div class="font-bold">"Name"</div>
															<input
																type="text"
																name="name"
																prop:value=name
																readonly
																class="bg-gray-200 cursor-not-allowed"
															/>
														</label>
														<label>
															<div class="font-bold">"Set latitude"</div>
															<input
																type="number"
																name="latitude"
																min="-90"
																max="90"
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
															<div class="font-bold">"Set longitude"</div>
															<input
																type="number"
																name="longitude"
																min="-180"
																max="180"
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
															<div class="font-bold">"Set date and time"</div>
															<input
																type="datetime-local"
																on:input=move |ev| set_made_on.set(event_target_value(&ev))
																prop:value=made_on
															/>
														</label>
														<div class="grid grid-flow-col justify-start gap-4">
															<Button class="w-fit">"Submit"</Button>
															<A href="/admin">
																<Button class="w-fit" appearance=ButtonAppearance::Subtle>
																	"Cancel"
																</Button>
															</A>
														</div>
													</div>
												</Form>
											}
												.into_any()
										}
										Err(e) => {
											view! { <p>"Error loading object: " {e.to_string()}</p> }
												.into_any()
										}
									}
								})
						}}
					</Suspense>
				</div>
			</div>
		</ErrorBoundary>
	}
}
