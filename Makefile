VERSION=$(shell awk '/^version/{print substr($$3, 2, length($$3)-2)}' Cargo.toml)

all:
	@echo "select target"

debug:
	cargo build

tag:
	git tag -a v${VERSION}
	git push origin --tags

release:
	cargo build --target x86_64-unknown-linux-musl --release
	strip ./target/x86_64-unknown-linux-musl/release/pulr

release-upload:
	gzip -c ./target/x86_64-unknown-linux-musl/release/pulr > /tmp/pulr.x86_64.gz
	./.dev/release-upload.sh
