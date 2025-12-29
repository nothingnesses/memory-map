use crate::graphql_queries::change_email::{ChangeEmailMutation, change_email_mutation};
use crate::graphql_queries::change_password::{ChangePasswordMutation, change_password_mutation};
use leptos::{prelude::*, task::spawn_local};
use thaw::*;

#[component]
pub fn Account() -> impl IntoView {
	let email = RwSignal::new(String::new());
	let old_password = RwSignal::new(String::new());
	let new_password = RwSignal::new(String::new());
	let confirm_new_password = RwSignal::new(String::new());

	let email_message = RwSignal::new(Option::<String>::None);
	let password_message = RwSignal::new(Option::<String>::None);
	let email_error = RwSignal::new(Option::<String>::None);
	let password_error = RwSignal::new(Option::<String>::None);

	let on_change_email = move |_| {
		let email_val = email.get();
		spawn_local(async move {
			let variables = change_email_mutation::Variables { new_email: email_val };
			match ChangeEmailMutation::run(variables).await {
				Ok(_) => {
					email_message.set(Some("Email updated successfully".to_string()));
					email_error.set(None);
				}
				Err(e) => {
					email_error.set(Some(e.to_string()));
					email_message.set(None);
				}
			}
		});
	};

	let on_change_password = move |_| {
		let old_pass = old_password.get();
		let new_pass = new_password.get();
		let confirm_pass = confirm_new_password.get();

		if new_pass != confirm_pass {
			password_error.set(Some("New passwords do not match".to_string()));
			return;
		}

		spawn_local(async move {
			let variables = change_password_mutation::Variables {
				old_password: old_pass,
				new_password: new_pass,
			};
			match ChangePasswordMutation::run(variables).await {
				Ok(_) => {
					password_message.set(Some("Password updated successfully".to_string()));
					password_error.set(None);
				}
				Err(e) => {
					password_error.set(Some(e.to_string()));
					password_message.set(None);
				}
			}
		});
	};

	view! {
		<div class="flex flex-col items-center justify-center h-full pt-10 gap-10">
			<h1 class="text-2xl font-bold">"Account Settings"</h1>

			// Change Email
			<div class="w-full max-w-md p-4 bg-white rounded shadow-md border border-gray-200">
				<h2 class="text-xl font-bold mb-4">"Change Email"</h2>
				<div class="mb-4">
					<label class="block text-gray-700 text-sm font-bold mb-2" for="email">
						"New Email"
					</label>
					<Input value=email placeholder="New Email" />
				</div>
				<Show when=move || email_message.get().is_some()>
					<p class="text-green-500 text-xs italic mb-4">{email_message.get()}</p>
				</Show>
				<Show when=move || email_error.get().is_some()>
					<p class="text-red-500 text-xs italic mb-4">{email_error.get()}</p>
				</Show>
				<Button
					on_click=on_change_email
					class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
				>
					"Update Email"
				</Button>
			</div>

			// Change Password
			<div class="w-full max-w-md p-4 bg-white rounded shadow-md border border-gray-200">
				<h2 class="text-xl font-bold mb-4">"Change Password"</h2>
				<div class="mb-4">
					<label class="block text-gray-700 text-sm font-bold mb-2" for="old_password">
						"Old Password"
					</label>
					<Input value=old_password placeholder="Old Password" attr:r#type="password" />
				</div>
				<div class="mb-4">
					<label class="block text-gray-700 text-sm font-bold mb-2" for="new_password">
						"New Password"
					</label>
					<Input value=new_password placeholder="New Password" attr:r#type="password" />
				</div>
				<div class="mb-6">
					<label
						class="block text-gray-700 text-sm font-bold mb-2"
						for="confirm_new_password"
					>
						"Confirm New Password"
					</label>
					<Input
						value=confirm_new_password
						placeholder="Confirm New Password"
						attr:r#type="password"
					/>
				</div>
				<Show when=move || password_message.get().is_some()>
					<p class="text-green-500 text-xs italic mb-4">{password_message.get()}</p>
				</Show>
				<Show when=move || password_error.get().is_some()>
					<p class="text-red-500 text-xs italic mb-4">{password_error.get()}</p>
				</Show>
				<Button
					on_click=on_change_password
					class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
				>
					"Update Password"
				</Button>
			</div>
		</div>
	}
}
