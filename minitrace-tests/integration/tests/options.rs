// Function naming convention: `<category>_<environment>()`
// Where:
// - `environment`:
//   - `indev`: When iterating on a single test case, you are expected to
//              edit the funrtion to point to the test case in development.
//   - `dev`: Generate all missing `*.expanded.rs` files, and flag changes.
//   - `ci`: Generate nothing, and fail on mismatches.
//