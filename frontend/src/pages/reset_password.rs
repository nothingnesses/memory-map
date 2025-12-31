use crate::graphql_queries::reset_password::{ResetPasswordMutation, reset_password_mutation};
use leptos::{prelude::*, task::spawn_local};
use leptos_router::hooks::{use_navigate, use_query_map};
use thaw::*;

#[component]
pub fn ResetPassword() -> impl IntoView {
	let navigate = use_navigate();
	let query = use_query_map();
	let token = move || query.get().get("token").unwrap_or_default();

	let new_password = RwSignal::new(String::new());
	let confirm_password = RwSignal::new(String::new());
	let error_message = RwSignal::new(Option::<String>::None);
	let success_message = RwSignal::new(Option::<String>::None);
	let is_loading = RwSignal::new(false);

	let on_reset = move |_| {
		let token_val = token();
		let password_val = new_password.get();
		let confirm_val = confirm_password.get();
		let navigate = navigate.clone();

		if token_val.is_empty() {
			error_message.set(Some("Invalid token".to_string()));
			return;
		}

		if password_val != confirm_val {
			error_message.set(Some("Passwords do not match".to_string()));
			return;
		}

		is_loading.set(true);
		spawn_local(async move {
			let variables =
				reset_password_mutation::Variables { token: token_val, new_password: password_val };

			match ResetPasswordMutation::run(variables).await {
				Ok(_) => {
					success_message.set(Some(
						"Password reset successfully. Redirecting to sign in...".to_string(),
					));
					error_message.set(None);
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
		<div class="grid gap-4 place-items-center h-full pt-10">
			<h1 class="text-2xl font-bold">"Reset Password"</h1>
			<div class="grid gap-4 w-full max-w-md p-4 bg-white rounded shadow-md border border-gray-200">
				<label class="grid gap-2">
					<div class="block text-gray-700 text-sm font-bold">
						"New Password"
					</div>
					<Input
						value=new_password
						placeholder="New Password"
						attr:r#type="password"
						disabled=is_loading
					/>
				</label>
				<label class="grid gap-2">
					<div
						class="block text-gray-700 text-sm font-bold"
					>
						"Confirm Password"
					</div>
					<Input
						value=confirm_password
						placeholder="Confirm Password"
						attr:r#type="password"
						disabled=is_loading
					/>
				</label>

				<Show when=move || error_message.with(Option::is_some)>
					<p class="text-red-500 text-xs italic">{error_message}</p>
				</Show>
				<Show when=move || success_message.with(Option::is_some)>
					<p class="text-green-500 text-xs italic">{success_message}</p>
				</Show>

				<div class="flex items-center justify-between">
					<Button
						on_click=on_reset
						class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
						disabled=is_loading
					>
						"Reset Password"
					</Button>
				</div>
			</div>
		</div>
	}
}
