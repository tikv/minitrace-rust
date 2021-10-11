use minitrace::*;
use minitrace_jaeger::Reporter;
use std::net::SocketAddr;

fn main() {
    let collector = {
        let (root_span, collector) = Span::root("root".to_owned());
        let _span_guard = root_span.enter();

        let _local_span_guard = LocalSpan::enter("child".to_owned());

        // do something ...
        collector
    };

    let spans: Vec<span::Span> = collector.collect();

    let socket = SocketAddr::new("127.0.0.1".parse().unwrap(), 6831);

    const TRACE_ID: u64 = 42;
    const SPAN_ID_PREFIX: u32 = 42;
    const ROOT_PARENT_SPAN_ID: u64 = 0;
    let bytes = Reporter::encode(
        String::from("service name"),
        TRACE_ID,
        ROOT_PARENT_SPAN_ID,
        SPAN_ID_PREFIX,
        &spans,
    )
    .expect("encode error");
    Reporter::report(socket, &bytes).expect("report error");
}
