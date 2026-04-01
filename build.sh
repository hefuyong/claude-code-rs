#!/bin/bash
# Build script that ensures MinGW tools are in PATH
SELF_CONTAINED="C:/Users/27425/.rustup/toolchains/stable-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained"
RUSTLIB_BIN="C:/Users/27425/.rustup/toolchains/stable-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin"
CARGO_BIN="C:/Users/27425/.cargo/bin"

export PATH="$SELF_CONTAINED:$RUSTLIB_BIN:$CARGO_BIN:$PATH"
export CARGO_TARGET_DIR="D:/rust-target"

cd "$(dirname "$0")"
exec cargo "$@"
