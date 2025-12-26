use crate::{
	dump_errors,
	graphql_queries::{
		s3_object_by_id::S3ObjectByIdQuery,
		upsert_s3_object::{
			UpsertS3ObjectQuery,
			upsert_s3_object_query::{LocationInput, Variables},
		},
	},
	iso_to_local_datetime_value, js_date_value_to_iso,
};
use leptos::{
	html::Input,
	logging::debug_error,
	prelude::*,
	task::spawn_local,
	wasm_bindgen::JsCast,
	web_sys::{FormData, HtmlFormElement, SubmitEvent},
};
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
	let made_on_input_ref = NodeRef::<Input>::new();

	let on_submit = move |event: SubmitEvent| {
		event.prevent_default();
		let target = event.target().unwrap();
		let form = target.unchecked_into::<HtmlFormElement>();
		let form_data = FormData::new_with_form(&form).unwrap();

		let name = form_data.get("name").as_string().unwrap_or_default();
		let latitude = form_data.get("latitude").as_string().and_then(|s| s.parse::<f64>().ok());
		let longitude = form_data.get("longitude").as_string().and_then(|s| s.parse::<f64>().ok());

		let mut made_on = None;
		if let Some(input) = made_on_input_ref.get() {
			let value = input.value();
			// Convert the local datetime value to ISO 8601 UTC string
			if let Some(iso_str) = js_date_value_to_iso(&value) {
				made_on = Some(iso_str);
			}
		}

		let location = if let (Some(lat), Some(lon)) = (latitude, longitude) {
			Some(LocationInput { latitude: lat, longitude: lon })
		} else {
			None
		};

		spawn_local(async move {
			let variables = Variables { name, made_on, location };

			// Send the mutation to update the S3 object
			match UpsertS3ObjectQuery::run(variables).await {
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
										Ok(s3_object) => {
											let made_on_default = s3_object.made_on.clone();
											// Use an effect to set the initial value of the datetime input
											// because we need to convert the ISO string to a local datetime string
											Effect::new(move |_| {
												if let Some(made_on) = made_on_default.as_ref() {
													if let Some(input) = made_on_input_ref.get() {
														if let Some(value) = iso_to_local_datetime_value(made_on) {
															input.set_value(&value);
														}
													}
												}
											});

											view! {
												<Form action="" on:submit=on_submit>
													<div class="relative grid gap-4">
														<label>
															<div class="font-bold">"Name"</div>
															<input
																type="text"
																name="name"
																value=s3_object.name.clone()
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
																value=s3_object
																	.location
																	.as_ref()
																	.map(|l| l.latitude)
																	.unwrap_or_default()
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
																value=s3_object
																	.location
																	.as_ref()
																	.map(|l| l.longitude)
																	.unwrap_or_default()
															/>
														</label>
														<label>
															<div class="font-bold">"Set date and time"</div>
															<input type="datetime-local" node_ref=made_on_input_ref />
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
