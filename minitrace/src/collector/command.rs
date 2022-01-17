// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::{CollectArgs, SpanRecord, SpanSet};
use crate::util::ParentSpans;

#[derive(Debug)]
pub enum CollectCommand {
    StartCollect(StartCollect),
    DropCollect(DropCollect),
    CommitCollect(CommitCollect),
    SubmitSpans(SubmitSpans),
}

#[derive(Debug)]
pub struct StartCollect {
    pub collect_id: u32,
    pub collect_args: CollectArgs,
}

#[derive(Debug)]
pub struct DropCollect {
    pub collect_id: u32,
}

#[derive(Debug)]
pub struct CommitCollect {
    pub collect_id: u32,
    pub tx: futures::channel::oneshot::Sender<Vec<SpanRecord>>,
}

#[derive(Debug)]
pub struct SubmitSpans {
    pub spans: SpanSet,
    pub parents: ParentSpans,
}
