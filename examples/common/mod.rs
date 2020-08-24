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
    trace_details: &minitrace::TraceResult,
) -> (
    Rc<RefCell<LeadingNode>>,
    u64, /* min begin */
    u64, /* max end*/
) {
    let mut spans = trace_details.spans.clone();
    spans.sort_by(|a, b| a.begin_cycles.cmp(&b.begin_cycles));
    let mut id_to_node: HashMap<u64, Node> = HashMap::new();

    let mut root = None;
    let mut min_begin = 0;
    let mut max_end = 0;

    let mut idle = vec![];
    for span in spans {
        let mut process = idle.clone();
        process.push(span);
        idle.clear();

        let end = span.begin_cycles + span.elapsed_cycles;
        if end > max_end {
            max_end = end;
        }

        for span in process {
            match span.state {
                minitrace::State::Root => {
                    min_begin = span.begin_cycles;
                    let leading_node = Rc::new(RefCell::new(LeadingNode { children: vec![] }));
                    id_to_node.insert(span.id, Node::LeadingNode(leading_node.clone()));
                    root = Some(leading_node);
                }
                minitrace::State::Local => match id_to_node.get(&span.related_id) {
                    Some(Node::NormalNode(parent)) => {
                        let normal_node = Rc::new(RefCell::new(NormalNode {
                            span,
                            normal_children: vec![],
                            leading_children: vec![],
                        }));
                        parent
                            .borrow_mut()
                            .normal_children
                            .push(normal_node.clone());
                        id_to_node.insert(span.id, Node::NormalNode(normal_node));
                    }
                    Some(_) => unreachable!(),
                    None => idle.push(span),
                },
                minitrace::State::Spawning => match id_to_node.get(&span.related_id) {
                    Some(Node::NormalNode(parent)) => {
                        let leading_node = Rc::new(RefCell::new(LeadingNode { children: vec![] }));
                        parent
                            .borrow_mut()
                            .leading_children
                            .push(leading_node.clone());
                        id_to_node.insert(span.id, Node::LeadingNode(leading_node));
                    }
                    Some(_) => unreachable!(),
                    None => idle.push(span),
                },
                minitrace::State::Scheduling => match id_to_node.get(&span.related_id) {
                    Some(Node::LeadingNode(prev)) => {
                        let prev = prev.clone();
                        id_to_node.insert(span.id, Node::LeadingNode(prev));
                    }
                    Some(_) => unreachable!(),
                    None => idle.push(span),
                },
                minitrace::State::Settle => match id_to_node.get(&span.related_id) {
                    Some(Node::LeadingNode(prev)) => {
                        let normal_node = Rc::new(RefCell::new(NormalNode {
                            span,
                            normal_children: vec![],
                            leading_children: vec![],
                        }));
                        prev.borrow_mut().children.push(normal_node.clone());
                        id_to_node.insert(span.id, Node::NormalNode(normal_node));
                    }
                    Some(_) => unreachable!(),
                    None => idle.push(span),
                },
            }
        }
    }
    assert!(idle.is_empty());
    (root.expect("root span isn't existing"), min_begin, max_end)
}

pub fn draw_stdout(trace_details: minitrace::TraceResult) {
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
