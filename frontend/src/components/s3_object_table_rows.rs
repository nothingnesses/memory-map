use crate::{
	CallbackAnyView,
	graphql_queries::s3_objects::s3_objects_query::S3ObjectsQueryS3Objects as S3Object,
};
use leptos::{logging::debug_log, prelude::*, web_sys::MouseEvent};
use leptos_router::components::*;
use thaw::*;

fn delete_objects(objects: Vec<S3Object>) {}

#[component]
pub fn S3ObjectTableRows(
	#[prop(into)] s3_objects: Signal<Vec<S3Object>>,
	#[prop(into, default = Callback::new(|_| "Close".into_any()))]
	close_button_content: CallbackAnyView,
) -> impl IntoView {
	let open_delete = RwSignal::new(false);
	let open_delete_object_dialog = move |event: MouseEvent| {
		open_delete.set(true);
	};
	let selected_objects = RwSignal::new(vec![]);
	view! {
		<ConfigProvider>
			<ForEnumerate
				each=move || s3_objects.get()
				key=|s3_object| s3_object.id.clone()
				let(s3_object_index,
				s3_object)
			>
				<TableRow>
					<TableCell>"@todo: Add checkboxes"</TableCell>
					<TableCell>{s3_object.id}</TableCell>
					<TableCell>{s3_object.name}</TableCell>
					<TableCell>{s3_object.made_on}</TableCell>
					<TableCell>
						{s3_object
							.location
							.map(|location| {
								format!("{}, {}", location.latitude, location.longitude)
							})}
					</TableCell>
					<TableCell>
						<A href=s3_object.url>"Click"</A>
					</TableCell>
					<TableCell>{s3_object.content_type}</TableCell>
					<TableCell>
						<div>
							<Button on_click=open_delete_object_dialog>"Delete"</Button>
						</div>
					</TableCell>
				</TableRow>
			</ForEnumerate>
			<Dialog open=open_delete>
				<DialogSurface>
					<DialogBody>
						<DialogContent>
							<div class="relative grid justify-items-center group">
								<Button on_click=move |_| {
									open_delete.set(false);
								}>{close_button_content.run(())}</Button>
								<div>
									<h2>
										"Are you sure you want to delete "
										{selected_objects
											.get()
											.iter()
											.map(|s3_object: &S3Object| {
												format!("\"{}\"", s3_object.name)
											})
											.collect::<Vec<_>>()
											.join(", ")}"?"
									</h2>
									<div>
										<Button on_click=move |_| {
											delete_objects(selected_objects.get());
											open_delete.set(false);
										}>"Yes"</Button>
										<Button on_click=move |_| {
											open_delete.set(false);
										}>"No"</Button>
									</div>
								</div>
							</div>
						</DialogContent>
					</DialogBody>
				</DialogSurface>
			</Dialog>
		</ConfigProvider>
	}
}
