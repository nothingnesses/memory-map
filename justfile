default:
    nix run ./devenv

# Valid targets are: memory-map-backend memory-map-frontend
watch target:
    bacon run -- --bin {{target}}

serve:
    cd memory-map-frontend; trunk serve --open
