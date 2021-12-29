use minitrace::prelude::*;
use std::net::SocketAddr;

fn main() {
    let collector = {
        let (root_span, collector) = Span::root("root");
        let _span_guard = root_span.set_local_parent();

        let _local_span_guard = LocalSpan::enter_with_local_parent("child");

        // do something ...
        collector
    };

    let spans: Vec<SpanRecord> = collector.collect();

    let socket = SocketAddr::new("127.0.0.1".parse().unwrap(), 6831);

    const TRACE_ID: u64 = 42;
    const SPAN_ID_PREFIX: u32 = 42;
    const ROOT_PARENT_SPAN_ID: u64 = 0;
    let bytes = minitrace_jaeger::encode(
        String::from("service name"),
        TRACE_ID,
        ROOT_PARENT_SPAN_ID,
        SPAN_ID_PREFIX,
        &spans,
    )
    .expect("encode error");
    minitrace_jaeger::report(socket, &bytes).expect("report error");
}
