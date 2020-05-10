#[derive(Clone, Copy, Debug)]
pub enum CollectorType {
    Void,
    Channel,
}

#[derive(Clone, Debug)]
pub enum CollectorTx {
    Void,
    Channel(crossbeam::channel::Sender<crate::Span>),
}

impl CollectorTx {
    #[inline]
    pub fn push(&self, span: crate::Span) {
        match self {
            CollectorTx::Void => (),
            CollectorTx::Channel(c) => {
                let _ = c.try_send(span);
            }
        }
    }
}

pub enum CollectorRx {
    Void,
    Channel(crossbeam::channel::Receiver<crate::Span>),
}

impl CollectorRx {
    #[inline]
    pub fn collect(self) -> Vec<crate::Span> {
        match self {
            CollectorRx::Void => vec![],
            CollectorRx::Channel(c) => c.iter().collect(),
        }
    }

    #[inline]
    pub fn try_collect(&self) -> Vec<crate::Span> {
        match self {
            CollectorRx::Void => vec![],
            CollectorRx::Channel(c) => c.try_iter().collect(),
        }
    }
}

pub struct Collector;

impl Collector {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(tp: CollectorType) -> (CollectorTx, CollectorRx) {
        match tp {
            CollectorType::Void => (CollectorTx::Void, CollectorRx::Void),
            CollectorType::Channel => {
                let (tx, rx) = crossbeam::channel::unbounded();
                (CollectorTx::Channel(tx), CollectorRx::Channel(rx))
            }
        }
    }

    #[inline]
    pub fn new_default() -> (CollectorTx, CollectorRx) {
        Self::new(crate::DEFAULT_COLLECTOR)
    }
}
