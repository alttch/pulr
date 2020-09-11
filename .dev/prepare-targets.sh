#!/bin/sh

rustup target add x86_64-unknown-linux-musl
rustup target add arm-unknown-linux-musleabihf
rustup target add x86_64-pc-windows-gnu
rustup toolchain install stable-x86_64-pc-windows-gnu
