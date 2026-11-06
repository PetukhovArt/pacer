pub trait Sink<T>: Send + Sync + 'static {
    fn send(&self, event: T) -> bool;
}

pub type BoxSink<T> = Box<dyn Sink<T>>;
