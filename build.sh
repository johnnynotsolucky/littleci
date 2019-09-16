#!/bin/bash
set -e

./build/docker/bsuild-images.sh

cross build --target x86_64-unknown-linux-gnu --release

# TODO
# cross build --target x86_64-pc-windows-gnu --release
