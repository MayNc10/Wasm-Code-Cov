#!/bin/bash
set -e
cd $HOME/warm-code-cov/example-component
cargo b 
cd $HOME/warm-code-cov/wat-annotator
cargo b --release
NEW=$(wasm-tools print $HOME/warm-code-cov/example-component/target/wasm32-wasi/debug/example-component.wasm | $HOME/warm-code-cov/wat-annotator/target/release/wat-annotator)
echo "${NEW}"
cd ..
echo "${NEW}" > modified.wat