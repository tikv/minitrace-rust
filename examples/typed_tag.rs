// Enum type user cases
#[derive(Debug)]
enum Job {
    JobXParent,
    JobXChildA,
    JobXChildB,

    JobYParent,
    JobYChildA,
    JobYChildB,
}

trait ToJob {
    fn u32_job(tag: u32) -> Job;
    fn to_u32(self) -> u32;
}

fn new_span_root<T: ToJob>(tx: minitrace::CollectorTx, tag: T) -> minitrace::SpanGuard {
    minitrace::new_span_root(tx, T::to_u32(tag))
}

fn new_span<T: ToJob>(tag: T) -> minitrace::SpanGuard {
    minitrace::new_span(T::to_u32(tag))
}

fn collect<T: ToJob>(rx: minitrace::CollectorRx) -> Vec<Job> {
    rx.collect()
        .into_iter()
        .map(|span| T::u32_job(span.tag))
        .collect()
}

mod jobx {
    #[repr(u32)]
    enum JobX {
        Parent,
        ChildA,
        ChildB,
    }

    impl super::ToJob for JobX {
        fn u32_job(tag: u32) -> super::Job {
            match tag {
                tag if tag == JobX::Parent as u32 => super::Job::JobXParent,
                tag if tag == JobX::ChildA as u32 => super::Job::JobXChildA,
                tag if tag == JobX::ChildB as u32 => super::Job::JobXChildB,
                _ => panic!("unknown number"),
            }
        }

        fn to_u32(self) -> u32 {
            self as u32
        }
    }

    pub(crate) fn jobx() -> Vec<super::Job> {
        let (tx, rx) = minitrace::Collector::new_default();
        let span = super::new_span_root(tx, JobX::Parent);
        let _g = span.enter();

        child(JobX::ChildA);
        child(JobX::ChildB);

        drop(_g);
        drop(span);

        super::collect::<JobX>(rx)
    }

    fn child(tag: JobX) {
        let span = super::new_span(tag);
        let _g = span.enter();
    }
}

mod joby {
    #[repr(u32)]
    enum JobY {
        Parent,
        ChildA,
        ChildB,
    }

    impl super::ToJob for JobY {
        fn u32_job(tag: u32) -> super::Job {
            match tag {
                tag if tag == JobY::Parent as u32 => super::Job::JobYParent,
                tag if tag == JobY::ChildA as u32 => super::Job::JobYChildA,
                tag if tag == JobY::ChildB as u32 => super::Job::JobYChildB,
                _ => panic!("unknown number"),
            }
        }

        fn to_u32(self) -> u32 {
            self as u32
        }
    }

    pub(crate) fn joby() -> Vec<super::Job> {
        let (tx, rx) = minitrace::Collector::new_default();
        let span = super::new_span_root(tx, JobY::Parent);
        let _g = span.enter();

        child(JobY::ChildA);
        child(JobY::ChildB);

        drop(_g);
        drop(span);

        super::collect::<JobY>(rx)
    }

    fn child(tag: JobY) {
        let span = super::new_span(tag);
        let _g = span.enter();
    }
}

fn main() {
    let _ = jobx::jobx();
    let _ = joby::joby();
}
