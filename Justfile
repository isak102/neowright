fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test

install:
	cargo install --path . && neowright skills install

release part:
	nix run nixpkgs#bump-my-version -- bump {{part}}

verify: fmt lint test
