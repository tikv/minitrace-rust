const BAR_LEN: usize = 70;

pub fn draw_stdout(spans: Vec<crate::Span>) {
    let mut children = std::collections::HashMap::new();
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

        if let Some(parent) = span.parent_id {
            children
                .entry(parent)
                .or_insert_with(|| vec![])
                .push(span.id);
        } else {
            root = Some(span.id);
        }
    }

    let root = root.expect("can not find root");
    let pivot = spans_map.get(&root).unwrap().0;
    let factor = BAR_LEN as f64 / max_end as f64;

    draw_rec(root, pivot, factor, &children, &spans_map);
}

fn draw_rec(
    cur_id: u32,
    pivot: u32,
    factor: f64,
    children_map: &std::collections::HashMap<u32, Vec<u32>>,
    spans_map: &std::collections::HashMap<u32, (u32, u32)>,
) {
    let (start, duration) = *spans_map.get(&cur_id).expect("can not get span");

    // draw leading space
    let leading_space_len = ((start - pivot) as f64 * factor) as usize;
    print!("{: <1$}", "", leading_space_len);

    // draw bar
    let bar_len = (duration as f64 * factor) as usize;
    print!("{:=<1$}", "", bar_len);

    // draw tailing space
    let tailing_space_len = BAR_LEN - bar_len - leading_space_len + 1;
    print!("{: <1$}", "", tailing_space_len);

    println!("{:2} ms", duration);

    if let Some(children) = children_map.get(&cur_id) {
        for child in children {
            draw_rec(*child, pivot, factor, &children_map, &spans_map);
        }
    }
}
