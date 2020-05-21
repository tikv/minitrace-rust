use crate::future::Instrument;

#[derive(Copy, Clone)]
enum CollectorType {
    Bounded,
    Unbounded,
}

// An auxiliary function for checking relations of spans.
// Note that the tags of each spans cannot be the same.
//
// Return: [(tag, Option<parent_tag>)], sorted by tag
fn rebuild_relation_by_tag(spans: Vec<crate::Span>) -> Vec<(u32, Option<u32>)> {
    let infos: std::collections::HashMap<u32, (Option<u32>, u32)> = spans
        .into_iter()
        .map(|s| {
            (
                s.id,
                (
                    match s.link {
                        crate::Link::Root { .. } => None,
                        crate::Link::Parent { id } => Some(id),
                        #[cfg(feature = "fine-async")]
                        crate::Link::Continue { .. } => unreachable!(),
                    },
                    s.tag,
                ),
            )
        })
        .collect();

    let mut res = Vec::with_capacity(infos.len());

    for (_id, (parent_id, tag)) in infos.iter() {
        if let Some(p) = parent_id {
            res.push((*tag, Some(infos[&p].1)));
        } else {
            res.push((*tag, None));
        }
    }

    res.sort();
    res
}

fn root(tag: u32, collector_type: CollectorType) -> (crate::SpanGuard, crate::CollectorRx) {
    let (tx, rx) = match collector_type {
        CollectorType::Bounded => crate::Collector::bounded(1024),
        CollectorType::Unbounded => crate::Collector::unbounded(),
    };
    let root = crate::new_span_root(tx, tag);
    (root, rx)
}

fn sync_spanned(tag: u32) {
    let span = crate::new_span(tag);
    let _g = span.enter();
}

#[test]
fn span_basic() {
    for clt_type in &[CollectorType::Bounded, CollectorType::Unbounded] {
        let (root, mut rx) = root(0, *clt_type);
        {
            let root = root;
            let _g = root.enter();

            sync_spanned(1);
        }

        let spans = rx.collect().unwrap();
        let spans = rebuild_relation_by_tag(spans);

        assert_eq!(spans.len(), 2);
        assert_eq!(&spans, &[(0, None), (1, Some(0))]);
    }
}

#[test]
fn span_async_basic() {
    for clt_type in &[CollectorType::Bounded, CollectorType::Unbounded] {
        let (root, mut rx) = root(0, *clt_type);
        let wg = crossbeam::sync::WaitGroup::new();

        {
            let root = root;
            let _g = root.enter();

            async fn dummy() {};

            for i in 1..=10 {
                let dummy = dummy().in_current_span(i as u32);
                let wg = wg.clone();

                std::thread::spawn(move || {
                    futures_03::executor::block_on(dummy);
                    drop(wg);
                });
            }
        }

        wg.wait();
        let spans = rx.collect().unwrap();
        let spans = rebuild_relation_by_tag(spans);

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
}

#[test]
fn span_wide_function() {
    for clt_type in &[CollectorType::Bounded, CollectorType::Unbounded] {
        let (root, mut rx) = root(0, *clt_type);
        {
            let root = root;
            let _g = root.enter();

            for i in 1..=10 {
                sync_spanned(i);
            }
        }

        let spans = rx.collect().unwrap();
        let spans = rebuild_relation_by_tag(spans);

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
}

#[test]
fn span_deep_function() {
    fn sync_spanned_rec_tag_step_to_1(step: u32) {
        let span = crate::new_span(step);
        let _g = span.enter();

        if step > 1 {
            sync_spanned_rec_tag_step_to_1(step - 1);
        }
    }

    for clt_type in &[CollectorType::Bounded, CollectorType::Unbounded] {
        let (root, mut rx) = root(0, *clt_type);
        {
            let root = root;
            let _g = root.enter();

            sync_spanned_rec_tag_step_to_1(10);
        }

        let spans = rx.collect().unwrap();
        let spans = rebuild_relation_by_tag(spans);

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
}

#[test]
fn test_bounded() {
    let (tx, mut rx) = crate::Collector::bounded(2);

    {
        let s = crate::new_span_root(tx, 0u32);
        let _g = s.enter();

        {
            // new span successfully
            let s = crate::new_span(1u32);
            let _g = s.enter();
        }

        {
            // collector is full, failed
            let s = crate::new_span(2u32);
            let _g = s.enter();
        }

        {
            // collector is full, failed
            let s = crate::new_span(3u32);
            let _g = s.enter();
        }

        {
            // collector is full, failed
            let s = crate::new_span(4u32);
            let _g = s.enter();
        }
    }

    let spans = rx.collect().unwrap();
    let spans = rebuild_relation_by_tag(spans);

    assert_eq!(spans.len(), 2);
    assert_eq!(&spans, &[(0, None), (1, Some(0))]);
}

#[test]
fn test_out_of_order_drop_enter() {
    let (tx, _rx) = crate::Collector::bounded(512);
    {
        let s = crate::new_span_root(tx, 0u32);
        let _g = s.enter();

        let s1 = crate::new_span(1u32);
        let s2 = crate::new_span(2u32);
        let s3 = crate::new_span(3u32);
        let s4 = crate::new_span(4u32);
        let e1 = s1.enter();
        let e2 = s2.enter();
        let e3 = s3.enter();
        let e4 = s4.enter();

        drop(e3);
        drop(e1);
        drop(e2);
        drop(e4);

        let mut spans = vec![];
        let mut enters = vec![];
        for i in 0..100 {
            spans.push(crate::new_span(i as u32));
        }
        for span in spans.iter() {
            enters.push(span.enter());
        }
    }
}
