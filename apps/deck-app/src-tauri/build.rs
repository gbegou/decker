//! Tauri's build step embeds the icon with a resource compiler it looks up
//! as `windres`, a name it does not let us configure. On the GNU Windows
//! toolchain from conda (the no-admin setup), the tool exists only as
//! `x86_64-w64-mingw32-windres.exe`: provide the plain name from a copy in
//! the build directory, on the PATH of this build script and its children.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    if env::var("HOST").map(|h| h.ends_with("windows-gnu")).unwrap_or(false) {
        provide_windres();
    }
    tauri_build::build()
}

fn provide_windres() {
    if Command::new("windres").arg("--version").output().is_ok() {
        return;
    }
    let Some(path) = env::var_os("PATH") else { return };
    let prefixed = env::split_paths(&path).map(|d| d.join("x86_64-w64-mingw32-windres.exe")).find(|p| p.is_file());
    let Some(prefixed) = prefixed else {
        println!(
            "cargo:warning=neither windres nor x86_64-w64-mingw32-windres found on PATH; the icon cannot be embedded"
        );
        return;
    };
    let shim_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("windres-shim");
    if std::fs::create_dir_all(&shim_dir).and_then(|_| std::fs::copy(&prefixed, shim_dir.join("windres.exe"))).is_err()
    {
        println!("cargo:warning=could not copy {} into the build directory", prefixed.display());
        return;
    }
    let new_path = env::join_paths(std::iter::once(shim_dir).chain(env::split_paths(&path))).expect("PATH");
    // Inherited by the resource compiler Tauri spawns from this process.
    env::set_var("PATH", new_path);
}
