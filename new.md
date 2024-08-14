example-component/src/main.rs:
# 20 33*18:  0:#[allow(warnings)]
# 12 100*11: 1:mod bindings;
-:           2:
-:           3:fn fizzbuzz(num: u32) -> String {
# 1 -:       4:    if num % 3 == 0 && num % 5 == 0 {
# 2 32:      5:        "fizz buzz".into()
# 1 6:       6:    } else if num % 3 == 0 {
# 1 94:      7:        "fizz".into()
# 1 27:      8:    } else if num % 5 == 0 {
# 1 67:      9:        "buzz".into()
# 1 14:      10:    } else {
-:           11:        num.to_string()
# 1 53:      12:    }
-:           13:}
# 1 100:     14:
-:           15:fn main() {
# 1 -:       16:    let end = 100;
# 1 1:       17:    for num in 1..=end {
# 3 25*1:    18:        println!("{}", fizzbuzz(num));
# 2 33:      19:    }
-:           20:}
# 1 1:       21: