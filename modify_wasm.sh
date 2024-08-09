#!/bin/bash
set -e
cd $HOME/warm-code-cov/
cargo b --release
NEW=$(wasm-tools print -g -p $1 | $HOME/warm-code-cov/target/release/wat-annotator --data-output-path data.json $2)

echo "${NEW}" > modified.wat
if [[ "$2" == "-v" ]] || [[ "$2" == "--verbose" ]]
then
echo "Embedding WAT"
fi
wasm-tools component wit modified.wat -o modified-world.wit
wasm-tools component embed modified-world.wit --world root:component/root modified.wat -o modified.wasm
wasm-tools validate --features component-model modified.wasm
