fn main() {
    println!("binaries may print");
    if std::env::args().count() > 5 {
        std::process::exit(2);
    }
}
