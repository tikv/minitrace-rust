// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

mod common;

fn func1(i: u64) {
    let _guard = minitrace::new_span(0u32);
    std::thread::sleep(std::time::Duration::from_millis(i));
    func2(i);
}

#[minitrace::trace(0u32)]
fn func2(i: u64) {
    std::thread::sleep(std::time::Duration::from_millis(i));
}

fn main() {
    let (root, collector) = minitrace::trace_enable(0u32);
    minitrace::property(b"sample property:it works");
    {
        let _guard = root;
        for i in 1..=10 {
            func1(i);
        }
    }

    let s = collector.collect();

    #[cfg(feature = "jaeger")]
    {
        let mut buf = Vec::with_capacity(2048);
        minitrace::jaeger::thrift_encode(&mut buf, "synchronous_example", &s, |e| e.to_string());
        let agent = std::net::SocketAddr::from(([127, 0, 0, 1], 6831));
        let _ = std::net::UdpSocket::bind(std::net::SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
            0,
        ))
        .and_then(move |s| s.send_to(&buf, agent));
    }

    crate::common::draw_stdout(s);
}
