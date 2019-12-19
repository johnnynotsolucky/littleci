#!/bin/bash

# docker build -t littleci/cross:x86_64-unknown-linux-gnu -f build/docker/Dockerfile.x86_64-unknown-linux-gnu ./build/docker/
# docker build -t littleci/cross:x86_64-pc-windows-gnu -f build/docker/Dockerfile.x86_64-pc-windows-gnu ./build/docker/
docker build -t littleci/cross:arm-unknown-linux-gnueabihf -f build/docker/Dockerfile.arm-unknown-linux-gnueabihf ./build/docker/

