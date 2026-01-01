use frontend::{App, AppConfig};
use leptos::prelude::*;

fn main() {
	// set up logging
	_ = console_log::init_with_level(log::Level::Debug);
	console_error_panic_hook::set_once();

	leptos::task::spawn_local(async {
		let result = async {
			let resp = reqwest::get("/config.json").await?;
			let config = resp.json::<AppConfig>().await?;
			Ok::<AppConfig, reqwest::Error>(config)
		}
		.await;

		match result {
			Ok(config) => {
				mount_to_body(move || {
					provide_context(config.clone());
					view! { <App /> }
				});
			}
			Err(e) => {
				log::error!("Failed to load config: {:?}", e);
				mount_to_body(move || {
					view! {
						<div class="p-20px font-[sans-serif] font-red">
							<h1>"Failed to load configuration"</h1>
							<p>"Please check the console for more details."</p>
						</div>
					}
				});
			}
		}
	});
}
