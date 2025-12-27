use crate::{
	CallbackAnyView, components::s3_object::S3Object as S3ObjectComponent,
	graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
};
use leptos::prelude::*;
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
