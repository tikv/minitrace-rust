// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

const BAR_LEN: usize = 70;

pub fn draw_stdout(_trace_details: minitrace::TraceDetails) {
    // let span_sets = trace_details.span_sets;
    // let cycles_per_sec = trace_details.cycles_per_second;
    // let mut children = std::collections::HashMap::new();
    // let mut following = std::collections::HashMap::new();
    // let mut follower_to_header = std::collections::HashMap::new();
    // let mut spans_map = std::collections::HashMap::new();

    // let mut root = None;
    // let mut root_cycles = None;
    // let mut max_end = 0;

    // for span_set in span_sets {
    //     let mut span_set = span_set;
    //     let spans = if span_set.spans.first().unwrap().state != minitrace::State::Root {
    //         span_set.spans[1].related_id = span_set.spans[0].related_id;
    //         &span_set.spans[1..]
    //     } else {
    //         &span_set.spans[..]
    //     };
    //     for span in spans {
    //         let start = span.begin_cycles;
    //         let end = start + span.elapsed_cycles;

    //         if end > max_end {
    //             max_end = end;
    //         }

    //         assert_eq!(
    //             spans_map.insert(span.id, (start, span.elapsed_cycles)),
    //             None,
    //             "duplicated id {:#?}",
    //             span.id
    //         );

    //         follower_to_header.insert(span.id, span.id);

    //         match span.state {
    //             minitrace::State::Root => {
    //                 root = Some(span.id);
    //                 root_cycles = Some(span.begin_cycles);
    //             }
    //             minitrace::State::Local => {
    //                 children
    //                     .entry(span.related_id)
    //                     .or_insert_with(Vec::new)
    //                     .push(span.id);
    //             }
    //             minitrace::State::Settle => {
    //                 dbg!(&span.related_id);
    //                 dbg!(&follower_to_header);
    //                 let header = follower_to_header[&span.related_id];
    //                 follower_to_header.insert(span.id, header);

    //                 following
    //                     .entry(header)
    //                     .or_insert_with(Vec::new)
    //                     .push(span.id);
    //             }
    //             _ => unreachable!(),
    //         }
    //     }
    // }

    // let root = root.expect("can not find root");
    // let root_cycles = root_cycles.unwrap();

    // for (_, (start, _)) in spans_map.iter_mut() {
    //     *start -= root_cycles;
    // }
    // max_end -= root_cycles;

    // if max_end == 0 {
    //     panic!("Insufficient precision");
    // }

    // let factor = BAR_LEN as f64 / max_end as f64;

    // draw_rec(
    //     root,
    //     factor,
    //     cycles_per_sec,
    //     &following,
    //     &children,
    //     &spans_map,
    // );
}

#[allow(dead_code)]
fn draw_rec(
    cur_id: u64,
    factor: f64,
    cycles_per_sec: u64,
    following: &std::collections::HashMap<u64, Vec<u64>>, // id -> [continue/following id]
    children_map: &std::collections::HashMap<u64, Vec<u64>>, // id -> [child_id]
    spans_map: &std::collections::HashMap<u64, (u64, u64)>, // id -> (start, duration)
) {
    let mut ids = vec![];
    let mut span = vec![];
    ids.push(cur_id);
    span.push(*spans_map.get(&cur_id).expect("can not get span"));

    following
        .get(&cur_id)
        .unwrap_or(&Vec::new())
        .iter()
        .for_each(|id| {
            ids.push(*id);
            span.push(*spans_map.get(&id).expect("can not get span"));
        });

    let mut draw_len = 0usize;
    let mut total_cycles = 0u64;

    for (start, duration) in span {
        // draw leading space
        let leading_space_len = (start as f64 * factor) as usize;
        print!("{: <1$}", "", leading_space_len - draw_len);
        draw_len = leading_space_len;

        // draw bar
        let bar_len = (duration as f64 * factor) as usize;
        print!("{:=<1$}", "", bar_len);
        draw_len += bar_len;

        total_cycles += duration;
    }

    // draw tailing space
    let tailing_space_len = BAR_LEN - draw_len + 1;
    print!("{: <1$}", "", tailing_space_len);

    println!(
        "{:6.2} ms",
        total_cycles as f64 * 1_000.0 / cycles_per_sec as f64
    );

    for id in ids {
        if let Some(children) = children_map.get(&id) {
            for child in children {
                draw_rec(
                    *child,
                    factor,
                    cycles_per_sec,
                    &following,
                    &children_map,
                    &spans_map,
                );
            }
        }
    }
}
