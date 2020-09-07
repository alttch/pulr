fn main() {
    println!("cargo:rustc-link-lib=static=plctag");
    println!("cargo:rustc-link-search=/opt/libplctag/x86_64");
}
