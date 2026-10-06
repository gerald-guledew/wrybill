//! Hands the name of the target being built to the code, so `wrybill doctor`
//! can say which build it is. On Linux the name tells `musl` from `gnu`.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_else(|_| "an unknown target".to_owned());
    println!("cargo::rustc-env=WRYBILL_BUILD_TARGET={target}");
    println!("cargo::rerun-if-changed=build.rs");
}
