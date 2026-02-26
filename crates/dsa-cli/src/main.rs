use dsa_core::traverse::{traverse, TraverseConfig};

fn main() {
    let cfg = TraverseConfig::default();
    for ev in traverse(".", &cfg).take(50) {
        println!("{ev:?}");
    }
}