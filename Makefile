all:
	@echo "select target"

debug:
	cargo build

release:
	cargo build --target x86_64-unknown-linux-musl --release
	strip ./target/x86_64-unknown-linux-musl/release/pulr
