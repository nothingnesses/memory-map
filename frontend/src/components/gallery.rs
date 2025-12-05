use crate::{
	components::carousel::Carousel, s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
};
use leptos::prelude::*;
use thaw::*;

#[component]
pub fn Gallery(
	#[prop(into, default = Box::new(move || "Open Gallery".into_any()))]
	open_button_content: Children,
	#[prop(into, default = Box::new(move || "Gallery".into_any()))] dialog_title_content: Children,
	#[prop(into)] s3_objects: Signal<Vec<S3Object>>,
) -> impl IntoView {
	let open = RwSignal::new(false);
	view! {
		<ConfigProvider>
			<Button on_click=move |_| { open.set(true) }>{open_button_content()}</Button>
			<Dialog open>
				<DialogSurface>
					<DialogBody class="grid-cols-1">
						<div class="relative w-full grid grid-flow-col justify-between">
							<DialogTitle>{dialog_title_content()}</DialogTitle>
							<Button on_click=move |_| { open.set(false) }>"Close"</Button>
						</div>
						<DialogContent>
							<Carousel s3_objects />
						</DialogContent>
					</DialogBody>
				</DialogSurface>
			</Dialog>
		</ConfigProvider>
	}
}
