fn main() {
    // No special linker flags needed; #[no_mangle] extern "C" functions in a
    // cdylib are exported automatically by the Rust toolchain on all platforms.
}
