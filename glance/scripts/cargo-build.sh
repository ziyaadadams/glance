#!/bin/bash
# Build script for Cargo within Meson

SOURCE_ROOT="$1"
BUILD_ROOT="$2"
OUTPUT="$3"
BUILD_TYPE="$4"

cd "$SOURCE_ROOT"

if [ "$BUILD_TYPE" = "release" ]; then
    cargo build --release
    cp target/release/glance "$OUTPUT"
else
    cargo build
    cp target/debug/glance "$OUTPUT"
fi
