VERSION=1.0.0

all: debug

clean:
	find . -type d -name target -exec rm -rf {} \; || exit 0
	find . -type f -name Cargo.lock -exec rm -f {} \; || exit 0

debug:
	cargo build

tag:
	git tag -a v${VERSION}
	git push origin --tags

pub:
	@# internal
	jks build pulr

ver:
	sed -i 's/^version = ".*/version = "${VERSION}"/g' Cargo.toml
	sed -i 's/^const VERSION.*/const VERSION: \&str = "${VERSION}";/g' src/main.rs

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
	cd ./target/x86_64-unknown-linux-musl/release && tar czvf /tmp/pulr.x86_64-musl.tgz pulr
	./.dev/release-upload.sh pulr.x86_64-musl.tgz

release-upload-arm:
	cd ./target/arm-unknown-linux-musleabihf/release && tar czvf /tmp/pulr.arm-musleabihf.tgz pulr
	./.dev/release-upload.sh pulr.arm-musleabihf.tgz
