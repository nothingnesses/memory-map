use crate::{
	CallbackAnyView, ModularAdd, ModularSubtract,
	components::s3_object::S3Object as S3ObjectComponent,
	s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
};
use leptos::prelude::*;
use thaw::*;

#[component]
pub fn Carousel(
	#[prop(into)] s3_objects: Signal<Vec<S3Object>>,
	#[prop(into, default = Callback::new(|_| "Close".into_any()))]
	close_button_content: CallbackAnyView,
	#[prop(into, default = Callback::new(|_| "Previous".into_any()))]
	previous_button_content: CallbackAnyView,
	#[prop(into, default = Callback::new(|_| "Next".into_any()))]
	next_button_content: CallbackAnyView,
	#[prop(into, default = Signal::derive(|| true))] show_navigation_buttons: Signal<bool>,
) -> impl IntoView {
	let open = RwSignal::new(false);
	let index: RwSignal<usize> = RwSignal::new(0);
	view! {
		<ConfigProvider class="grid place-items-center">
			<div class="relative grid grid-cols-1 sm:grid-cols-2 gap-4 md:grid-cols-4 xl:grid-cols-6 2xl:grid-cols-8">
				<ForEnumerate
					each=move || s3_objects.get()
					key=|s3_object| s3_object.id.clone()
					let(s3_object_index,
					s3_object)
				>
					<Button on_click=move |_| {
						open.set(true);
						index.set(s3_object_index.get());
					}>
						<S3ObjectComponent s3_object=Signal::derive(move || s3_object.clone()) />
					</Button>
				</ForEnumerate>
			</div>
			<Dialog class="grid" open>
				<DialogSurface class="place-self-center max-w-dvw max-h-dvh p-0 bg-transparent m-0">
					<DialogBody class="grid place-content-center">
						<DialogContent class="w-fit h-full">
							<S3ObjectComponent
								class="relative z-1 object-scale-down w-fit h-full max-w-dvw"
								s3_object=Signal::derive(move || {
									s3_objects.get()[index.get()].clone()
								})
							/>
							<div class="absolute inset-0 h-full w-full">
								<Button
									class="absolute top-0 right-0 z-1 transition-all"
									on_click=move |_| { open.set(false) }
								>
									{close_button_content.run(())}
								</Button>
								// @todo Maybe this should be a component that emits index updates
								<Show when=move || { show_navigation_buttons.get() }>
									<div class="absolute inset-0 w-full h-full grid grid-flow-col justify-between items-center">
										<Button
											class="relative z-1 w-fit transition-all"
											on_click=move |_| {
												index
													.set(
														index.get().modular_subtract(1, s3_objects.get().len()),
													);
											}
										>
											{previous_button_content.run(())}
										</Button>
										<Button
											class="relative z-1 w-fit transition-all"
											on_click=move |_| {
												index
													.set(index.get().modular_add(1, s3_objects.get().len()));
											}
										>
											{next_button_content.run(())}
										</Button>
									</div>
								</Show>
							</div>
						</DialogContent>
					</DialogBody>
				</DialogSurface>
			</Dialog>
		</ConfigProvider>
	}
}
