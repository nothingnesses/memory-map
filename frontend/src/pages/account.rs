use crate::{
	auth::UserContext,
	components::password_input::PasswordInput,
	graphql_queries::{
		change_email::{ChangeEmailMutation, change_email_mutation},
		change_password::{ChangePasswordMutation, change_password_mutation},
		me::PublicityDefault,
		update_user_publicity::{UpdateUserPublicityMutation, update_user_publicity_mutation},
	},
};
use leptos::{prelude::*, task::spawn_local};
use thaw::*;

#[component]
pub fn Account() -> impl IntoView {
	let user_ctx = use_context::<UserContext>().expect("UserContext missing");
	let email = RwSignal::new(String::new());
	let old_password = RwSignal::new(String::new());
	let new_password = RwSignal::new(String::new());
	let confirm_new_password = RwSignal::new(String::new());
	let default_publicity = RwSignal::new(PublicityDefault::Private);

	let email_message = RwSignal::new(Option::<String>::None);
	let password_message = RwSignal::new(Option::<String>::None);
	let publicity_message = RwSignal::new(Option::<String>::None);
	let email_error = RwSignal::new(Option::<String>::None);
	let password_error = RwSignal::new(Option::<String>::None);
	let publicity_error = RwSignal::new(Option::<String>::None);

	let is_email_loading = RwSignal::new(false);
	let is_password_loading = RwSignal::new(false);
	let is_publicity_loading = RwSignal::new(false);

	Effect::new(move |_| {
		if let Some(Some(user)) = user_ctx.user.get() {
			email.set(user.email);
			default_publicity.set(user.default_publicity);
		}
	});

	let on_change_email = move |_| {
		let email_val = email.get();
		is_email_loading.set(true);
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
			is_email_loading.set(false);
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

		is_password_loading.set(true);
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
			is_password_loading.set(false);
		});
	};

	let on_change_publicity = move |ev| {
		let val = event_target_value(&ev);
		if let Ok(new_publicity) = val.parse::<PublicityDefault>() {
			default_publicity.set(new_publicity.clone());

			is_publicity_loading.set(true);
			spawn_local(async move {
				let variables =
					update_user_publicity_mutation::Variables { default_publicity: new_publicity };
				match UpdateUserPublicityMutation::run(variables).await {
					Ok(_) => {
						publicity_message
							.set(Some("Default publicity updated successfully".to_string()));
						publicity_error.set(None);
					}
					Err(e) => {
						publicity_error.set(Some(e.to_string()));
						publicity_message.set(None);
					}
				}
				is_publicity_loading.set(false);
			});
		}
	};

	view! {
		<div class="grid gap-4 place-items-center h-full pt-10 gap-10">
			<h1 class="text-2xl font-bold">"Account Settings"</h1>

			// Default Publicity
			<div class="grid gap-4 w-full max-w-md p-4 bg-white rounded shadow-md border border-gray-200">
				<h2 class="text-xl font-bold">"Default Publicity"</h2>
				<label class="grid gap-2">
					<div class="block text-gray-700 text-sm font-bold">
						"Default Publicity for New Objects"
					</div>
					<select
						class="p-2 border rounded bg-white w-full"
						on:change=on_change_publicity
						prop:value=move || default_publicity.get().to_string()
						disabled=is_publicity_loading
					>
						<option value="Public">"Public"</option>
						<option value="Private">"Private"</option>
					</select>
				</label>
				<Show when=move || publicity_message.with(Option::is_some)>
					<p class="text-green-500 text-xs italic">{publicity_message}</p>
				</Show>
				<Show when=move || publicity_error.with(Option::is_some)>
					<p class="text-red-500 text-xs italic">{publicity_error}</p>
				</Show>
			</div>

			// Change Email
			<div class="grid gap-4 w-full max-w-md p-4 bg-white rounded shadow-md border border-gray-200">
				<h2 class="text-xl font-bold">"Change Email"</h2>
				<label class="grid gap-2">
					<div class="block text-gray-700 text-sm font-bold">"New Email"</div>
					<Input
						value=email
						placeholder="New Email"
						disabled=is_email_loading
					/>
				</label>
				<Show when=move || email_message.with(Option::is_some)>
					<p class="text-green-500 text-xs italic">{email_message}</p>
				</Show>
				<Show when=move || email_error.with(Option::is_some)>
					<p class="text-red-500 text-xs italic">{email_error}</p>
				</Show>
				<Button
					on_click=on_change_email
					class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
					disabled=is_email_loading
				>
					"Update Email"
				</Button>
			</div>

			// Change Password
			<div class="grid gap-4 w-full max-w-md p-4 bg-white rounded shadow-md border border-gray-200">
				<h2 class="text-xl font-bold">"Change Password"</h2>
				<label class="grid gap-2">
					<div class="block text-gray-700 text-sm font-bold">
						"Old Password"
					</div>
					<PasswordInput
						value=old_password
						placeholder="Old Password"
						disabled=is_password_loading
					/>
				</label>
				<label class="grid gap-2">
					<div class="block text-gray-700 text-sm font-bold">
						"New Password"
					</div>
					<PasswordInput
						value=new_password
						placeholder="New Password"
						disabled=is_password_loading
					/>
				</label>
				<label class="grid gap-2">
					<div
						class="block text-gray-700 text-sm font-bold"
					>
						"Confirm New Password"
					</div>
					<PasswordInput
						value=confirm_new_password
						placeholder="Confirm New Password"
						disabled=is_password_loading
					/>
				</label>
				<Show when=move || password_message.with(Option::is_some)>
					<p class="text-green-500 text-xs italic mb-4">{password_message}</p>
				</Show>
				<Show when=move || password_error.with(Option::is_some)>
					<p class="text-red-500 text-xs italic mb-4">{password_error}</p>
				</Show>
				<Button
					on_click=on_change_password
					class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
					disabled=is_password_loading
				>
					"Update Password"
				</Button>
			</div>
		</div>
	}
}
