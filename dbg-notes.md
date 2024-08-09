locations of our code:
- 5018 (%Ok("i32.const ")) /home/may/warm-code-cov/example-component/src/main.rs:1:1
- 501a (%Ok("2.const 0\n")) /home/may/warm-code-cov/example-component/src/main.rs:0:1

`./modify_wasm.sh 2>&1 >/dev/null | grep "example-component"` grep stderr output

from the wasm dwarf spec:

```Note: It is expected that a DWARF consumer does not know how to decode WebAssembly instructions. The instruction pointer is selected as the offset in the binary file of the first byte of the instruction, and it is consistent with the WebAssembly Web API §conventions definition of the code location.```

We should confirm that the debug locations are not changed from the wasm file when we run `wasm-print`.

`wasm-tools objdump` seems to give the same hex offsets for both .wasm and .wat, and identifies code sections in the webassembly module. I'm curious what's going on here.


method:
- `wasm-tools print -g -p` to get binary offset comments
- get the location of the code section as a binary offset somehow, maybe using `wasm-tools objdump`
- use the binary offsets from dwarf + the offset of the code section to get offsets into the wat source
- use that to map wat ranges to source lines (read dwarf spec on this)
- counter calls will return their source location
- use that to build gcov output

to get the source triplet for a line, search for the closest recorded pc that comes before the line pc.
