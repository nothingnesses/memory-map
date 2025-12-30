use crate::{
	CallbackAnyView,
	components::s3_object::S3Object as S3ObjectComponent,
	graphql_queries::{
		s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
		update_s3_object::{
			PublicityOverride, UpdateS3ObjectMutation,
			update_s3_object_mutation::{LocationInput, Variables},
		},
	},
};
use email_address::EmailAddress;
use leptos::{html::Select, logging::debug_error, prelude::*, task::spawn_local};
use std::{collections::HashSet, str::FromStr};
use thaw::*;

#[component]
pub fn S3ObjectTableRow(
	#[prop(into)] s3_object: Signal<S3Object>,
	#[prop(into)] selected_ids: Signal<HashSet<String>>,
	#[prop(into)] on_toggle: Callback<String>,
	#[prop(into)] on_delete: Callback<S3Object>,
	#[prop(into)] on_edit: Callback<S3Object>,
	#[prop(into)] delete_button_content: CallbackAnyView,
	#[prop(into)] edit_button_content: CallbackAnyView,
) -> impl IntoView {
	let viewing_object = RwSignal::new(None::<S3Object>);
	let open_view = RwSignal::new(false);
	let toaster = ToasterInjection::expect_context();

	let show_allowed_users_dialog = RwSignal::new(false);
	let allowed_users_input = RwSignal::new(String::new());
	let select_ref = NodeRef::<Select>::new();

	// Initialize allowed_users_input when the dialog opens or object changes
	Effect::new(move |_| {
		let obj = s3_object.get();
		if !obj.allowed_users.is_empty() {
			allowed_users_input.set(obj.allowed_users.join(", "));
		} else {
			allowed_users_input.set(String::new());
		}
	});

	let update_object = move |new_publicity: PublicityOverride,
	                          new_allowed_users: Option<Vec<String>>| {
		let s3_object = s3_object.get();
		spawn_local(async move {
			let location = s3_object
				.location
				.map(|loc| LocationInput { latitude: loc.latitude, longitude: loc.longitude });

			let made_on = s3_object.made_on;

			let variables = Variables {
				id: s3_object.id.clone(),
				name: s3_object.name.clone(),
				made_on,
				location,
				publicity: new_publicity,
				allowed_users: new_allowed_users.or(Some(s3_object.allowed_users.clone())),
			};

			match UpdateS3ObjectMutation::run(variables).await {
				Ok(_) => {
					toaster.dispatch_toast(
						move || {
							view! {
								<Toast>
									<ToastTitle>"Success"</ToastTitle>
									<ToastBody>"Object publicity updated"</ToastBody>
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
									<ToastBody>{format!("Failed to update object: {e}")}</ToastBody>
								</Toast>
							}
						},
						ToastOptions::default().with_intent(ToastIntent::Error),
					);
				}
			}
		});
	};

	let on_change_publicity = move |ev| {
		let val = event_target_value(&ev);
		if let Ok(new_publicity) = val.parse::<PublicityOverride>() {
			if new_publicity == PublicityOverride::SelectedUsers {
				show_allowed_users_dialog.set(true);
			} else {
				update_object(new_publicity, None);
			}
		}
	};

	let on_cancel_allowed_users = move |_| {
		show_allowed_users_dialog.set(false);
		// Reset the select to the current value
		if let Some(select) = select_ref.get() {
			select.set_value(&s3_object.get().publicity.to_string());
		}
	};

	let on_save_allowed_users = move |_| {
		let input = allowed_users_input.get();
		let users: Vec<String> =
			input.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

		// Validate emails
		let invalid_emails: Vec<String> = users
			.iter()
			.filter(|email| EmailAddress::from_str(email).is_err())
			.map(|s| s.to_string())
			.collect();

		if !invalid_emails.is_empty() {
			toaster.dispatch_toast(
				move || {
					view! {
						<Toast>
							<ToastTitle>"Invalid Emails"</ToastTitle>
							<ToastBody>
								{format!("Invalid email addresses: {}", invalid_emails.join(", "))}
							</ToastBody>
						</Toast>
					}
				},
				ToastOptions::default().with_intent(ToastIntent::Error),
			);
			return;
		}

		update_object(PublicityOverride::SelectedUsers, Some(users));
		show_allowed_users_dialog.set(false);
	};

	view! {
		<TableRow>
			<TableCell class="wrap-anywhere">
				<input
					type="checkbox"
					prop:checked=move || { selected_ids.get().contains(&s3_object.get().id) }
					on:change=move |_| on_toggle.run(s3_object.get().id.clone())
				/>
			</TableCell>
			<TableCell class="wrap-anywhere">{move || s3_object.get().id}</TableCell>
			<TableCell class="wrap-anywhere">{move || s3_object.get().name}</TableCell>
			<TableCell class="wrap-anywhere">{move || s3_object.get().made_on}</TableCell>
			<TableCell class="wrap-anywhere">
				{move || {
					s3_object
						.get()
						.location
						.map(|location| {
							format!("{}, {}", location.latitude, location.longitude)
						})
				}}
			</TableCell>
			<TableCell class="wrap-anywhere">
				<Button
					class="p-0 h-auto"
					on_click=move |_| {
						viewing_object.set(Some(s3_object.get()));
						open_view.set(true);
					}
				>
					<S3ObjectComponent s3_object=s3_object class="w-20 h-20 object-cover" />
				</Button>
			</TableCell>
			<TableCell class="wrap-anywhere">{move || s3_object.get().content_type}</TableCell>
			<TableCell class="wrap-anywhere">
				<select
					node_ref=select_ref
					class="p-2 border rounded bg-white"
					on:change=on_change_publicity
					prop:value=move || s3_object.get().publicity.to_string()
				>
					<option value="Default">"Default"</option>
					<option value="Public">"Public"</option>
					<option value="Private">"Private"</option>
					<option value="Selected Users">"Selected Users"</option>
				</select>
			</TableCell>
			<TableCell class="wrap-anywhere py-2">
				<div class="relative grid gap-4">
					<Button on_click=move |_| {
						on_delete.run(s3_object.get())
					}>{delete_button_content.run(())}</Button>
					<Button on_click=move |_| {
						on_edit.run(s3_object.get())
					}>{edit_button_content.run(())}</Button>
				</div>
			</TableCell>
		</TableRow>

		<Dialog open=open_view>
			<DialogSurface>
				<DialogBody>
					<DialogContent>
						<div class="grid gap-4">
							<div class="flex justify-end">
								<Button on_click=move |_| open_view.set(false)>"Close"</Button>
							</div>
							<div class="flex justify-center">
								{move || {
									viewing_object
										.get()
										.map(|obj| {
											view! {
												<S3ObjectComponent
													s3_object=Signal::derive(move || obj.clone())
													class="max-w-[80vw] max-h-[80vh]"
												/>
											}
										})
								}}
							</div>
						</div>
					</DialogContent>
				</DialogBody>
			</DialogSurface>
		</Dialog>

		<Dialog open=show_allowed_users_dialog>
			<DialogSurface>
				<DialogBody>
					<DialogTitle>"Manage Allowed Users"</DialogTitle>
					<DialogContent>
						<div class="grid gap-4">
							<label>
								<div class="font-bold mb-2">
									"Allowed Users (comma separated emails)"
								</div>
								<input
									type="text"
									class="w-full p-2 border rounded"
									prop:value=allowed_users_input
									on:input=move |ev| {
										allowed_users_input.set(event_target_value(&ev))
									}
									placeholder="user1@example.com, user2@example.com"
								/>
							</label>
						</div>
					</DialogContent>
					<DialogActions>
						<Button
							appearance=ButtonAppearance::Subtle
							on_click=on_cancel_allowed_users
						>
							"Cancel"
						</Button>
						<Button appearance=ButtonAppearance::Primary on_click=on_save_allowed_users>
							"Save"
						</Button>
					</DialogActions>
				</DialogBody>
			</DialogSurface>
		</Dialog>
	}
}
