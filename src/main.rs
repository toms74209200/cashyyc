fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Err(e) = cashyyc::run::run(args) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
