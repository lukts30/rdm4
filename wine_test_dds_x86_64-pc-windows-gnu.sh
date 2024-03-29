#!/bin/sh

# needs: 
#   wine6 <
#   binfmt_misc for wine
#   mingw-w64-gcc
#   rustup target add x86_64-pc-windows-gnu
#   texconv.exe in cwd
set -x
WINEPATH=$(winepath -w $(pwd)) cargo test --all --target=x86_64-pc-windows-gnu --verbose


# fd -e rdm --full-path --exclude "*anim*" -x ~/Dokumente/rdm4/target/debug/rdm4-bin --force -i 2> log
# cat log  | grep Error | grep --invert-match -E "Meta|U16"