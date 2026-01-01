use frontend::{App, AppConfig, constants::*};
use leptos::prelude::*;

fn main() {
	// set up logging
	_ = console_log::init_with_level(log::Level::Debug);
	console_error_panic_hook::set_once();

	mount_to_body(|| {
		let config_resource = LocalResource::new(|| async {
			let origin = window().location().origin().unwrap_or_else(|_| {
				option_env!("FRONTEND_URL").unwrap_or(DEFAULT_FRONTEND_URL).to_string()
			});
			let url = format!("{origin}{CONFIG_FILE_PATH}");
			match reqwest::get(&url).await {
				Ok(resp) => match resp.json::<AppConfig>().await {
					Ok(config) => Some(config),
					Err(e) => {
						log::error!("{MSG_FAILED_TO_PARSE_CONFIG}: {e:?}");
						None
					}
				},
				Err(e) => {
					log::error!("{MSG_FAILED_TO_FETCH_CONFIG}: {e:?}");
					None
				}
			}
		});

		view! {
			<Suspense fallback=|| view! { <p>{MSG_LOADING_CONFIG}</p> }>
				{move || {
					match config_resource.get() {
						Some(Some(config)) => {
							provide_context(config);
							view! { <App /> }.into_any()
						}
						Some(None) => {
							view! {
								<div class="p-20px font-[sans-serif] font-red">
									<h1>{TITLE_FAILED_LOAD_CONFIG}</h1>
									<p>{MSG_CHECK_CONSOLE}</p>
								</div>
							}
							.into_any()
						}
						None => view! { <p>{MSG_LOADING_CONFIG}</p> }.into_any(),
					}
				}}
			</Suspense>
		}
	});
}
