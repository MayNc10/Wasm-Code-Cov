#[allow(warnings)]
mod bindings;

fn fizzbuzz(num: u32) -> String {
    if num % 3 == 0 && num % 5 == 0 {
        "fizz buzz".into()
    }
    else if num % 3 == 0 {
        "fizz".into()
    }
    else if num % 5 == 0 {
        "buzz".into()
    }
    else { num.to_string() }

}

fn main() {
    let end = 100;
    for num in 1..=end {
        println!("{}", fizzbuzz(num));
    }
}
