#!/bin/bash

docker build -t littleci/cross:x86_64-unknown-linux-gnu -f scripts/build/docker/Dockerfile.x86_64-unknown-linux-gnu ./scripts/build/docker/
# docker build -t littleci/cross:x86_64-pc-windows-gnu -f scripts/build/docker/Dockerfile.x86_64-pc-windows-gnu ./scripts/build/docker/
# docker build -t littleci/cross:arm-unknown-linux-gnueabihf -f scripts/build/docker/Dockerfile.arm-unknown-linux-gnueabihf ./scripts/build/docker/

