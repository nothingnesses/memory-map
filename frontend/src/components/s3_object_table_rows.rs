use crate::{
	CallbackAnyView, components::s3_object_table_row::S3ObjectTableRow,
	graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
};
use leptos::prelude::*;
use lucide_leptos::{Pencil, Trash};
use std::collections::HashSet;

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
	view! {
		<ForEnumerate
			each=move || s3_objects.get()
			key=|s3_object| s3_object.id.clone()
			let(_s3_object_index,
			s3_object)
		>
			<S3ObjectTableRow
				s3_object=Signal::derive(move || s3_object.clone())
				selected_ids
				on_toggle
				on_delete
				on_edit
				delete_button_content
				edit_button_content
			/>
		</ForEnumerate>
	}
}
