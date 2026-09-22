use reexports_app::Order;

fn main() {
    let mut order = Order::new(1);
    let total = reexports_app::place(&mut order).unwrap();
    println!("{total}");
    std::process::exit(0);
}
