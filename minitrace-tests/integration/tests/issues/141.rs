// Move this test to the regression suite when issue #141 is resolved.
//
// Note this integration test for issue #141, which will move to the regression
// suite when resolved, is blocked by issue #137.  In turn issue #137 is blocked
// by [macrotest issue 74](https://github.com/eupn/macrotest/issues/74). This in
// turn appears to be due to [Cargo issue
// #4942](https://github.com/rust-lang/cargo/issues/4942).  Consequently,
// depending on whether Cargo resolve this issue or declare it a 'feature' it is
// possible that the workaround described
// [here](https://github.com/rust-lang/cargo/issues/4942#issuecomment-357729844)
// could be a fix.
//
// If that is not a fix, the next step is to reorganise the workspace from
// 'virtual' to 'real' - which requires moving sources around....
use minitrace::trace;

#[trace]
fn f() {}

fn main() {
    let (root, collector) = minitrace::Span::root("root");
    {
        let _g = root.set_local_parent();
        f();
    }
    drop(root);
    let records: Vec<minitrace::collector::SpanRecord> =
        futures::executor::block_on(collector.collect());
}
