use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=ESPERRA_UPDATE_PUBLIC_KEY");
    let key = env::var("ESPERRA_UPDATE_PUBLIC_KEY").unwrap_or_else(|_| "00".repeat(32));
    assert!(key.len() == 64 && key.bytes().all(|value| value.is_ascii_hexdigit()), "ESPERRA_UPDATE_PUBLIC_KEY must contain 32 hex-encoded bytes");

    let bytes = key.as_bytes().chunks_exact(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).map(|value| value.to_string()).collect::<Vec<_>>().join(", ");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("update_public_key.rs");
    fs::write(output, format!("[{bytes}]\n")).unwrap();
}
