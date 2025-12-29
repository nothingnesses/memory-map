use crate::auth::UserContext;
use crate::graphql_queries::{
	login::{LoginMutation, login_mutation},
	request_password_reset::{RequestPasswordResetMutation, request_password_reset_mutation},
};
use leptos::{prelude::*, task::spawn_local};
use leptos_router::{components::A, hooks::use_navigate};
use thaw::*;

#[component]
pub fn SignIn() -> impl IntoView {
	let navigate = use_navigate();
	let email = RwSignal::new(String::new());
	let password = RwSignal::new(String::new());
	let error_message = RwSignal::new(Option::<String>::None);
	let success_message = RwSignal::new(Option::<String>::None);

	let on_sign_in = move |_| {
		let email_val = email.get();
		let password_val = password.get();
		let navigate = navigate.clone();

		spawn_local(async move {
			let variables = login_mutation::Variables { email: email_val, password: password_val };

			match LoginMutation::run(variables).await {
				Ok(_) => {
					if let Some(ctx) = use_context::<UserContext>() {
						ctx.refetch.run(());
					}
					navigate("/", Default::default());
				}
				Err(e) => {
					error_message.set(Some(e.to_string()));
				}
			}
		});
	};

	let on_forgot_password = move |_| {
		let email_val = email.get();
		if email_val.is_empty() {
			error_message
				.set(Some("Please enter your email address to reset password".to_string()));
			return;
		}

		spawn_local(async move {
			let variables = request_password_reset_mutation::Variables { email: email_val };

			match RequestPasswordResetMutation::run(variables).await {
				Ok(_) => {
					success_message.set(Some("Password reset email sent".to_string()));
					error_message.set(None);
				}
				Err(e) => {
					error_message.set(Some(e.to_string()));
				}
			}
		});
	};

	view! {
		<div class="flex flex-col items-center justify-center h-full pt-10">
			<h1 class="text-2xl font-bold mb-4">"Sign In"</h1>
			<div class="w-full max-w-md p-4 bg-white rounded shadow-md border border-gray-200">
				<div class="mb-4">
					<label class="block text-gray-700 text-sm font-bold mb-2" for="email">
						"Email"
					</label>
					<Input value=email placeholder="Email" />
				</div>
				<div class="mb-6">
					<label class="block text-gray-700 text-sm font-bold mb-2" for="password">
						"Password"
					</label>
					<Input value=password placeholder="Password" attr:r#type="password" />
				</div>

				<Show when=move || error_message.get().is_some()>
					<p class="text-red-500 text-xs italic mb-4">{error_message.get()}</p>
				</Show>
				<Show when=move || success_message.get().is_some()>
					<p class="text-green-500 text-xs italic mb-4">{success_message.get()}</p>
				</Show>

				<div class="flex items-center justify-between">
					<Button
						on_click=on_sign_in
						class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
					>
						"Sign In"
					</Button>
					<Button on_click=on_forgot_password appearance=ButtonAppearance::Transparent>
						"Forgot Password?"
					</Button>
				</div>
				<div class="mt-4 text-center">
					<A href="/register" attr:class="text-blue-500 hover:text-blue-700">
						"Don't have an account? Register"
					</A>
				</div>
			</div>
		</div>
	}
}
