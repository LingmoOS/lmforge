use tracing_subscriber::Layer;

pub trait TelemetryLayer: Sized {
    fn with_console(self) -> ConsoleWithSelf<Self>
    where
        Self: Layer<tracing_subscriber::Registry> + Send + Sync + 'static,
    {
        ConsoleWithSelf { inner: self }
    }
}

impl<T> TelemetryLayer for T where T: Layer<tracing_subscriber::Registry> {}

pub struct ConsoleWithSelf<T> {
    inner: T,
}

impl<T> ConsoleWithSelf<T>
where
    T: Layer<tracing_subscriber::Registry> + Send + Sync + 'static,
{
    pub fn with_file_logging(
        self,
        build_id: &str,
        log_dir: &std::path::Path,
    ) -> anyhow::Result<impl Layer<tracing_subscriber::Registry> + Send + Sync + 'static> {
        use crate::telemetry::console::ConsoleLayer;
        use crate::telemetry::file_logger::FileLogLayer;

        let console_layer = ConsoleLayer::new();
        let file_layer = FileLogLayer::new(build_id, log_dir)?;

        Ok(self.inner.and_then(console_layer).and_then(file_layer))
    }
}
