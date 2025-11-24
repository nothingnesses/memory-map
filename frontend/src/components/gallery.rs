use leptos::prelude::*;
use thaw::*;

#[component]
pub fn Gallery() -> impl IntoView {
	let open = RwSignal::new(false);
	view! {
		<Button on_click=move |_| open.set(true)>"Open Dialog"</Button>
		<Dialog open>
			<DialogSurface>
				<DialogBody>
					<DialogTitle>"Dialog title"</DialogTitle>
					<DialogContent>"Dialog body"</DialogContent>
					<DialogActions>
						<Button appearance=ButtonAppearance::Primary>"Do Something"</Button>
					</DialogActions>
				</DialogBody>
			</DialogSurface>
		</Dialog>
	}
}
