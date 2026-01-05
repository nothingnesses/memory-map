#![allow(dead_code)]
use crate::auth::UserContext;
use crate::errors::use_context_safe;
use crate::graphql_queries::me::UserRole;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

#[component]
pub fn ProtectedRoute(
	#[allow(dead_code)] children: ChildrenFn,
	#[prop(default = false)]
	#[allow(dead_code)]
	admin_only: bool,
) -> impl IntoView {
	let user_ctx = match use_context_safe::<UserContext>("UserContext") {
		Some(c) => c,
		None => return view! { "System Error: User context missing" }.into_any(),
	};
	let navigate = use_navigate();

	view! {
		<Suspense fallback=|| view! { "Loading..." }>
			{move || {
				let user_opt = user_ctx.user.get();
				let navigate = navigate.clone();
				match user_opt {
					Some(Some(user)) => {
						if admin_only && user.role != UserRole::Admin {
							request_animation_frame(move || {
								navigate("/403", Default::default());
							});
							().into_any()
						} else {
							children().into_any()
						}
					}
					Some(None) => {
						request_animation_frame(move || {
							navigate("/sign-in", Default::default());
						});
						().into_any()
					}
					None => ().into_any(),
				}
			}}
		</Suspense>
	}
	.into_any()
}
