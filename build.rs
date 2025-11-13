use std::fs;
fn main() {
    let version_text = fs::read_to_string("VERSION").expect("Failed to read VERSION file");
    for line in version_text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            println!("cargo:rustc-env={}={}", key, value);
        }
    }    
    // This call will make make config entries available in the code for every device tree node, to
    // allow conditional compilation based on whether it is present in the device tree.
    // For example, it will be possible to have:
    // ```rust
    // #[cfg(dt = "aliases::led0")]
    // ```
    zephyr_build::export_bool_kconfig();
    zephyr_build::dt_cfgs();
}
