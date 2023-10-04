#!/bin/sh

# needs: 
#   wine6 <
#   binfmt_misc for wine
#   mingw-w64-gcc
#   rustup target add x86_64-pc-windows-gnu
#   texconv.exe in cwd
set -x
WINEPATH=$(winepath -w $(pwd)) cargo test --all --target=x86_64-pc-windows-gnu --verbose