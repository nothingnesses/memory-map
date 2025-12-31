use crate::graphql_queries::{
	admin_update_user::{AdminUpdateUserMutation, admin_update_user_mutation},
	request_password_reset::{RequestPasswordResetMutation, request_password_reset_mutation},
	users::{UserRole, UsersQuery},
};
use leptos::{prelude::*, task::spawn_local, wasm_bindgen::JsCast, web_sys::HtmlSelectElement};
use thaw::*;

#[component]
pub fn Users() -> impl IntoView {
	let trigger: RwSignal<usize> = RwSignal::new(0);
	let users_resource = LocalResource::new(move || {
		trigger.get();
		async move { UsersQuery::run().await.unwrap_or_default() }
	});

	let on_update_email = move |id: String, email: String, loading: RwSignal<bool>| {
		loading.set(true);
		spawn_local(async move {
			let variables =
				admin_update_user_mutation::Variables { id, role: None, email: Some(email) };
			let _ = AdminUpdateUserMutation::run(variables).await;
			loading.set(false);
			trigger.update(|n| *n = n.wrapping_add(1));
		});
	};

	let on_toggle_role = move |id: String, new_role: String, loading: RwSignal<bool>| {
		loading.set(true);
		spawn_local(async move {
			let variables =
				admin_update_user_mutation::Variables { id, role: Some(new_role), email: None };
			let _ = AdminUpdateUserMutation::run(variables).await;
			loading.set(false);
			trigger.update(|n| *n = n.wrapping_add(1));
		});
	};

	let on_reset_password = move |email: String, loading: RwSignal<bool>| {
		loading.set(true);
		spawn_local(async move {
			let variables = request_password_reset_mutation::Variables { email };
			let _ = RequestPasswordResetMutation::run(variables).await;
			loading.set(false);
		});
	};

	view! {
		<div class="container mx-auto pt-10">
			<h1 class="text-2xl font-bold mb-4">"Users"</h1>
			<Table>
				<TableHeader>
					<TableRow>
						<TableHeaderCell>"ID"</TableHeaderCell>
						<TableHeaderCell>"Email"</TableHeaderCell>
						<TableHeaderCell>"Role"</TableHeaderCell>
						<TableHeaderCell>"Created At"</TableHeaderCell>
						<TableHeaderCell>"Actions"</TableHeaderCell>
					</TableRow>
				</TableHeader>
				<TableBody>
					<Suspense fallback=move || {
						view! {
							<TableRow>
								<TableCell>"Loading..."</TableCell>
							</TableRow>
						}
					}>
						{move || {
							users_resource
								.get()
								.map(|users| {
									users
										.into_iter()
										.map(|user| {
											let id = user.id.clone();
											let email = RwSignal::new(user.email.clone());
											let created_at = user.created_at.clone();
											let update_email_action = on_update_email;
											let toggle_role_action = on_toggle_role;
											let reset_action = on_reset_password;
											let user_role = user.role.clone();
											let is_loading = RwSignal::new(false);
											let id_for_email = id.clone();
											let id_for_role = id.clone();

											view! {
												<TableRow>
													<TableCell>{id}</TableCell>
													<TableCell>
														<div class="flex gap-2">
															<Input value=email disabled=is_loading />
															<Button
																disabled=is_loading
																on_click=move |_| {
																	update_email_action(
																		id_for_email.clone(),
																		email.get(),
																		is_loading,
																	)
																}
															>
																"Save"
															</Button>
														</div>
													</TableCell>
													<TableCell>
														<div class="flex gap-2 items-center">
															<select
																class="p-2 border rounded bg-white"
																on:change=move |ev| {
																	let val = ev
																		.target()
																		.unwrap()
																		.unchecked_into::<HtmlSelectElement>()
																		.value();
																	toggle_role_action(id_for_role.clone(), val, is_loading)
																}
																prop:value=move || match user_role {
																	UserRole::ADMIN => "admin",
																	UserRole::USER => "user",
																	_ => "user",
																}
																disabled=is_loading
															>
																<option value="user">"User"</option>
																<option value="admin">"Admin"</option>
															</select>
														</div>
													</TableCell>
													<TableCell>{created_at}</TableCell>
													<TableCell>
														<Button
															disabled=is_loading
															on_click=move |_| { reset_action(email.get(), is_loading) }
														>
															"Reset Password"
														</Button>
													</TableCell>
												</TableRow>
											}
										})
										.collect_view()
								})
						}}
					</Suspense>
				</TableBody>
			</Table>
		</div>
	}
}
