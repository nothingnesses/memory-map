use frontend::{App, AppConfig};
use leptos::prelude::*;

fn main() {
	// set up logging
	_ = console_log::init_with_level(log::Level::Debug);
	console_error_panic_hook::set_once();

	leptos::task::spawn_local(async {
		let config = reqwest::get("/config.json")
			.await
			.expect("Failed to fetch config")
			.json::<AppConfig>()
			.await
			.expect("Failed to parse config");

		mount_to_body(move || {
			provide_context(config.clone());
			view! { <App /> }
		});
	});
}
