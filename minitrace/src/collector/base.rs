pub trait Collector: Send {
    fn start(&self);
    fn handle_commands(&mut self, flush: bool);
}
