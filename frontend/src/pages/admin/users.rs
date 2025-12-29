use crate::graphql_queries::admin_update_user::{
	AdminUpdateUserMutation, admin_update_user_mutation,
};
use crate::graphql_queries::request_password_reset::{
	RequestPasswordResetMutation, request_password_reset_mutation,
};
use crate::graphql_queries::users::{UsersQuery, users_query::UsersQueryUsers as User};
use leptos::{prelude::*, task::spawn_local};
use thaw::*;

#[component]
pub fn Users() -> impl IntoView {
	let trigger = RwSignal::new(0);
	let users_resource = LocalResource::new(move || {
		trigger.get();
		async move { UsersQuery::run().await.unwrap_or_default() }
	});

	let on_update_user = move |id: String, role: String, email: String| {
		spawn_local(async move {
			let variables = admin_update_user_mutation::Variables { id: id.into(), role, email };
			let _ = AdminUpdateUserMutation::run(variables).await;
			trigger.update(|n| *n += 1);
		});
	};

	let on_reset_password = move |email: String| {
		spawn_local(async move {
			let variables = request_password_reset_mutation::Variables { email };
			let _ = RequestPasswordResetMutation::run(variables).await;
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
											let current_role = format!("{:?}", user.role);
											let update_action = on_update_user.clone();
											let reset_action = on_reset_password.clone();
											let user_id = user.id.clone();
											let user_role = user.role.clone();

											view! {
												<TableRow>
													<TableCell>{user.id}</TableCell>
													<TableCell>
														<Input value=email />
													</TableCell>
													<TableCell>{current_role}</TableCell>
													<TableCell>{user.created_at}</TableCell>
													<TableCell>
														<div class="flex gap-2">
															<Button on_click=move |_| {
																let r = if format!("{:?}", user_role) == "Admin" {
																	"user"
																} else {
																	"admin"
																};
																update_action(user_id.clone(), r.to_string(), email.get())
															}>"Toggle Role / Update Email"</Button>
															<Button on_click=move |_| {
																reset_action(email.get())
															}>"Reset Password"</Button>
														</div>
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
