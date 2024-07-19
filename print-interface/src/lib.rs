#[allow(warnings)]
mod bindings;

use crate::bindings::exports::component::print_interface::printer::Guest;

struct Component;

impl Guest for Component {
    fn print(str: String) {
        print!("{}", str);
    }
    fn println(str: String) {
        println!("{}\n", str);
    }
}

bindings::export!(Component with_types_in bindings);
