#!/bin/bash
set -e
cd $HOME/warm-code-cov/example-component
cargo component b
cd $HOME/warm-code-cov/wat-annotator
cargo b --release
NEW=$(wasm-tools print $HOME/warm-code-cov/example-component/target/wasm32-wasi*/debug/example-component.wasm | $HOME/warm-code-cov/wat-annotator/target/release/wat-annotator)
echo "${NEW}"
cd ..
echo "${NEW}" > modified.wat
wasm-tools component wit modified.wat -o modified-world.wit
wasm-tools component embed modified-world.wit --world root:component/root modified.wat -o modified.wasm
wasm-tools validate --features component-model modified.wasm
