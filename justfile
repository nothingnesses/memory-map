default:
    nix run ./devenv

# Valid targets are: backend frontend
watch target:
    bacon run -- --bin {{target}}

serve:
    cd frontend; trunk serve --open
