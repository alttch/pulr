fn main() {
    println!("cargo:rustc-link-lib=plctag");
    println!("cargo:rustc-link-search=/opt/libplctag/win_x64");
}
