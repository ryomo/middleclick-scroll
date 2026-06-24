// Temporary probe: prints what `devices::enumerate_mice()` returns.
// Run with: `cargo run --example list_mice`
#[path = "../src/util.rs"]
mod util;
#[path = "../src/devices.rs"]
mod devices;

fn main() {
    let mice = devices::enumerate_mice();
    println!("found {} mouse device(s)\n", mice.len());
    for (i, d) in mice.iter().enumerate() {
        // {:?} (Debug) escapes control chars, so a literal "\0" vs a real NUL is visible.
        println!("[{i}]");
        println!("  handle: {:#x}", d.handle);
        println!("  path:   {:?}", d.path);
        println!("  name:   {:?}", d.name);
        println!();
    }
}
