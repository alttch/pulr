VERSION=$(shell awk '/^version/{print substr($$3, 2, length($$3)-2)}' Cargo.toml)
GITHUB_TOKEN=$(shell cat ~/.keys/github)

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
	curl -H "Authorization: token ${GITHUB_TOKEN}" \
		-H "Content-Type: application/x-executable" \
		--data-binary @/tmp/pulr.x86_64.gz \
		"https://uploads.github.com/repos/hubot/singularity/releases/v${VERSION}/assets?name=pulr.x86_64.gz"
