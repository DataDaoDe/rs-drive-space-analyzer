use dsa_core::traverse::{traverse, TraverseConfig};

fn main() {
    let root = std::env::args().nth(1).expect("usage: dsa-cli <path>");
    let cfg = TraverseConfig::default();

    for ev in traverse(root, &cfg) {
        println!("{ev:?}");
    }
}