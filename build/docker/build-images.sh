#!/bin/bash

docker build -t johnnynotsolucky/cross:x86_64-unknown-linux-gnu -f build/docker/Dockerfile.x86_64-unknown-linux-gnu ./build/docker/
docker build -t johnnynotsolucky/cross:x86_64-pc-windows-gnu -f build/docker/Dockerfile.x86_64-pc-windows-gnu ./build/docker/

