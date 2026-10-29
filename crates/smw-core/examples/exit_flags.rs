//! cargo run -p smw-core --example exit_flags -- <rom>
//! Count the flag nibbles screen exits use across all levels.
use std::collections::BTreeMap;

use smw_core::project::Project;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut p = Project::open(a[1].clone().into()).unwrap();
    let mut counts: BTreeMap<u8, usize> = BTreeMap::new();
    for l in 0..0x200u16 {
        for x in p.exit_settings(l).unwrap_or_default() {
            *counts.entry(x).or_default() += 1;
        }
    }
    println!("{counts:02X?}");
}
