#[macro_export]
macro_rules! main_tokio {
    () => {
        // tokio main here
    };
    ( $( $x:expr ),+ ) => {{
        $x
        // tokio main again
    }};
}
#[cfg(feature = "tk")]
#[cfg_attr(feature = "tk", macro_export)]
macro_rules! main_runtime {
    () => {
        // tokio runtime here
    };
    ( $( $x:expr ),+ ) => {{
        $x
        // tokio runtime again
    }};
}
