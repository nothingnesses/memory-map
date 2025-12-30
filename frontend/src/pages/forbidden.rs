use leptos::prelude::*;

#[component]
pub fn Forbidden() -> impl IntoView {
	view! {
		<div class="flex flex-col items-center justify-center h-full pt-10">
			<h1 class="text-4xl font-bold mb-4">"403 Forbidden"</h1>
			<p class="text-xl">"You do not have permission to view this page."</p>
		</div>
	}
}
