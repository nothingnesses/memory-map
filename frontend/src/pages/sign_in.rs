use crate::graphql_queries::{
	config::ConfigQuery,
	login::{LoginMutation, login_mutation},
	request_password_reset::{RequestPasswordResetMutation, request_password_reset_mutation},
};
use leptos::{ev, prelude::*, task::spawn_local};
use leptos_router::components::A;
use thaw::*;

#[component]
pub fn SignIn() -> impl IntoView {
	let email = RwSignal::new(String::new());
	let password = RwSignal::new(String::new());
	let error_message = RwSignal::new(Option::<String>::None);
	let success_message = RwSignal::new(Option::<String>::None);
	let is_loading = RwSignal::new(false);

	let config_resource = LocalResource::new(move || async move { ConfigQuery::run().await.ok() });

	let on_sign_in = move |_| {
		let email_val = email.get();
		let password_val = password.get();

		is_loading.set(true);
		spawn_local(async move {
			let variables = login_mutation::Variables { email: email_val, password: password_val };

			match LoginMutation::run(variables).await {
				Ok(_) => {
					let _ = window().location().set_href("/");
				}
				Err(e) => {
					error_message.set(Some(e.to_string()));
				}
			}
			is_loading.set(false);
		});
	};

	let on_forgot_password = move |_| {
		let email_val = email.get();
		if email_val.is_empty() {
			error_message
				.set(Some("Please enter your email address to reset password".to_string()));
			return;
		}

		is_loading.set(true);
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
			is_loading.set(false);
		});
	};

	let on_submit = move |ev: ev::SubmitEvent| {
		ev.prevent_default();
		on_sign_in(());
	};

	view! {
		<div class="flex flex-col items-center justify-center h-full pt-10">
			<h1 class="text-2xl font-bold mb-4">"Sign In"</h1>
			<form
				on:submit=on_submit
				class="w-full max-w-md p-4 bg-white rounded shadow-md border border-gray-200"
			>
				<div class="mb-4">
					<label class="block text-gray-700 text-sm font-bold mb-2" for="email">
						"Email"
					</label>
					<Input value=email placeholder="Email" disabled=move || is_loading.get() />
				</div>
				<div class="mb-6">
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

				<Show when=move || error_message.get().is_some()>
					<p class="text-red-500 text-xs italic mb-4">{error_message.get()}</p>
				</Show>
				<Show when=move || success_message.get().is_some()>
					<p class="text-green-500 text-xs italic mb-4">{success_message.get()}</p>
				</Show>

				<div class="flex items-center justify-between">
					<Button
						attr:r#type="submit"
						class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
						disabled=move || is_loading.get()
					>
						"Sign In"
					</Button>
					<Button
						on_click=on_forgot_password
						appearance=ButtonAppearance::Transparent
						disabled=move || is_loading.get()
					>
						"Forgot Password?"
					</Button>
				</div>
				<div class="mt-4 text-center">
					<Suspense>
						{move || {
							config_resource
								.get()
								.flatten()
								.map(|config| {
									if config.enable_registration {
										view! {
											<A
												href="/register"
												attr:class="text-blue-500 hover:text-blue-700"
											>
												"Don't have an account? Register"
											</A>
										}
											.into_any()
									} else {
										().into_any()
									}
								})
						}}
					</Suspense>
				</div>
			</form>
		</div>
	}
}
