use crate::constants::{MSG_403, TITLE_403};
use leptos::prelude::*;

#[component]
pub fn Forbidden() -> impl IntoView {
	view! {
		<div class="flex flex-col items-center justify-center h-full pt-10">
			<h1 class="text-4xl font-bold mb-4">{TITLE_403}</h1>
			<p class="text-xl">{MSG_403}</p>
		</div>
	}
}
