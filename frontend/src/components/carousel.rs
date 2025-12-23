use crate::{
	CallbackAnyView, ModularAdd, ModularSubtract,
	components::full_size_s3_object::FullSizeS3Object,
	components::s3_object::S3Object as S3ObjectComponent,
	graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
};
use leptos::{ev, logging::debug_log, prelude::*};
use lucide_leptos::{ChevronLeft, ChevronRight, X};
use thaw::*;

#[component]
pub fn Carousel(
	#[prop(into)] s3_objects: Signal<Vec<S3Object>>,
	#[prop(into, default = Callback::new(|_|
		view! {
			<div class="p-4 rounded-full bg-[rgba(0,0,0,0.4)] group-hover:text-white group-hover:group-active:text-white text-white">
				<X />
			</div>
		}.into_any()
	))]
	close_button_content: CallbackAnyView,
	#[prop(into, default = Callback::new(|_|
		view! {
			<ChevronLeft />
		}.into_any()
	))]
	previous_button_content: CallbackAnyView,
	#[prop(into, default = Callback::new(|_|
		view! {
			<ChevronRight />
		}.into_any()
	))]
	next_button_content: CallbackAnyView,
	#[prop(into, default = Signal::derive(|| true))] show_navigation_buttons: Signal<bool>,
) -> impl IntoView {
	let open = RwSignal::new(false);
	let index: RwSignal<usize> = RwSignal::new(0);
	let previous_slide = move || {
		index.set(index.get().modular_subtract(1, s3_objects.get().len()));
		debug_log!("called `previous_slide`");
	};
	let next_slide = move || {
		index.set(index.get().modular_add(1, s3_objects.get().len()));
		debug_log!("called `next_slide`");
	};
	let handle = window_event_listener(ev::keydown, move |ev| {
		let key = ev.key();
		debug_log!("{:?}", key.as_str());
		match key.as_str() {
			"ArrowLeft" => previous_slide(),
			"ArrowRight" => next_slide(),
			_ => {}
		};
	});
	on_cleanup(move || handle.remove());
	view! {
		<ConfigProvider>
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
			<Dialog class=r#"dialog [&_.thaw-dialog-surface\_\_backdrop]:hidden bg-none"# open>
				<DialogSurface class="dialog-surface border-none rounded-none m-unset p-unset bg-transparent">
					<div class="dialog-content relative w-dvw h-dvh grid place-items-center">
						// Buttons
						<div class="buttons absolute w-dvw h-dvh">
							// @todo Maybe this should be a component that emits index updates
							<Show when=move || { show_navigation_buttons.get() }>
								<div class="navigation-buttons absolute w-full h-full grid justify-between items-center grid-flow-col">
									<Button
										class="previous-button relative z-1 rounded-none w-100px h-dvh border-none bg-[rgba(0,0,0,0.4)] hover:bg-[rgba(0,0,0,0.4)] hover:active:bg-[rgba(0,0,0,0.4)] min-w-unset p-unset"
										on_click=move |_| previous_slide()
									>
										<div class="text-white">
											{previous_button_content.run(())}
										</div>
									</Button>
									<Button
										class="next-button relative z-1 rounded-none h-dvh w-100px border-none bg-[rgba(0,0,0,0.4)] hover:bg-[rgba(0,0,0,0.4)] hover:active:bg-[rgba(0,0,0,0.4)] min-w-unset p-unset"
										on_click=move |_| next_slide()
									>
										<div class="text-white">{next_button_content.run(())}</div>
									</Button>
								</div>
							</Show>
							<Button
								class="close-button absolute z-1 rounded-none right-0 bg-transparent border-none hover:bg-transparent hover:active:bg-transparent min-w-unset p-unset group"
								on_click=move |_| { open.set(false) }
							>
								{close_button_content.run(())}
							</Button>
						</div>
						// Lightbox
						<Button
							class="relative z-0 w-full h-full rounded-none border-none bg-[rgba(0,0,0,0.4)] hover:bg-[rgba(0,0,0,0.4)] hover:active:bg-[rgba(0,0,0,0.4)] p-unset"
							on_click=move |_| { open.set(false) }
						></Button>
						// Content
						<FullSizeS3Object class="full-size-s3-object absolute w-fit h-auto" s3_object=Signal::derive(move || {
							s3_objects.get()[index.get()].clone()
						}) />
					</div>
				</DialogSurface>
			</Dialog>
		</ConfigProvider>
	}
}
