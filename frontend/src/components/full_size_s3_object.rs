use crate::{
	components::s3_object::S3Object as S3ObjectComponent,
	graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
};
use leptos::prelude::*;
use thaw::*;

#[component]
pub fn FullSizeS3Object(
	#[prop(into)] s3_object: Signal<S3Object>,
	#[prop(optional, into)] class: MaybeProp<Signal<String>>,
	#[prop(into)] rotation: Signal<usize>,
) -> impl IntoView {
	let open = RwSignal::new(false);
	view! {
		<ConfigProvider class=Signal::derive(move || {
			class.get().map(|s| s.get()).unwrap_or_default()
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
							class="absolute z-1 overflow-auto max-w-full max-h-full"
							class=("rotate-90", move || rotation.get() == 1)
							class=("rotate-180", move || rotation.get() == 2)
							class=("rotate-270", move || rotation.get() == 3)
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
		</ConfigProvider>
	}
}
