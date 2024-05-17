fn main() {
    if cfg!(feature = "esp32c3") { 
        println!("cargo:rustc-link-search=ld");
        println!("cargo:rerun-if-changed=ld/esp32c3.x");
        println!("cargo:rerun-if-changed=ld/linkall.x");
        println!("cargo:rerun-if-changed=ld/memory.x");
        println!("cargo:rerun-if-changed=ld/rom-functions.x");
    }
}
