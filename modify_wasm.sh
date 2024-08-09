#!/bin/bash
set -e
cd $HOME/warm-code-cov/
cargo b --release
NEW=$(wasm-tools print -g -p $1 | $HOME/warm-code-cov/target/debug/wat-annotator --data-output-path data.json)

echo "${NEW}" > modified.wat
echo "Embedding WAT"
wasm-tools component wit modified.wat -o modified-world.wit
wasm-tools component embed modified-world.wit --world root:component/root modified.wat -o modified.wasm
wasm-tools validate --features component-model modified.wasm
