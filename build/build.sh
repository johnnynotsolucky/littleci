#!/bin/bash
set -e

(cd swagger/ && npm ci && npm run build)

./build/docker/build-images.sh

cross build --target x86_64-unknown-linux-gnu --release

# TODO
# cross build --target x86_64-pc-windows-gnu --release
