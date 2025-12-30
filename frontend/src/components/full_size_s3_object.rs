use crate::{
	components::s3_object::S3Object as S3ObjectComponent,
	graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
};
use leptos::prelude::*;
use thaw::*;

#[component]
pub fn FullSizeS3Object(
	#[prop(into)] s3_object: Signal<S3Object>,
	#[prop(optional, into)] class: MaybeProp<String>,
	#[prop(into)] rotation: Signal<usize>,
) -> impl IntoView {
	let open = RwSignal::new(false);
	view! {
		<div class=Signal::derive(move || {
			format!(
				"{} {}",
				class.get().unwrap_or_default(),
				match rotation.get() {
					1 => "rotate-90",
					2 => "rotate-180",
					3 => "rotate-270",
					_ => "",
				},
			)
		})>
			<Button
				class="p-unset rounded-none border-none"
				on_click=move |_| {
					open.set(true);
				}
			>
				// Constrained size content
				<S3ObjectComponent class="max-w-dvw max-h-dvh object-scale-down" s3_object />
			</Button>
			<Dialog open>
				<DialogSurface class="dialog-surface border-none rounded-none m-unset p-unset bg-transparent">
					<div class="dialog-content relative w-dvw h-dvh grid place-items-center">
						// Lightbox
						<Button
							class="relative z-0 w-full h-full rounded-none border-none bg-[rgba(0,0,0,0.4)] hover:bg-[rgba(0,0,0,0.4)] hover:active:bg-[rgba(0,0,0,0.4)] p-unset"
							on_click=move |_| { open.set(false) }
						></Button>
						// Full size content
						<div
							class="absolute z-1 overflow-auto"
							// Apply rotations
							class=("rotate-90", move || rotation.get() == 1)
							class=("rotate-180", move || rotation.get() == 2)
							class=("rotate-270", move || rotation.get() == 3)
							// Swap the width and height of the element in the flow of the document to prevent overflowing layout issue
							class=("max-w-full", move || rotation.get().is_multiple_of(2))
							class=("max-h-full", move || rotation.get().is_multiple_of(2))
							class=("max-w-dvh", move || !rotation.get().is_multiple_of(2))
							class=("max-h-dvw", move || !rotation.get().is_multiple_of(2))
						>
							<Button
								class="p-unset rounded-none border-none"
								on_click=move |_| {
									open.set(false);
								}
							>
								<S3ObjectComponent class="max-w-none block" s3_object />
							</Button>
						</div>
					</div>
				</DialogSurface>
			</Dialog>
		</div>
	}
}
