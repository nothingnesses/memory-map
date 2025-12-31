use leptos::prelude::*;
use lucide_leptos::{Eye, EyeOff};

#[component]
pub fn PasswordInput(
	#[prop(into)] value: RwSignal<String>,
	#[prop(into)] placeholder: String,
	#[prop(into)] disabled: Signal<bool>,
	#[prop(optional, into)] id: Option<String>,
) -> impl IntoView {
	let show_password = RwSignal::new(false);

	view! {
		<div class="relative grid grid-flow-col gap-4">
			<input
				id=id
				class="rounded-sm px-2 border border-black"
				value=value
				placeholder=placeholder
				type=move || if show_password.get() { "text" } else { "password" }
				disabled=disabled
			/>
			<button
				type="button"
				class="cursor-pointer w-fit"
				aria-label=move || if show_password.get() { "Hide password" } else { "Show password" }
				on:click=move |ev| {
					ev.prevent_default();
					show_password.update(|s| *s = !*s);
				}
			>
				<Show
					when=move || show_password.get()
					fallback=|| view! { <Eye attr:class="w-5 h-5" attr:aria-hidden="true" /> }
				>
					<EyeOff attr:class="w-5 h-5" attr:aria-hidden="true" />
				</Show>
			</button>
		</div>
	}
}
