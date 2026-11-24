//! Embeds the Typst packages of `packages/preview/<name>/<version>/…`
//! (CeTZ and its dependencies, for figures) as a table of
//! `(name, version, path, bytes)`, so that the engine serves
//! `@preview/<name>:<version>` imports without a registry.
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages/preview");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut entries = String::new();
    if let Ok(names) = std::fs::read_dir(&root) {
        for name in names.flatten().filter(|e| e.path().is_dir()) {
            for version in std::fs::read_dir(name.path()).into_iter().flatten().flatten() {
                let base = version.path();
                let mut files = vec![];
                walk(&base, &mut files);
                files.sort();
                for file in files {
                    let rel = file.strip_prefix(&base).unwrap().to_string_lossy().replace('\\', "/");
                    entries.push_str(&format!(
                        "    ({:?}, {:?}, {:?}, include_bytes!({:?}).as_slice()),\n",
                        name.file_name().to_string_lossy(),
                        version.file_name().to_string_lossy(),
                        format!("/{rel}"),
                        file.display().to_string()
                    ));
                }
            }
        }
    }
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("packages.rs");
    std::fs::write(out, format!("pub static PACKAGES: &[(&str, &str, &str, &[u8])] = &[\n{entries}];\n")).unwrap();
}

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            println!("cargo:rerun-if-changed={}", p.display());
            out.push(p);
        }
    }
}
