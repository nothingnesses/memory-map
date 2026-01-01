use crate::constants::{MSG_404, TITLE_404};
use leptos::prelude::*;

/// 404 Not Found Page
#[component]
pub fn NotFound() -> impl IntoView {
	view! { <h1>{TITLE_404} <br /> {MSG_404}</h1> }
}
