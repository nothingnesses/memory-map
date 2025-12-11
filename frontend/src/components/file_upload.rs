use crate::dump_errors;
use leptos::web_sys::SubmitEvent;
use leptos::{
	logging::{debug_error, debug_log},
	prelude::*,
};
use leptos_router::components::Form;
use thaw::*;

#[component]
pub fn FileUpload() -> impl IntoView {
	let on_submit = move |event: SubmitEvent| {
		debug_log!("{:?}", event);
	};
	view! {
		<ErrorBoundary fallback=|errors| {
			debug_error!("Failed to upload files: {:?}", errors.get());
			return dump_errors(errors);
		}>
			<Form action="" on:submit=on_submit>
				<div class="relative grid">
					<label>
						<div>"Set latitude"</div>
						<input type="number" min="-90" max="90" />
					</label>
					<label>
						<div>"Set longitude"</div>
						<input type="number" min="-180" max="180" />
					</label>
					<label>
						<div>"Select files to upload"</div>
						<input type="file" accept="image/*,video/*" multiple />
					</label>
					<Button class="w-fit">"Submit"</Button>
				</div>
			</Form>
		</ErrorBoundary>
	}
}
