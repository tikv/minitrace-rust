// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

//! A module for relationship checking in test

use crate::collector::SpanSet;
use crate::local::span_id::SpanId;
use crate::util::{CollectToken, RawSpans};

use std::collections::HashMap;

#[derive(Debug, PartialOrd, PartialEq, Ord, Eq)]
pub struct Tree {
    event: &'static str,
    children: Vec<Tree>,
    properties: Vec<(&'static str, String)>,
}

pub fn t(event: &'static str, children: impl IntoIterator<Item = Tree>) -> Tree {
    Tree {
        event,
        children: children.into_iter().collect(),
        properties: vec![],
    }
}

pub fn tp(
    event: &'static str,
    children: impl IntoIterator<Item = Tree>,
    properties: impl IntoIterator<Item = (&'static str, String)>,
) -> Tree {
    Tree {
        event,
        children: children.into_iter().collect(),
        properties: properties.into_iter().collect(),
    }
}

impl Tree {
    pub fn sort(&mut self) {
        for child in &mut self.children {
            child.sort();
        }
        self.children.as_mut_slice().sort_unstable();
    }

    pub fn from_raw_spans(raw_spans: RawSpans) -> Vec<Tree> {
        let mut children = HashMap::new();

        let spans = raw_spans.into_inner().1;
        children.insert(SpanId::default(), ("", vec![], vec![]));
        for span in &spans {
            children.insert(span.id, (span.event, vec![], span.properties.clone()));
        }
        for span in &spans {
            children
                .get_mut(&span.parent_id)
                .as_mut()
                .unwrap()
                .1
                .push(span.id);
        }

        let mut t = Self::build_tree(SpanId::default(), &mut children);
        t.sort();
        t.children
    }

    /// Return a vector of collect id -> Tree
    pub fn from_span_sets(span_sets: &[(SpanSet, CollectToken)]) -> Vec<(u32, Tree)> {
        let mut collect = HashMap::<
            u32,
            HashMap<SpanId, (&'static str, Vec<SpanId>, Vec<(&'static str, String)>)>,
        >::new();
        for (span_set, token) in span_sets {
            for item in token.iter() {
                collect
                    .entry(item.collect_id)
                    .or_default()
                    .insert(SpanId::default(), ("", vec![], vec![]));
                match span_set {
                    SpanSet::Span(span) => {
                        collect
                            .entry(item.collect_id)
                            .or_default()
                            .insert(span.id, (span.event, vec![], span.properties.clone()));
                    }
                    SpanSet::LocalSpans(spans) => {
                        for span in spans.spans.iter() {
                            collect
                                .entry(item.collect_id)
                                .or_default()
                                .insert(span.id, (span.event, vec![], span.properties.clone()));
                        }
                    }
                    SpanSet::SharedLocalSpans(spans) => {
                        for span in spans.spans.iter() {
                            collect
                                .entry(item.collect_id)
                                .or_default()
                                .insert(span.id, (span.event, vec![], span.properties.clone()));
                        }
                    }
                }
            }
        }

        for (span_set, token) in span_sets {
            for item in token.iter() {
                match span_set {
                    SpanSet::Span(span) => {
                        let parent_id = if span.parent_id == SpanId::default() {
                            item.parent_id_of_roots
                        } else {
                            span.parent_id
                        };
                        collect
                            .get_mut(&item.collect_id)
                            .as_mut()
                            .unwrap()
                            .get_mut(&parent_id)
                            .as_mut()
                            .unwrap()
                            .1
                            .push(span.id);
                    }
                    SpanSet::LocalSpans(spans) => {
                        for span in spans.spans.iter() {
                            let parent_id = if span.parent_id == SpanId::default() {
                                item.parent_id_of_roots
                            } else {
                                span.parent_id
                            };
                            collect
                                .get_mut(&item.collect_id)
                                .as_mut()
                                .unwrap()
                                .get_mut(&parent_id)
                                .as_mut()
                                .unwrap()
                                .1
                                .push(span.id);
                        }
                    }
                    SpanSet::SharedLocalSpans(spans) => {
                        for span in spans.spans.iter() {
                            let parent_id = if span.parent_id == SpanId::default() {
                                item.parent_id_of_roots
                            } else {
                                span.parent_id
                            };
                            collect
                                .get_mut(&item.collect_id)
                                .as_mut()
                                .unwrap()
                                .get_mut(&parent_id)
                                .as_mut()
                                .unwrap()
                                .1
                                .push(span.id);
                        }
                    }
                }
            }
        }

        let mut res = collect
            .into_iter()
            .map(|(id, mut children)| {
                let mut tree = Self::build_tree(SpanId::default(), &mut children);
                tree.sort();
                assert_eq!(tree.children.len(), 1);
                (id, tree.children.pop().unwrap())
            })
            .collect::<Vec<(u32, Tree)>>();
        res.sort_unstable();
        res
    }

    #[allow(clippy::type_complexity)]
    fn build_tree(
        id: SpanId,
        raw: &mut HashMap<SpanId, (&'static str, Vec<SpanId>, Vec<(&'static str, String)>)>,
    ) -> Tree {
        let (event, children, properties) = raw.get(&id).cloned().unwrap();
        Tree {
            event,
            children: children
                .into_iter()
                .map(|id| Self::build_tree(id, raw))
                .collect(),
            properties,
        }
    }
}