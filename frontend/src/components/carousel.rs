use crate::{
	CallbackAnyView, ModularAdd, ModularSubtract,
	components::full_size_s3_object::FullSizeS3Object,
	components::s3_object::S3Object as S3ObjectComponent,
	graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
};
use leptos::{ev, logging::debug_log, prelude::*};
use lucide_leptos::{ChevronLeft, ChevronRight, RotateCcw, RotateCw, X};
use std::time;
use thaw::*;
use web_sys::js_sys;

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
	#[prop(into, default = Signal::derive(|| 2000))] button_timeout_duration: Signal<u64>,
	#[prop(into, default = Signal::derive(|| 1024))] mobile_width: Signal<u64>,
	#[prop(into, default = Callback::new(|_|
		view! {
			<RotateCcw />
		}.into_any()
	))]
	anti_clockwise_button_content: CallbackAnyView,
	#[prop(into, default = Callback::new(|_|
		view! {
			<RotateCw />
		}.into_any()
	))]
	clockwise_button_content: CallbackAnyView,
	#[prop(into, default = Signal::derive(|| true))] show_bottom_buttons: Signal<bool>,
) -> impl IntoView {
	let rotation: RwSignal<usize> = RwSignal::new(0);
	let is_open = RwSignal::new(false);
	let index: RwSignal<usize> = RwSignal::new(0);
	let reset_rotation = move || {
		rotation.set(0);
		debug_log!("called `reset_rotation`");
	};
	let rotate_anti_clockwise = move || {
		rotation.set(rotation.get().modular_subtract(1, 4));
		debug_log!("called `rotate_anti_clockwise`");
	};
	let rotate_clockwise = move || {
		rotation.set(rotation.get().modular_add(1, 4));
		debug_log!("called `rotate_clockwise`");
	};
	let close = move || {
		is_open.set(false);
		reset_rotation();
		debug_log!("called `close`");
	};
	let previous_slide = move || {
		index.set(index.get().modular_subtract(1, s3_objects.get().len()));
		reset_rotation();
		debug_log!("called `previous_slide`");
	};
	let next_slide = move || {
		index.set(index.get().modular_add(1, s3_objects.get().len()));
		reset_rotation();
		debug_log!("called `next_slide`");
	};
	let show_buttons = RwSignal::new(true);
	let timer_handle: RwSignal<Option<TimeoutHandle>> = RwSignal::new(None);
	let last_activity = RwSignal::new(js_sys::Date::now());
	let trigger_check = RwSignal::new(());

	let is_mobile = RwSignal::new(false);
	let check_mobile = move || {
		let mobile = window()
			.match_media(
				format!("(max-width: {}px), (pointer: coarse)", mobile_width.get()).as_str(),
			)
			.ok()
			.flatten()
			.map(|m| m.matches())
			.unwrap_or(false);
		if is_mobile.get_untracked() != mobile {
			is_mobile.set(mobile);
		}
	};

	// Initial check
	check_mobile();

	let resize_handle = window_event_listener(ev::resize, move |_| {
		check_mobile();
	});

	let reset_timer = move || {
		last_activity.set(js_sys::Date::now());
		if !show_buttons.get_untracked() {
			show_buttons.set(true);
			debug_log!("buttons should be displayed");
		}
	};

	Effect::new(move |_| {
		trigger_check.track();
		if !is_mobile.get_untracked() && timer_handle.get_untracked().is_none() {
			let handle = set_timeout_with_handle(
				move || {
					let now = js_sys::Date::now();
					let elapsed = now - last_activity.get_untracked();
					let timeout = button_timeout_duration.get_untracked() as f64;

					if elapsed >= timeout {
						show_buttons.set(false);
						timer_handle.set(None);
						debug_log!("buttons should be hidden");
					} else {
						// Reschedule for remaining time
						let remaining = timeout - elapsed;
						let handle = set_timeout_with_handle(
							move || {
								timer_handle.set(None);
								trigger_check.set(());
							},
							time::Duration::from_millis(remaining as u64),
						)
						.ok();
						timer_handle.set(handle);
					}
				},
				time::Duration::from_millis(button_timeout_duration.get_untracked()),
			)
			.ok();
			timer_handle.set(handle);
		}
	});

	// Trigger initial timer if open
	Effect::new(move |_| {
		if is_open.get() && !is_mobile.get_untracked() {
			reset_timer();
			trigger_check.set(());
		}
	});

	let buttons_visible = move || is_mobile.get() || show_buttons.get();

	let keydown_handle = window_event_listener(ev::keydown, move |ev| {
		let key = ev.key();
		debug_log!("{:?}", key.as_str());
		match key.as_str() {
			"ArrowLeft" => previous_slide(),
			"ArrowRight" => next_slide(),
			_ => {}
		};
	});

	let mouse_move_handle = window_event_listener(ev::mousemove, move |_| {
		if is_open.get() && !is_mobile.get_untracked() {
			reset_timer();
			if timer_handle.get_untracked().is_none() {
				trigger_check.set(());
			}
		}
	});

	on_cleanup(move || {
		keydown_handle.remove();
		mouse_move_handle.remove();
		resize_handle.remove();
		if let Some(handle) = timer_handle.get_untracked() {
			handle.clear();
		}
	});
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
						is_open.set(true);
						index.set(s3_object_index.get());
					}>
						<S3ObjectComponent s3_object=Signal::derive(move || s3_object.clone()) />
					</Button>
				</ForEnumerate>
			</div>
			<Dialog
				class=r#"dialog [&_.thaw-dialog-surface\_\_backdrop]:hidden bg-none"#
				open=is_open
			>
				<DialogSurface class="dialog-surface border-none rounded-none m-unset p-unset bg-transparent">
					<div class="dialog-content relative w-dvw h-dvh grid place-items-center">
						// Buttons
						<div
							class="buttons absolute w-dvw h-dvh transition-opacity duration-500"
							class=(["opacity-0", "pointer-events-none"], move || !buttons_visible())
						>
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
										class="next-button relative z-1 rounded-none w-100px h-dvh border-none bg-[rgba(0,0,0,0.4)] hover:bg-[rgba(0,0,0,0.4)] hover:active:bg-[rgba(0,0,0,0.4)] min-w-unset p-unset"
										on_click=move |_| next_slide()
									>
										<div class="text-white">{next_button_content.run(())}</div>
									</Button>
								</div>
							</Show>
							<Show when=move || { show_bottom_buttons.get() }>
								<div class="bottom-buttons absolute w-full h-full grid items-end">
									<div class="relative grid gap-4 justify-content-center items-center grid-flow-col w-full bg-[rgba(0,0,0,0.4)]">
										<Button
											class="previous-button relative z-1 h-100px rounded-none border-none bg-transparent hover:bg-transparent hover:active:bg-transparent min-w-unset p-unset"
											on_click=move |_| rotate_anti_clockwise()
										>
											<div class="text-white">
												{anti_clockwise_button_content.run(())}
											</div>
										</Button>
										<Button
											class="next-button relative z-1 h-100px rounded-none border-none bg-transparent hover:bg-transparent hover:active:bg-transparent min-w-unset p-unset"
											on_click=move |_| rotate_clockwise()
										>
											<div class="text-white">
												{clockwise_button_content.run(())}
											</div>
										</Button>
									</div>
								</div>
							</Show>
							<Button
								class="close-button absolute z-1 rounded-none right-0 bg-transparent border-none hover:bg-transparent hover:active:bg-transparent min-w-unset p-unset group"
								on_click=move |_| close()
							>
								{close_button_content.run(())}
							</Button>
						</div>
						// Lightbox
						<Button
							class="relative z-0 w-full h-full rounded-none border-none bg-[rgba(0,0,0,0.4)] hover:bg-[rgba(0,0,0,0.4)] hover:active:bg-[rgba(0,0,0,0.4)] p-unset"
							on_click=move |_| close()
						></Button>
						// Content
						<FullSizeS3Object
							class="full-size-s3-object absolute w-fit h-auto"
							rotation
							s3_object=Signal::derive(move || {
								s3_objects.get()[index.get()].clone()
							})
						/>
					</div>
				</DialogSurface>
			</Dialog>
		</ConfigProvider>
	}
}
