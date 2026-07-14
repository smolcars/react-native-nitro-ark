use std::path::PathBuf;

fn resolved_bark_wallet_version() -> String {
    let lockfile_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock");
    let lockfile = std::fs::read_to_string(&lockfile_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", lockfile_path.display()));
    let lockfile: toml::Value = toml::from_str(&lockfile)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", lockfile_path.display()));
    let packages = lockfile
        .get("package")
        .and_then(toml::Value::as_array)
        .expect("Cargo.lock is missing its package list");
    let bark_packages: Vec<_> = packages
        .iter()
        .filter(|package| package.get("name").and_then(toml::Value::as_str) == Some("bark-wallet"))
        .collect();

    assert_eq!(
        bark_packages.len(),
        1,
        "expected exactly one resolved bark-wallet package in Cargo.lock"
    );

    bark_packages[0]
        .get("version")
        .and_then(toml::Value::as_str)
        .expect("resolved bark-wallet package is missing its version")
        .to_owned()
}

fn main() {
    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/cxx.rs");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!(
        "cargo:rustc-env=BARK_WALLET_VERSION={}",
        resolved_bark_wallet_version()
    );

    cxx_build::bridge("src/cxx.rs")
        .flag_if_supported("-std=c++17")
        .compile("arkcxxbridge");
}
