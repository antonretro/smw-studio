//! cargo run -p smw-core --example toolbox_add -- <zip>... [--creator "<pack name>=<who>"]...
//! Add zips to the user's Toolbox (%APPDATA%\SMW Studio\Toolbox) and optionally set credits.
use smw_core::toolbox;

fn main() {
    let lib = toolbox::default_dir();
    std::fs::create_dir_all(&lib).unwrap();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut credits = Vec::new();
    let mut zips = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--creator" {
            let v = it.next().unwrap();
            let (name, who) = v.split_once('=').unwrap();
            credits.push((name.to_string(), who.to_string()));
        } else {
            zips.push(a);
        }
    }
    let existing: Vec<String> = toolbox::list(&lib).into_iter().map(|p| toolbox::tidy_name(&p.name)).collect();
    for z in zips {
        let name = toolbox::tidy_name(&std::path::Path::new(&z).file_stem().unwrap().to_string_lossy());
        if existing.contains(&name) {
            println!("already in Toolbox: {name}");
            continue;
        }
        match toolbox::import_zip(&lib, std::path::Path::new(&z)) {
            Ok(p) => println!("added: {} [{}]{}", p.name, p.kind, if p.creator.is_empty() { String::new() } else { format!(" by {}", p.creator) }),
            Err(e) => println!("FAILED {z}: {e}"),
        }
    }
    for mut p in toolbox::list(&lib) {
        if let Some((_, who)) = credits.iter().find(|(n, _)| toolbox::tidy_name(n) == toolbox::tidy_name(&p.name)) {
            p.creator = who.clone();
            toolbox::save(&lib, &p).unwrap();
            println!("credit: {} -> {who}", p.name);
        }
    }
    println!("Toolbox at {}", lib.display());
}
