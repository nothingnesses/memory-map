use crate::graphql_queries::register::{RegisterMutation, register_mutation};
use leptos::{prelude::*, task::spawn_local};
use leptos_router::hooks::use_navigate;
use thaw::*;

#[component]
pub fn Register() -> impl IntoView {
	let navigate = use_navigate();
	let email = RwSignal::new(String::new());
	let password = RwSignal::new(String::new());
	let confirm_password = RwSignal::new(String::new());
	let error_message = RwSignal::new(Option::<String>::None);
	let is_loading = RwSignal::new(false);

	let on_register = move |_| {
		let email_val = email.get();
		let password_val = password.get();
		let confirm_password_val = confirm_password.get();
		let navigate = navigate.clone();

		if password_val != confirm_password_val {
			error_message.set(Some("Passwords do not match".to_string()));
			return;
		}

		is_loading.set(true);
		spawn_local(async move {
			let variables =
				register_mutation::Variables { email: email_val, password: password_val };

			match RegisterMutation::run(variables).await {
				Ok(_) => {
					navigate("/sign-in", Default::default());
				}
				Err(e) => {
					error_message.set(Some(e.to_string()));
				}
			}
			is_loading.set(false);
		});
	};

	view! {
		<div class="flex flex-col items-center justify-center h-full pt-10">
			<h1 class="text-2xl font-bold mb-4">"Register"</h1>
			<div class="w-full max-w-md p-4 bg-white rounded shadow-md border border-gray-200">
				<div class="mb-4">
					<label class="block text-gray-700 text-sm font-bold mb-2" for="email">
						"Email"
					</label>
					<Input value=email placeholder="Email" disabled=move || is_loading.get() />
				</div>
				<div class="mb-4">
					<label class="block text-gray-700 text-sm font-bold mb-2" for="password">
						"Password"
					</label>
					<Input
						value=password
						placeholder="Password"
						attr:r#type="password"
						disabled=move || is_loading.get()
					/>
				</div>
				<div class="mb-6">
					<label
						class="block text-gray-700 text-sm font-bold mb-2"
						for="confirm_password"
					>
						"Confirm Password"
					</label>
					<Input
						value=confirm_password
						placeholder="Confirm Password"
						attr:r#type="password"
						disabled=move || is_loading.get()
					/>
				</div>

				<Show when=move || error_message.get().is_some()>
					<p class="text-red-500 text-xs italic mb-4">{error_message.get()}</p>
				</Show>

				<div class="flex items-center justify-between">
					<Button
						on_click=on_register
						class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
						disabled=move || is_loading.get()
					>
						"Register"
					</Button>
				</div>
			</div>
		</div>
	}
}
