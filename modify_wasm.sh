#!/bin/bash
set -e
cd $HOME/warm-code-cov/
cargo component build -p example-component
#cargo b --release -p wat-annotator
#NEW=$(wasm-tools print $HOME/warm-code-cov/target/wasm32-wasi*/debug/example-component.wasm | $HOME/warm-code-cov/target/release/wat-annotator)

cargo b -p wat-annotator
NEW=$(wasm-tools print -g -p $HOME/warm-code-cov/target/wasm32-wasi*/debug/example-component.wasm | $HOME/warm-code-cov/target/debug/wat-annotator --file-map-output-path file_map.json)

#echo "${NEW}"
echo "${NEW}" > modified.wat
echo "Embedding WAT"
wasm-tools component wit modified.wat -o modified-world.wit
wasm-tools component embed modified-world.wit --world root:component/root modified.wat -o modified.wasm
wasm-tools validate --features component-model modified.wasm
