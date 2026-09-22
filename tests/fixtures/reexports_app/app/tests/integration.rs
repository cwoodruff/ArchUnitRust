use reexports_app::*;

#[test]
fn places_an_order() {
    let mut order = Order::new(1);
    assert_eq!(place(&mut order).unwrap(), 1);
}
