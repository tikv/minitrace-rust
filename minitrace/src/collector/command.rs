// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use crate::collector::{CollectArgs, SpanRecord, SpanSet};
use crate::util::CollectToken;

pub enum CollectCommand {
    StartCollect(StartCollect),
    DropCollect(DropCollect),
    CommitCollect(CommitCollect),
    SubmitSpans(SubmitSpans),
}

pub struct StartCollect {
    pub collect_id: u32,
    pub collect_args: CollectArgs,
}

pub struct DropCollect {
    pub collect_id: u32,
}

pub struct CommitCollect {
    pub collect_id: u32,
    pub tx: futures::channel::oneshot::Sender<Vec<SpanRecord>>,
}

pub struct SubmitSpans {
    pub spans: SpanSet,
    pub collect_token: CollectToken,
}
