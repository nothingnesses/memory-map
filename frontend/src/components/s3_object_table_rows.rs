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
use leptos::{logging::debug_error, prelude::*, task::spawn_local};
use lucide_leptos::{Pencil, Trash};
use std::collections::HashSet;
use thaw::*;

#[component]
pub fn S3ObjectTableRows(
	#[prop(into)] s3_objects: Signal<Vec<S3Object>>,
	#[prop(into)] selected_ids: Signal<HashSet<String>>,
	#[prop(into)] on_toggle: Callback<String>,
	#[prop(into)] on_delete: Callback<S3Object>,
	#[prop(into)] on_edit: Callback<S3Object>,
	#[prop(into, default = Callback::new(|_|
		view! {
			<div class="relative grid grid-flow-col gap-4 place-items-center">
				<Trash />
				"Delete"
			</div>
		}.into_any()
	))]
	delete_button_content: CallbackAnyView,
	#[prop(into, default = Callback::new(|_|
		view! {
			<div class="relative grid grid-flow-col gap-4 place-items-center">
				<Pencil />
				"Edit"
			</div>
		}.into_any()
	))]
	edit_button_content: CallbackAnyView,
) -> impl IntoView {
	let viewing_object = RwSignal::new(None::<S3Object>);
	let open_view = RwSignal::new(false);
	let toaster = ToasterInjection::expect_context();

	let on_change_publicity = move |s3_object: S3Object, new_publicity: PublicityOverride| {
		spawn_local(async move {
			let location = s3_object
				.location
				.map(|loc| LocationInput { latitude: loc.latitude, longitude: loc.longitude });

			let made_on = s3_object.made_on; // Already string or Option<String>

			let variables = Variables {
				id: s3_object.id.clone(),
				name: s3_object.name.clone(),
				made_on,
				location,
				publicity: new_publicity,
				allowed_users: Some(s3_object.allowed_users.clone()),
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

	view! {
		<ForEnumerate
			each=move || s3_objects.get()
			key=|s3_object| s3_object.id.clone()
			let(_s3_object_index,
			s3_object)
		>
			{
				let s3_object_for_checkbox = s3_object.clone();
				let s3_object_for_toggle = s3_object.clone();
				let s3_object_for_delete = s3_object.clone();
				let s3_object_for_edit = s3_object.clone();
				let s3_object_for_view = s3_object.clone();
				let s3_object_for_thumbnail = s3_object.clone();
				let s3_object_for_publicity = s3_object.clone();
				let current_publicity = s3_object.publicity.clone();

				view! {
					<TableRow>
						<TableCell class="wrap-anywhere">
							<input
								type="checkbox"
								prop:checked=move || {
									selected_ids.get().contains(&s3_object_for_checkbox.id)
								}
								on:change=move |_| on_toggle.run(s3_object_for_toggle.id.clone())
							/>
						</TableCell>
						<TableCell class="wrap-anywhere">{s3_object.id.clone()}</TableCell>
						<TableCell class="wrap-anywhere">{s3_object.name.clone()}</TableCell>
						<TableCell class="wrap-anywhere">{s3_object.made_on.clone()}</TableCell>
						<TableCell class="wrap-anywhere">
							{s3_object
								.location
								.clone()
								.map(|location| {
									format!("{}, {}", location.latitude, location.longitude)
								})}
						</TableCell>
						<TableCell class="wrap-anywhere">
							<Button
								class="p-0 h-auto"
								on_click=move |_| {
									viewing_object.set(Some(s3_object_for_view.clone()));
									open_view.set(true);
								}
							>
								<S3ObjectComponent
									s3_object=Signal::derive(move || {
										s3_object_for_thumbnail.clone()
									})
									class="w-20 h-20 object-cover"
								/>
							</Button>
						</TableCell>
						<TableCell class="wrap-anywhere">
							{s3_object.content_type.clone()}
						</TableCell>
						<TableCell class="wrap-anywhere">
							<select
								class="p-2 border rounded bg-white"
								on:change=move |ev| {
									let val = event_target_value(&ev);
									let new_publicity = match val.as_str() {
										"Default" => PublicityOverride::Default,
										"Public" => PublicityOverride::Public,
										"Private" => PublicityOverride::Private,
										_ => PublicityOverride::Default,
									};
									on_change_publicity(
										s3_object_for_publicity.clone(),
										new_publicity,
									);
								}
								prop:value=move || match current_publicity {
									PublicityOverride::Default => "Default",
									PublicityOverride::Public => "Public",
									PublicityOverride::Private => "Private",
									PublicityOverride::SelectedUsers => "Selected Users",
								}
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
									on_delete.run(s3_object_for_delete.clone())
								}>{delete_button_content.run(())}</Button>
								<Button on_click=move |_| {
									on_edit.run(s3_object_for_edit.clone())
								}>{edit_button_content.run(())}</Button>
							</div>
						</TableCell>
					</TableRow>
				}
			}
		</ForEnumerate>

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
	}
}
