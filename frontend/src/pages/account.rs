use crate::{
	auth::UserContext,
	graphql_queries::{
		change_email::{ChangeEmailMutation, change_email_mutation},
		change_password::{ChangePasswordMutation, change_password_mutation},
		me::PublicityDefault,
		update_user_publicity::{UpdateUserPublicityMutation, update_user_publicity_mutation},
	},
};
use leptos::{prelude::*, task::spawn_local, web_sys::HtmlSelectElement};
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
		let new_publicity = match val.as_str() {
			"Public" => PublicityDefault::Public,
			"Private" => PublicityDefault::Private,
			_ => PublicityDefault::Private,
		};
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
	};

	view! {
		<div class="flex flex-col items-center justify-center h-full pt-10 gap-10">
			<h1 class="text-2xl font-bold">"Account Settings"</h1>

			// Default Publicity
			<div class="w-full max-w-md p-4 bg-white rounded shadow-md border border-gray-200">
				<h2 class="text-xl font-bold mb-4">"Default Publicity"</h2>
				<div class="mb-4">
					<label class="block text-gray-700 text-sm font-bold mb-2" for="publicity">
						"Default Publicity for New Objects"
					</label>
					<select
						class="p-2 border rounded bg-white w-full"
						on:change=on_change_publicity
						prop:value=move || match default_publicity.get() {
							PublicityDefault::Public => "Public",
							PublicityDefault::Private => "Private",
						}
						disabled=move || is_publicity_loading.get()
					>
						<option value="Public">"Public"</option>
						<option value="Private">"Private"</option>
					</select>
				</div>
				<Show when=move || publicity_message.get().is_some()>
					<p class="text-green-500 text-xs italic mb-4">{publicity_message.get()}</p>
				</Show>
				<Show when=move || publicity_error.get().is_some()>
					<p class="text-red-500 text-xs italic mb-4">{publicity_error.get()}</p>
				</Show>
			</div>

			// Change Email
			<div class="w-full max-w-md p-4 bg-white rounded shadow-md border border-gray-200">
				<h2 class="text-xl font-bold mb-4">"Change Email"</h2>
				<div class="mb-4">
					<label class="block text-gray-700 text-sm font-bold mb-2" for="email">
						"New Email"
					</label>
					<Input
						value=email
						placeholder="New Email"
						disabled=move || is_email_loading.get()
					/>
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
					disabled=move || is_email_loading.get()
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
					<Input
						value=old_password
						placeholder="Old Password"
						attr:r#type="password"
						disabled=move || is_password_loading.get()
					/>
				</div>
				<div class="mb-4">
					<label class="block text-gray-700 text-sm font-bold mb-2" for="new_password">
						"New Password"
					</label>
					<Input
						value=new_password
						placeholder="New Password"
						attr:r#type="password"
						disabled=move || is_password_loading.get()
					/>
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
						disabled=move || is_password_loading.get()
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
					disabled=move || is_password_loading.get()
				>
					"Update Password"
				</Button>
			</div>
		</div>
	}
}
