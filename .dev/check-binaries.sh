#!/usr/bin/env bash

CHECK_FILES=(
  "target/x86_64-unknown-linux-musl/release/pulr"
  "target/arm-unknown-linux-musleabihf/release/pulr"
  "target/x86_64-pc-windows-gnu/release/pulr.exe"
  "tools/ndj2influx/target/x86_64-unknown-linux-musl/release/ndj2influx"
  "tools/ndj2influx/target/arm-unknown-linux-musleabihf/release/ndj2influx"
  "tools/ndj2influx/target/x86_64-pc-windows-gnu/release/ndj2influx.exe"
)

for f in ${CHECK_FILES[@]}; do
  echo -n "$f "
  if [[ $f == *"target/arm-"* ]]; then
    file $f | grep "statically linked, stripped$" > /dev/null || exit 1
  elif [[ $f == *"target/x86_64-pc-windows-"* ]]; then
    file $f | grep "(stripped to external PDB)" > /dev/null || exit 1
  else
    ldd $f | grep -E "statically linked|not a dynamic executable" > /dev/null || exit 1
    file $f | grep ", stripped$" > /dev/null || exit 1
  fi
  echo "OK"
done

echo "FILES CHECKED"
