use std::env;

fn main() {
    let mut args = env::args().skip(1);
    let function = args.next().expect("function");
    let a: i8 = args.next().expect("a").parse().expect("i8 a");
    let b: i8 = args.next().expect("b").parse().expect("i8 b");
    let value = match function.as_str() {
        "saturating_add" => i128::from(a.saturating_add(b)),
        "widening_mul" => i128::from(i16::from(a) * i16::from(b)),
        other => panic!("unsupported control function {other}"),
    };
    println!("{{\"status\":\"returned\",\"value\":{value}}}");
}
