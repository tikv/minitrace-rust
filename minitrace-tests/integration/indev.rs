pub mod tests;

use tests::IntegrationTest;

fn setup() {
    println!("Setup")
}

fn teardown() {
    println!("Teardown")
}
// NOTE: This function is executed by `cargo test -- --list`.
// Hence we guard it:
#[cfg(any(feature = "as", feature = "tk"))]
fn main() {
    // Setup test environment
    setup();

    // Run the tests
    for t in inventory::iter::<IntegrationTest> {
        if let Some(category) = t.indev {
            (t.test_fn)()
        }
    }

    // Teardown test environment
    teardown();
}
