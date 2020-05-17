const BAR_LEN: usize = 70;

pub fn draw_stdout(spans: Vec<crate::Span>) {
    let mut children = std::collections::HashMap::new();
    #[allow(unused_mut)]
    let mut following = std::collections::HashMap::new();
    let mut follower_to_header = std::collections::HashMap::new();
    let mut spans_map = std::collections::HashMap::new();

    let mut root = None;
    let mut max_end = 0;
    for span in spans {
        let start = span.elapsed_start;
        let end = span.elapsed_end;

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
            crate::Link::Root => root = Some(span.id),
            crate::Link::Parent { id } => {
                children.entry(id).or_insert_with(Vec::new).push(span.id);
            }
            #[cfg(feature = "fine-async")]
            crate::Link::Continue { id } => {
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

    if max_end == 0 {
        println!("Insufficient precision: total cost time < 1 ms");
    }

    let factor = BAR_LEN as f64 / max_end as f64;

    draw_rec(root, factor, &following, &children, &spans_map);
}

fn draw_rec(
    cur_id: u32,
    factor: f64,
    following: &std::collections::HashMap<u32, Vec<u32>>,
    children_map: &std::collections::HashMap<u32, Vec<u32>>,
    spans_map: &std::collections::HashMap<u32, (u32, u32)>,
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
    let mut total_cost = 0u32;

    for (start, duration) in span {
        // draw leading space
        let leading_space_len = (start as f64 * factor) as usize;
        print!("{: <1$}", "", leading_space_len - draw_len);
        draw_len = leading_space_len;

        // draw bar
        let bar_len = (duration as f64 * factor) as usize;
        print!("{:=<1$}", "", bar_len);
        draw_len += bar_len;

        total_cost += duration;
    }

    // draw tailing space
    let tailing_space_len = BAR_LEN - draw_len + 1;
    print!("{: <1$}", "", tailing_space_len);

    println!("{:2} ms", total_cost);

    for id in ids {
        if let Some(children) = children_map.get(&id) {
            for child in children {
                draw_rec(*child, factor, &following, &children_map, &spans_map);
            }
        }
    }
}
