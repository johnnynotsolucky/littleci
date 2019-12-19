#!/bin/bash
set -e

# TODO build the correct branch when necessary
git pull

rustup component add rustfmt
cargo +nightly fmt --all -- --check

(cd swagger/ && npm ci && npm run build)

./build/docker/build-images.sh

cross build --target x86_64-unknown-linux-gnu --release
cross build --target arm-unknown-linux-gnueabihf --release

# TODO
# cross build --target x86_64-pc-windows-gnu --release
