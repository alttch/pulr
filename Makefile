VERSION=$(shell awk '/^version/{print substr($$3, 2, length($$3)-2)}' Cargo.toml)

all: debug

clean:
	find . -type d -name target -exec rm -rf {} \; || exit 0
	find . -type f -name Cargo.lock -exec rm -f {} \; || exit 0

debug:
	cargo build

tag:
	git tag -a v${VERSION}
	git push origin --tags

release: release_x86_64 release_armhf

release_x86_64:
	cp -vf build-x86_64.rs build.rs
	cargo build --target x86_64-unknown-linux-musl --release
	strip ./target/x86_64-unknown-linux-musl/release/pulr

release_armhf:
	cp -vf build-arm.rs build.rs
	cargo build --target arm-unknown-linux-musleabihf --release
	/usr/bin/arm-linux-gnueabihf-strip ./target/arm-unknown-linux-musleabihf/release/pulr

release-upload: release-upload-x86_64 release-upload-arm

release-upload-x86_64:
	gzip -c ./target/x86_64-unknown-linux-musl/release/pulr > /tmp/pulr.x86_64-musl.gz
	./.dev/release-upload.sh pulr.x86_64-musl.gz

release-upload-arm:
	gzip -c ./target/arm-unknown-linux-musleabihf/release/pulr > /tmp/pulr.arm-musleabihf.gz
	./.dev/release-upload.sh pulr.arm-musleabihf.gz
