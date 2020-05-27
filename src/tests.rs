use crate::prelude::*;

// An auxiliary function for checking relations of spans.
// Note that the events of each spans cannot be the same.
//
// Return: [(event, Option<parent_event>)], sorted by event
fn rebuild_relation_by_event(spans: Vec<crate::SpanSet>) -> Vec<(u32, Option<u32>)> {
    let spans = spans
        .into_iter()
        .map(|s| s.spans.into_iter())
        .flatten()
        .collect::<Vec<_>>();
    let infos: std::collections::HashMap<u64, (Option<u64>, u32)> = spans
        .into_iter()
        .map(|s| {
            (
                s.id,
                (
                    match s.link {
                        crate::Link::Root => None,
                        crate::Link::Parent { id } => Some(id),
                        crate::Link::Continue { .. } => unreachable!(),
                    },
                    s.event,
                ),
            )
        })
        .collect();

    let mut res = Vec::with_capacity(infos.len());

    for (_id, (parent_id, event)) in infos.iter() {
        if let Some(p) = parent_id {
            res.push((*event, Some(infos[&p].1)));
        } else {
            res.push((*event, None));
        }
    }

    res.sort();
    res
}

fn sync_spanned(event: u32) {
    let _guard = crate::new_span(event);
}

#[test]
fn span_basic() {
    let (root, collector) = crate::trace_enable(0);
    {
        let _guard = root;
        sync_spanned(1);
    }

    let spans = collector.collect();
    let spans = rebuild_relation_by_event(spans);

    assert_eq!(spans.len(), 2);
    assert_eq!(&spans, &[(0, None), (1, Some(0))]);
}

#[test]
fn span_async_basic() {
    let (root, collector) = crate::trace_enable(0);

    let wg = crossbeam::sync::WaitGroup::new();

    {
        let _guard = root;

        async fn dummy() {};

        for i in 1..=5 {
            let dummy = dummy().trace_task(i);
            let wg = wg.clone();

            std::thread::spawn(move || {
                futures_03::executor::block_on(dummy);
                drop(wg);
            });
        }

        for i in 6..=10 {
            let handle = crate::trace_crossthread(i);
            let wg = wg.clone();

            std::thread::spawn(move || {
                let mut handle = handle;
                let guard = handle.trace_enable();
                drop(guard);
                drop(wg);
            });
        }
    }

    wg.wait();
    let spans = collector.collect();
    let spans = rebuild_relation_by_event(spans);

    assert_eq!(spans.len(), 11);
    assert_eq!(
        &spans,
        &[
            (0, None),
            (1, Some(0)),
            (2, Some(0)),
            (3, Some(0)),
            (4, Some(0)),
            (5, Some(0)),
            (6, Some(0)),
            (7, Some(0)),
            (8, Some(0)),
            (9, Some(0)),
            (10, Some(0))
        ]
    );
}

#[test]
fn span_wide_function() {
    let (root, collector) = crate::trace_enable(0);

    {
        let _guard = root;
        for i in 1..=10 {
            sync_spanned(i);
        }
    }

    let spans = collector.collect();
    let spans = rebuild_relation_by_event(spans);

    assert_eq!(spans.len(), 11);
    assert_eq!(
        &spans,
        &[
            (0, None),
            (1, Some(0)),
            (2, Some(0)),
            (3, Some(0)),
            (4, Some(0)),
            (5, Some(0)),
            (6, Some(0)),
            (7, Some(0)),
            (8, Some(0)),
            (9, Some(0)),
            (10, Some(0))
        ]
    );
}

#[test]
fn span_deep_function() {
    fn sync_spanned_rec_event_step_to_1(step: u32) {
        let _guard = crate::new_span(step);

        if step > 1 {
            sync_spanned_rec_event_step_to_1(step - 1);
        }
    }

    let (root, collector) = crate::trace_enable(0);

    {
        let _guard = root;
        sync_spanned_rec_event_step_to_1(10);
    }

    let spans = collector.collect();
    let spans = rebuild_relation_by_event(spans);

    assert_eq!(spans.len(), 11);
    assert_eq!(
        &spans,
        &[
            (0, None),
            (1, Some(2)),
            (2, Some(3)),
            (3, Some(4)),
            (4, Some(5)),
            (5, Some(6)),
            (6, Some(7)),
            (7, Some(8)),
            (8, Some(9)),
            (9, Some(10)),
            (10, Some(0))
        ]
    );
}
