use crate::{
	CallbackAnyView, ModularAdd, ModularSubtract, render_s3_object,
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
		<ConfigProvider>
			<div class="relative grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
				<ForEnumerate
					each=move || s3_objects.get()
					key=|s3_object| s3_object.id.clone()
					let(s3_object_index,
					s3_object)
				>
					<Button on_click=move |_| {
						open.set(true);
						index.set(s3_object_index.get());
					}>{render_s3_object(s3_object)}</Button>
				</ForEnumerate>
			</div>
			<Dialog open>
				<DialogSurface>
					<DialogBody class="grid-cols-1">
						<div class="relative w-full grid grid-flow-col justify-between">
							<Button on_click=move |_| {
								open.set(false)
							}>{close_button_content.run(())}</Button>
						</div>
						<DialogContent>
							<div>
								{move || render_s3_object(s3_objects.get()[index.get()].clone())}
								<Show when=move || { show_navigation_buttons.get() }>
									<div>
										<Button on_click=move |_| {
											index
												.set(
													index.get().modular_subtract(1, s3_objects.get().len()),
												);
										}>{previous_button_content.run(())}</Button>
										<Button on_click=move |_| {
											index
												.set(index.get().modular_add(1, s3_objects.get().len()));
										}>{next_button_content.run(())}</Button>
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
