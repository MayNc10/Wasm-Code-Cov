#[allow(warnings)]

fn fizzbuzz(num: u32) -> String {
    if num % 3 == 0 && num % 5 == 0 {
        "fizz buzz".into()
    } else if num % 3 == 0 {
        "fizz".into()
    } else if num % 5 == 0 {
        "buzz".into()
    } else if num % 101 == 0 {
        panic!();
    } else {
        num.to_string()
    }
}

mod bindings;

fn main() {
    let end = 100;
    for num in 1..=end {
        println!("{}", fizzbuzz(num));
    }
}
