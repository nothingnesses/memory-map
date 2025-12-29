use crate::components::header::Header;
use crate::pages::{
	account::Account, admin::users::Users, home::Home, objects::Objects, register::Register,
	reset_password::ResetPassword, sign_in::SignIn,
};
use auth::UserContext;
use graphql_queries::me::MeQuery;
use leptos::{
	ev, html,
	prelude::*,
	wasm_bindgen::JsValue,
	web_sys::{self, js_sys},
};
use leptos_meta::*;
use leptos_router::{components::*, path};
use std::ops::{Add, Deref, Rem, Sub};
use thaw::{ConfigProvider, ToasterProvider};

// Modules
pub mod auth;
mod components;
pub mod graphql_queries;
mod pages;

/// The Shell component wraps the main application content.
/// It manages the global layout state, including the header's visibility on scroll.
#[component]
fn Shell(children: Children) -> impl IntoView {
	let menu_open = RwSignal::new(false);

	let page_wrapper_ref = NodeRef::<html::Div>::new();
	let last_scroll_y = StoredValue::new(0.0);
	let translate_y = StoredValue::new(0.0);
	let is_scrolling = StoredValue::new(false);
	let header_height = 100.0;

	// Updates the header's vertical position based on scroll direction
	let update_header_position = move |val: f64| {
		// Clamp the translation between -header_height (hidden) and 0.0 (fully visible)
		let current = f64::min(f64::max(val, -header_height), 0.0);
		translate_y.set_value(current);

		// Only update the CSS variable if the menu is closed
		if !menu_open.get() {
			if let Some(el) = page_wrapper_ref.get() {
				let _ = el
					.deref()
					.style()
					.set_property("--hide-on-scroll-translate-y", &format!("{}px", current));
			}
		}
	};

	Effect::new(move |_| {
		// Initialize last_scroll_y
		last_scroll_y.set_value(window().scroll_y().unwrap_or(0.0).max(0.0));

		let update_pos = update_header_position.clone();

		// Handle scroll events to hide/show header
		let on_scroll = move |_| {
			if !is_scrolling.get_value() {
				if let Some(el) = page_wrapper_ref.get() {
					// Add 'scrolling' class to disable transitions during active scroll
					let _ = el.deref().class_list().toggle_with_force("scrolling", true);
				}
				is_scrolling.set_value(true);
			}

			let window = window();
			let current_scroll_y = window.scroll_y().unwrap_or(0.0).max(0.0);
			let last = last_scroll_y.get_value();
			let delta = current_scroll_y - last;
			let current_translate = translate_y.get_value();

			// Update position based on scroll delta
			update_pos(current_translate - delta);
			last_scroll_y.set_value(current_scroll_y);
		};

		let update_pos_end = update_header_position.clone();

		// Handle scroll end to snap header to open/closed state
		let on_scroll_end = move |_: web_sys::CustomEvent| {
			is_scrolling.set_value(false);
			if let Some(el) = page_wrapper_ref.get() {
				// Remove 'scrolling' class to re-enable transitions
				let _ = el.deref().class_list().toggle_with_force("scrolling", false);
			}
			let current_translate = translate_y.get_value();
			let hidden_height = -header_height;

			// Snap to nearest state (fully hidden or fully visible)
			let target = if current_translate > hidden_height / 2.0 { 0.0 } else { hidden_height };
			update_pos_end(target);
		};

		let cleanup_scroll = window_event_listener(ev::scroll, on_scroll);
		// 'scrollend' is a newer event, might need polyfill or browser support check in some contexts,
		// but here we assume it's available or handled.
		let cleanup_scrollend = window_event_listener(ev::Custom::new("scrollend"), on_scroll_end);

		on_cleanup(move || {
			drop(cleanup_scroll);
			drop(cleanup_scrollend);
		});
	});

	// Reset header position when menu is opened
	Effect::new(move |_| {
		if menu_open.get() {
			translate_y.set_value(0.0);
			if let Some(el) = page_wrapper_ref.get() {
				let _ = el.deref().style().set_property("--hide-on-scroll-translate-y", "0px");
			}
		}
	});

	view! {
		<div class="relative group/page scroll-smooth" node_ref=page_wrapper_ref>
			<Header menu_open=menu_open />
			<main class="relative pt-150px">{children()}</main>
		</div>
	}
}

/// An app router which renders the homepage and handles 404's
#[component]
pub fn App() -> impl IntoView {
	// Provides context that manages stylesheets, titles, meta tags, etc.
	provide_meta_context();

	let trigger = RwSignal::new(0);
	let user_resource = LocalResource::new(move || {
		trigger.get();
		async move { MeQuery::run().await.ok().flatten() }
	});

	provide_context(UserContext {
		user: user_resource,
		refetch: Callback::new(move |_| trigger.update(|n| *n += 1)),
	});

	view! {
		<ConfigProvider>
			<ToasterProvider>
				<Html attr:lang="en" attr:dir="ltr" attr:data-theme="light" />

				// sets the document title
				<Title text="Memory Map" />

				// injects metadata in the <head> of the page
				<Meta charset="UTF-8" />
				<Meta name="viewport" content="width=device-width, initial-scale=1.0" />

				<Router>
					<Shell>
						<Routes fallback=|| view! { NotFound }>
							<Route path=path!("/") view=Home />
							<Route path=path!("/objects") view=Objects />
							<Route path=path!("/sign-in") view=SignIn />
							<Route path=path!("/register") view=Register />
							<Route path=path!("/account") view=Account />
							<Route path=path!("/reset-password") view=ResetPassword />
							<Route path=path!("/admin/users") view=Users />
						</Routes>
					</Shell>
				</Router>
			</ToasterProvider>
		</ConfigProvider>
	}
}

/// [Copied from here](https://docs.rs/graphql_client/0.14.0/src/graphql_client/reqwest.rs.html#8-17),
/// since we can't initialise a `reqwest::Client` to use with the original version,
/// since `graphql_client` didn't `pub use` their version of `reqwest` for us to `use`.
pub async fn post_graphql<Q: graphql_client::GraphQLQuery, U: reqwest::IntoUrl>(
	client: &reqwest::Client,
	url: U,
	variables: Q::Variables,
) -> Result<graphql_client::Response<Q::ResponseData>, reqwest::Error> {
	let body = Q::build_query(variables);
	let reqwest_response = client.post(url).json(&body).send().await?;

	reqwest_response.json().await
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LocationStrings {
	pub latitude: String,
	pub longitude: String,
}

pub fn dump_errors(errors: ArcRwSignal<Errors>) -> impl IntoView {
	view! {
		<h1>"Uh oh! Something went wrong!"</h1>

		<p>"Errors: "</p>
		// Render a list of errors as strings - good for development purposes
		<ul>
			{move || {
				errors
					.get()
					.into_iter()
					.map(|(_, e)| view! { <li>{e.to_string()}</li> })
					.collect_view()
			}}

		</ul>
	}
}

pub trait ModularAdd {
	fn modular_add(
		self,
		addend: Self,
		maximum: Self,
	) -> Self;
}

impl<T> ModularAdd for T
where
	T: Copy + PartialOrd + Add<Output = T> + Rem<Output = T>,
{
	fn modular_add(
		self,
		addend: Self,
		maximum: Self,
	) -> Self {
		(self + addend) % maximum
	}
}

pub trait ModularSubtract {
	fn modular_subtract(
		self,
		subtrahend: Self,
		maximum: Self,
	) -> Self;
}

impl<T> ModularSubtract for T
where
	T: Copy + PartialOrd + Sub<Output = T> + Rem<Output = T>,
{
	fn modular_subtract(
		self,
		subtrahend: Self,
		maximum: Self,
	) -> Self {
		let effective_subtrahend = subtrahend % maximum;

		if effective_subtrahend > self {
			maximum - (effective_subtrahend - self)
		} else {
			self - effective_subtrahend
		}
	}
}

pub type CallbackAnyView = Callback<(), AnyView>;

/// Converts a date string from an HTML input (e.g. "2023-12-26T14:30")
/// to an ISO 8601 UTC string (e.g. "2023-12-26T14:30:00.000Z").
pub fn js_date_value_to_iso(value: &str) -> Option<String> {
	if value.is_empty() {
		return None;
	}
	let date = js_sys::Date::new(&JsValue::from_str(value));
	// Check for invalid date (NaN timestamp) to prevent panic in to_iso_string
	if date.get_time().is_nan() {
		return None;
	}
	date.to_iso_string().as_string()
}

/// Converts an ISO 8601 UTC string (e.g. "2023-12-26T14:30:00.000Z")
/// to a local datetime string suitable for an HTML input (e.g. "2023-12-26T14:30").
pub fn iso_to_local_datetime_value(iso: &str) -> Option<String> {
	let date = js_sys::Date::new(&JsValue::from_str(iso));
	if date.get_time().is_nan() {
		return None;
	}
	let offset = date.get_timezone_offset() * 60000.0;
	let local_date = js_sys::Date::new(&JsValue::from_f64(date.get_time() - offset));
	local_date.to_iso_string().as_string().map(|s| s.chars().take(16).collect())
}
