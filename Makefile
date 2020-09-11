VERSION=1.0.1

all: debug

clean:
	find . -type d -name target -exec rm -rf {} \; || exit 0
	find . -type f -name Cargo.lock -exec rm -f {} \; || exit 0
	rm -f /tmp/pulr.*

debug:
	cp -vf build-x86_64.rs build.rs
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

release: prepare-targets release_x86_64 release_armhf release_win64 check-binaries

prepare-targets:
	./.dev/prepare-targets.sh

release_x86_64:
	cp -vf build-x86_64.rs build.rs
	cargo build --target x86_64-unknown-linux-musl --release
	cd ./tools/ndj2influx && cargo build --target x86_64-unknown-linux-musl --release

release_armhf:
	cp -vf build-armhf.rs build.rs
	cargo build --target arm-unknown-linux-musleabihf --release
	cd ./tools/ndj2influx && cross build --target arm-unknown-linux-musleabihf --release
	/usr/bin/arm-linux-gnueabihf-strip ./tools/ndj2influx/target/arm-unknown-linux-musleabihf/release/ndj2influx

release_win64:
	cp -vf build-win64.rs build.rs
	cargo build --target x86_64-pc-windows-gnu --release
	cd ./tools/ndj2influx && cargo build --target x86_64-pc-windows-gnu --release

check-binaries:
	./.dev/check-binaries.sh

release-upload: release-upload-x86_64 release-upload-arm release-upload-win64

release-upload-x86_64:
	cd ./target/x86_64-unknown-linux-musl/release && \
	 	tar --owner=root --group=root -cvf /tmp/pulr.linux-x86_64-musl.tar pulr
	cd ./tools/ndj2influx/target/x86_64-unknown-linux-musl/release && \
	 	tar --owner=root --group=root -rvf /tmp/pulr.linux-x86_64-musl.tar ndj2influx
	gzip /tmp/pulr.linux-x86_64-musl.tar
	./.dev/release-upload.sh pulr.linux-x86_64-musl.tar.gz
	rm /tmp/pulr.linux-x86_64-musl.tar.gz

release-upload-arm:
	cd ./target/arm-unknown-linux-musleabihf/release && \
	 	tar --owner=root --group=root -cvf /tmp/pulr.linux-arm-musleabihf.tar pulr
	cd ./tools/ndj2influx/target/arm-unknown-linux-musleabihf/release && \
	 	tar --owner=root --group=root -rvf /tmp/pulr.linux-arm-musleabihf.tar ndj2influx
	gzip /tmp/pulr.linux-arm-musleabihf.tar
	./.dev/release-upload.sh pulr.linux-arm-musleabihf.tar.gz
	rm /tmp/pulr.linux-arm-musleabihf.tar.gz

release-upload-win64:
	rm -f /tmp/pulr.windows-x86_64.zip
	cd ./target/x86_64-pc-windows-gnu/release && \
	 	zip /tmp/pulr.windows-x86_64.zip pulr.exe
	cd ./tools/ndj2influx/target/x86_64-pc-windows-gnu/release && \
	 	zip /tmp/pulr.windows-x86_64.zip ndj2influx.exe
	cd /opt/libplctag/win_x64 && \
		zip /tmp/pulr.windows-x86_64.zip plctag.dll
	./.dev/release-upload.sh pulr.windows-x86_64.zip
	rm /tmp/pulr.windows-x86_64.zip
