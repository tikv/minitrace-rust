const BAR_LEN: usize = 70;

pub fn draw_stdout(spans: Vec<minitrace::SpanSet>) {
    let mut children = std::collections::HashMap::new();
    let mut following = std::collections::HashMap::new();
    let mut follower_to_header = std::collections::HashMap::new();
    let mut spans_map = std::collections::HashMap::new();

    let mut root = None;
    let mut root_cycles = None;
    let mut max_end = 0;

    let spans = spans
        .into_iter()
        .map(|s| s.spans.into_iter())
        .flatten()
        .collect::<Vec<_>>();

    for span in spans {
        let start = span.begin_cycles;
        let end = span.end_cycles;

        if end > max_end {
            max_end = end;
        }

        assert_eq!(
            spans_map.insert(span.id, (start, end - start)),
            None,
            "duplicated id {:#?}",
            span.id
        );

        follower_to_header.insert(span.id, span.id);

        match span.link {
            minitrace::Link::Root => {
                root = Some(span.id);
                root_cycles = Some(span.begin_cycles);
            }
            minitrace::Link::Parent { id } => {
                children.entry(id).or_insert_with(Vec::new).push(span.id);
            }
            minitrace::Link::Continue { id } => {
                let header = follower_to_header[&id];
                follower_to_header.insert(span.id, header);

                following
                    .entry(header)
                    .or_insert_with(Vec::new)
                    .push(span.id);
            }
        }
    }

    let root = root.expect("can not find root");
    let root_cycles = root_cycles.unwrap();
    for (_, (start, _)) in spans_map.iter_mut() {
        *start -= root_cycles;
    }
    max_end -= root_cycles;

    if max_end == 0 {
        println!("Insufficient precision: total cost time < 1 ms");
        return;
    }

    let factor = BAR_LEN as f64 / max_end as f64;

    draw_rec(root, factor, &following, &children, &spans_map);
}

fn draw_rec(
    cur_id: u64,
    factor: f64,
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
        total_cycles as f64 * 1_000.0 / minitrace::cycles_per_sec() as f64
    );

    for id in ids {
        if let Some(children) = children_map.get(&id) {
            for child in children {
                draw_rec(*child, factor, &following, &children_map, &spans_map);
            }
        }
    }
}
