use leptos::prelude::*;
use leptos_router::components::A;
use lucide_leptos::{Menu, X};

/// The Header component containing the navigation menu.
/// It supports a toggleable menu for mobile/desktop views and handles
/// the visual state when the menu is open or closed.
#[component]
pub fn Header(#[prop(into)] menu_open: RwSignal<bool>) -> impl IntoView {
	// Toggle the menu open state
	let toggle_header_menu = move || {
		menu_open.update(|n| *n = !*n);
	};

	// Close the menu (e.g., when a link is clicked)
	let close_header_menu = move || {
		menu_open.set(false);
	};

	// CSS classes for the header layer, including the hide-on-scroll transition logic
	const HEADER_LAYER_CLASSES: &str = "hide-on-scroll inset-0 h-100px w-dvw translate-y-[--hide-on-scroll-translate-y] group-[:not(.scrolling)]/page:transition-all";

	view! {
		<header class="fixed z-1">
			// Background layer with gradient and blur
			<div class=format!("absolute {HEADER_LAYER_CLASSES}")>
				<div class="absolute h-full w-full backdrop-blur-[2px] bg-gradient-to-b from-black to-black/40"></div>
			</div>

			// Menu overlay (visible when menu_open is true)
			<Show when=move || { menu_open.get() }>
				<div class="absolute inset-0">
					<div class="relative w-dvw w-dvh">
						// Backdrop button to close menu when clicking outside
						<button
							class="absolute inset-0 w-dvw h-dvh bg-black/40 backdrop-blur-[2px]"
							on:click=move |_| close_header_menu()
						></button>

						// Navigation menu content
						<div class="fixed h-dvh w-full max-w-[375px] top-0 right-0 overflow-y-scroll bg-#444">
							<nav class="
							group/header-menu
							text-white
							grid
							group-[:not(.scrolling)]/page:transition-all
							content-start
							justify-items-center
							h-full
							w-full
							font-bold
							mt-100px
							">
								<A
									attr:class="py-4 w-full grid place-items-center"
									href="/"
									on:click=move |_| close_header_menu()
								>
									"Map"
								</A>
								<A
									attr:class="py-4 w-full grid place-items-center"
									href="/objects"
									on:click=move |_| close_header_menu()
								>
									"Objects"
								</A>
							</nav>
						</div>
					</div>
				</div>
			</Show>

			// Header controls (Menu button)
			<div class=format!("relative pointer-events-none {HEADER_LAYER_CLASSES}")>
				<div class="absolute inset-0 h-full px-4 grid grid-flow-col items-center gap-4">
					<button
						class="pointer-events-auto relative z-1 cursor-pointer justify-self-end rounded-full grid place-items-center w-40px aspect-square bg-#666"
						on:click=move |_| toggle_header_menu()
						attr:aria-label=move || {
							if menu_open.get() { "Close menu" } else { "Open menu" }
						}
					>
						<Show
							when=move || { menu_open.get() }
							fallback=|| view! { <Menu color="#fff" /> }
						>
							<X color="#fff" />
						</Show>
					</button>
				</div>
			</div>
		</header>
	}
}
