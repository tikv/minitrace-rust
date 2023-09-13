#[macro_export]
pub mod defaults;

#[derive(Debug)]
pub struct IntegrationTest {
    pub name: &'static str,
    pub test_fn: fn(),
    pub indev: Option<bool>,
}

inventory::collect!(IntegrationTest);

// #[cfg(feature = "tk")]
// #[cfg_attr(feature = "tk", macro_export)]
// macro_rules! main_runtime2 {
//     () => {
//         // tokio runtime 2 here
//     };
//     ( $( $x:expr ),+ ) => {{
//         $x
//         // tokio runtime 2 again
//     }};
// }
