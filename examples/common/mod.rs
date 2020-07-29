// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const BAR_LEN: usize = 70;

#[derive(Debug)]
struct LeadingNode {
    children: Vec<Rc<RefCell<NormalNode>>>,
}

#[derive(Debug)]
struct NormalNode {
    span: minitrace::Span,
    normal_children: Vec<Rc<RefCell<NormalNode>>>,
    leading_children: Vec<Rc<RefCell<LeadingNode>>>,
}

#[derive(Debug)]
enum Node {
    LeadingNode(Rc<RefCell<LeadingNode>>),
    NormalNode(Rc<RefCell<NormalNode>>),
}

fn build_tree(
    trace_details: &minitrace::TraceDetails,
) -> (
    Rc<RefCell<LeadingNode>>,
    u64, /* min begin */
    u64, /* max end*/
) {
    let mut span_sets = trace_details.span_sets.clone();
    span_sets.sort_by(|a, b| a.spans[0].begin_cycles.cmp(&b.spans[0].begin_cycles));
    let mut id_to_node: HashMap<u64, Node> = HashMap::new();

    let mut root = None;
    let mut min_begin = None;
    let mut max_end = 0;
    for span_set in span_sets {
        let leading_span = span_set.spans[0];
        let next_span = span_set.spans[1];
        assert_eq!(next_span.state, minitrace::State::Settle);

        let next_node = Rc::new(RefCell::new(NormalNode {
            span: next_span,
            normal_children: vec![],
            leading_children: vec![],
        }));
        id_to_node.insert(next_span.id, Node::NormalNode(next_node.clone()));
        let end = next_span.begin_cycles + next_span.elapsed_cycles;
        if end > max_end {
            max_end = end;
        }

        if leading_span.state == minitrace::State::Root {
            let leading_node = Rc::new(RefCell::new(LeadingNode {
                children: vec![next_node.clone()],
            }));
            min_begin = Some(leading_span.begin_cycles);
            root = Some(leading_node.clone());
            id_to_node.insert(leading_span.id, Node::LeadingNode(leading_node));
        } else {
            assert!(
                leading_span.state == minitrace::State::Spawning
                    || leading_span.state == minitrace::State::Scheduling
            );
            match id_to_node.get(&leading_span.related_id) {
                Some(Node::LeadingNode(prev)) => {
                    let node = {
                        let node_ref = &mut *prev.borrow_mut();
                        node_ref.children.push(next_node.clone());
                        prev.clone()
                    };
                    id_to_node.insert(leading_span.id, Node::LeadingNode(node));
                }
                Some(Node::NormalNode(normal_node)) => {
                    let leading_node = {
                        let leading_node = Rc::new(RefCell::new(LeadingNode {
                            children: vec![next_node.clone()],
                        }));
                        let node_ref = &mut *normal_node.borrow_mut();
                        node_ref.leading_children.push(leading_node.clone());
                        leading_node
                    };
                    id_to_node.insert(leading_span.id, Node::LeadingNode(leading_node));
                }
                None => unreachable!("{}", leading_span.related_id),
            }
        }

        for span in &span_set.spans[2..] {
            let node = Rc::new(RefCell::new(NormalNode {
                span: *span,
                normal_children: vec![],
                leading_children: vec![],
            }));
            {
                let end = span.begin_cycles + span.elapsed_cycles;
                if end > max_end {
                    max_end = end;
                }
                if let Node::NormalNode(normal_node) = &id_to_node[&span.related_id] {
                    let node_ref = &mut *normal_node.borrow_mut();
                    node_ref.normal_children.push(node.clone());
                } else {
                    panic!("related span of {} isn't existing", span.related_id);
                }
            }
            id_to_node.insert(span.id, Node::NormalNode(node));
        }
    }

    (
        root.expect("root span isn't existing"),
        min_begin.unwrap(),
        max_end,
    )
}

pub fn draw_stdout(trace_details: minitrace::TraceDetails) {
    let (tree, min_begin, max_end) = build_tree(&trace_details);
    let factor = BAR_LEN as f64 / (max_end - min_begin) as f64;
    draw_leading(factor, min_begin, trace_details.cycles_per_second, &tree);
}

fn draw_leading(factor: f64, anchor: u64, cycles_per_sec: u64, leading: &Rc<RefCell<LeadingNode>>) {
    let mut draw_len = 0usize;
    let mut total_cycles = 0u64;
    for normal_node in &leading.borrow().children {
        let node_ref = normal_node.borrow();
        let (start, elapsed) = (
            node_ref.span.begin_cycles - anchor,
            node_ref.span.elapsed_cycles,
        );
        // draw leading space
        let leading_space_len = (start as f64 * factor) as usize;
        let space_len = leading_space_len - draw_len;
        print!("{: <1$}", "", space_len);
        draw_len += space_len;

        // draw bar
        let bar_len = (elapsed as f64 * factor) as usize;
        print!("{:=<1$}", "", bar_len);
        draw_len += bar_len;

        total_cycles += elapsed;
    }
    // draw tailing space
    let tailing_space_len = BAR_LEN - draw_len + 1;
    print!("{: <1$}", "", tailing_space_len);

    // draw time
    println!(
        "{:6.2} ms",
        total_cycles as f64 * 1_000.0 / cycles_per_sec as f64
    );

    for normal_node in &leading.borrow().children {
        draw_normal(factor, anchor, cycles_per_sec, normal_node);
    }
}

fn draw_normal(
    factor: f64,
    anchor: u64,
    cycles_per_sec: u64,
    normal_node: &Rc<RefCell<NormalNode>>,
) {
    for normal_node in &normal_node.borrow().normal_children {
        let node_ref = normal_node.borrow();
        let (start, elapsed) = (
            node_ref.span.begin_cycles - anchor,
            node_ref.span.elapsed_cycles,
        );
        // draw leading space
        let leading_space_len = (start as f64 * factor) as usize;
        print!("{: <1$}", "", leading_space_len);

        // draw bar
        let bar_len = (elapsed as f64 * factor) as usize;
        print!("{:=<1$}", "", bar_len);

        // draw tailing space
        let tailing_space_len = BAR_LEN - bar_len - leading_space_len + 1;
        print!("{: <1$}", "", tailing_space_len);

        // draw time
        println!(
            "{:6.2} ms",
            elapsed as f64 * 1_000.0 / cycles_per_sec as f64
        );
        draw_normal(factor, anchor, cycles_per_sec, normal_node);
    }
    for leading_node in &normal_node.borrow().leading_children {
        draw_leading(factor, anchor, cycles_per_sec, leading_node);
    }
}
