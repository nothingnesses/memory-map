use crate::{
	AppConfig,
	components::password_input::PasswordInput,
	constants::{
		BUTTON_REGISTER, LABEL_CONFIRM_PASSWORD, LABEL_EMAIL, LABEL_PASSWORD,
		MSG_PASSWORDS_DO_NOT_MATCH, TITLE_REGISTER,
	},
	graphql_queries::{
		config::ConfigQuery,
		register::{RegisterMutation, register_mutation},
	},
};
use leptos::{ev, prelude::*, task::spawn_local};
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

	let config = use_context::<AppConfig>().expect(crate::constants::ERR_APP_CONFIG_MISSING);
	let api_url = config.api_url.clone();
	let config_resource = LocalResource::new(move || {
		let api_url = api_url.clone();
		async move { ConfigQuery::run(api_url).await.ok() }
	});

	let navigate_effect = navigate.clone();
	Effect::new(move |_| {
		if let Some(config) = config_resource.get().flatten()
			&& !config.enable_registration
		{
			navigate_effect("/sign-in", Default::default());
		}
	});

	let on_register = move |_| {
		let email_val = email.get();
		let password_val = password.get();
		let confirm_password_val = confirm_password.get();
		let navigate = navigate.clone();
		let config = use_context::<AppConfig>().expect(crate::constants::ERR_APP_CONFIG_MISSING);
		let api_url = config.api_url.clone();

		if password_val != confirm_password_val {
			error_message.set(Some(MSG_PASSWORDS_DO_NOT_MATCH.to_string()));
			return;
		}

		is_loading.set(true);
		spawn_local(async move {
			let variables =
				register_mutation::Variables { email: email_val, password: password_val };

			match RegisterMutation::run(api_url, variables).await {
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

	let on_submit = move |ev: ev::SubmitEvent| {
		ev.prevent_default();
		on_register(());
	};

	view! {
		<div class="grid gap-4 place-items-center h-full pt-10">
			<h1 class="text-2xl font-bold">{TITLE_REGISTER}</h1>
			<form
				on:submit=on_submit
				class="grid gap-4 w-full max-w-md p-4 bg-white rounded shadow-md border border-gray-200"
			>
				<label class="grid gap-2">
					<div class="block text-gray-700 text-sm font-bold">{LABEL_EMAIL}</div>
					<Input value=email placeholder=LABEL_EMAIL disabled=is_loading />
				</label>
				<label class="grid gap-2">
					<div class="block text-gray-700 text-sm font-bold">{LABEL_PASSWORD}</div>
					<PasswordInput
						value=password
						placeholder=LABEL_PASSWORD
						disabled=is_loading
					/>
				</label>
				<label class="grid gap-2">
					<div class="block text-gray-700 text-sm font-bold">{LABEL_CONFIRM_PASSWORD}</div>
					<PasswordInput
						value=confirm_password
						placeholder=LABEL_CONFIRM_PASSWORD
						disabled=is_loading
					/>
				</label>

				<Show when=move || error_message.with(Option::is_some)>
					<p class="text-red-500 text-xs italic">{error_message}</p>
				</Show>

				<div class="flex items-center justify-between">
					<Button
						attr:r#type="submit"
						class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
						disabled=is_loading
					>
						{BUTTON_REGISTER}
					</Button>
				</div>
			</form>
		</div>
	}
}
