VERSION=0.0.3

all:
	cargo build

ver:
	sed -i 's/^version = ".*/version = "${VERSION}"/g' Cargo.toml
	sed -i 's/^const VERSION.*/const VERSION: \&str = "${VERSION}";/g' src/main.rs
