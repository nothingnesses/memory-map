use crate::auth::UserContext;
use crate::graphql_queries::me::UserRole;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

#[component]
pub fn ProtectedRoute(
	children: ChildrenFn,
	#[prop(default = false)] admin_only: bool,
) -> impl IntoView {
	let user_ctx = use_context::<UserContext>().expect("UserContext missing");
	let navigate = use_navigate();

	view! {
		<Suspense fallback=|| view! { "Loading..." }>
			{move || {
				let user_opt = user_ctx.user.get();
				let navigate = navigate.clone();
				match user_opt {
					Some(Some(user)) => {
						if admin_only && user.role != UserRole::ADMIN {
							request_animation_frame(move || {
								navigate("/403", Default::default());
							});
							view! {}.into_any()
						} else {
							children().into_any()
						}
					}
					Some(None) => {
						request_animation_frame(move || {
							navigate("/sign-in", Default::default());
						});
						view! {}.into_any()
					}
					None => view! {}.into_any(),
				}
			}}
		</Suspense>
	}
}
