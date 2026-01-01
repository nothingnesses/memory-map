use crate::{
	components::location_markers::LocationMarkers,
	constants::{
		MAP_INITIAL_LAT, MAP_INITIAL_LNG, MAP_INITIAL_ZOOM, MAP_TITLE, TILE_LAYER_ATTRIBUTION,
		TILE_LAYER_URL,
	},
	dump_errors,
};
use leptos::prelude::*;
use leptos_leaflet::prelude::*;

/// Default Home Page
#[component]
pub fn Home() -> impl IntoView {
	view! {
		<ErrorBoundary fallback=dump_errors>
			<div class="relative w-dvw">
				<div class="container mx-auto grid gap-4">
					<h1 class="text-22px font-bold">{MAP_TITLE}</h1>
					<MapContainer
						class="w-full h-dvh z-0"
						center=Position::new(MAP_INITIAL_LAT, MAP_INITIAL_LNG)
						zoom=MAP_INITIAL_ZOOM
						set_view=true
					>
						<TileLayer url=TILE_LAYER_URL attribution=TILE_LAYER_ATTRIBUTION />
						<LocationMarkers />
					</MapContainer>
				</div>
			</div>
		</ErrorBoundary>
	}
}
