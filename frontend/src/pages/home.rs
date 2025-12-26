use crate::{components::location_markers::LocationMarkers, dump_errors};
use leptos::prelude::*;
use leptos_leaflet::prelude::*;

/// Default Home Page
#[component]
pub fn Home() -> impl IntoView {
	view! {
		<ErrorBoundary fallback=dump_errors>
			<div class="relative w-dvw">
				<div class="container mx-auto grid gap-4">
					<h1 class="text-22px font-bold">"Map"</h1>
					<MapContainer
						class="w-full h-dvh"
						center=Position::new(51.505, -0.09)
						zoom=3.0
						set_view=true
					>
						<TileLayer
							url="https://tile.openstreetmap.org/{z}/{x}/{y}.png"
							attribution="&copy; <a href=\"https://www.openstreetmap.org/copyright\">OpenStreetMap</a> contributors"
						/>
						<LocationMarkers />
					</MapContainer>
				</div>
			</div>
		</ErrorBoundary>
	}
}
