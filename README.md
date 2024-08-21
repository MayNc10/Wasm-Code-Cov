# wcov
`wcov` is a tool for generating code coverage reports from Wasm components and modules. This output comes in the form of gcov .gcda/.gcno files or lcov .info files, which can be further processed by tools like `genhtml` for easier viewing. Because `wcov` modifies already built Wasm files, no extra compilation libraries or tools are needed (in fact, you don't even need access to the source files to run it). 
## Installation
`wcov` can be installed with Cargo, the Rust package manager/general utility, by running 
`cargo install --git https://github.com/MayNc10/Wasm-Code-Cov wcov`

## Usage
First, compile a Wasm component or module with DWARF debugging information included. This information is necessary for `wcov` to map the compiled code back to source, and without it coverage reports can't be generated. 
Next, run `wcov -p <WASM_FILE> -b <BUILD_DIR> -o <SRC_FILES_TO_OUTPUT>`. <WASM_FILE> is a path to the Wasm component or module to test coverage for, <BUILD_DIR> is a directory for `wcov` to place its output in (which can be your current directory, depending on user preference), and <SRC_FILES_TO_OUTPUT> is a list of paths to source files to output. These source files must be part of the Wasm component being tested. `wcov` will output Lcov info files corresponding to the source files. 
Finally, use a tool like `genhtml` to create a nice visualization of the coverage information. 

## Development Goals
### Use Cases
- [x] Support basic Wasm components
- [ ] Supports more complicated Wasm components
- [ ] Supports nested components
- [ ] Supports Wasm modules  
### Output
- [x] Outputs .gcov files
- [x] Outputs Lcov .info files
    - [ ] Outputs Lcov branch information
- [ ] Outputs .gcda files
- [ ] Outputs .gcno files
### User Experience
- [x] Offer install from Github
- [ ] Publish as crate and offer installation from Crates.io 