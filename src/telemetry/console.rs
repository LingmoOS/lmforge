use tracing_subscriber::fmt::{MakeWriter, format::FmtSpan};
use std::io;
use std::sync::Mutex;
use console::Term;

const STAGE_WIDTH: usize = 10;

pub struct ConsoleLayer {
    writer: Mutex<Term>,
}

impl ConsoleLayer {
    pub fn new() -> Self {
        ConsoleLayer {
            writer: Mutex::new(Term::stdout()),
        }
    }
}

impl Default for ConsoleLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> tracing_subscriber::Layer<S> for ConsoleLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = ConsoleVisitor::new();
        event.record(&mut visitor);

        let metadata = event.metadata();
        let level = *metadata.level();
        let target = metadata.target();
        
        let stage = extract_stage(target);
        let message = visitor.message.unwrap_or_default();

        if let Ok(mut writer) = self.writer.lock() {
            let output = format_console_line(stage.as_deref(), level, &message);
            
            if level == tracing::Level::ERROR {
                let _ = writeln!(writer, "\x1b[31m{}\x1b[0m", output);
            } else if level == tracing::Level::WARN {
                let _ = writeln!(writer, "\x1b[33m{}\x1b[0m", output);
            } else {
                let _ = writeln!(writer, "{}", output);
            }
        }
    }
}

struct ConsoleVisitor {
    message: Option<String>,
}

impl ConsoleVisitor {
    fn new() -> Self {
        ConsoleVisitor { message: None }
    }
}

impl tracing::field::Visit for ConsoleVisitor {
    fn record_debug(&mut self, field: &tracing::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{:?}", value));
        }
    }

    fn record_str(&mut self, field: &tracing::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }
}

fn extract_stage(target: &str) -> Option<String> {
    if target.contains("workspace") {
        Some("workspace".to_string())
    } else if target.contains("packages") || target.contains("runtime") {
        Some("packages ".to_string())
    } else if target.contains("overlay") {
        Some("overlay  ".to_string())
    } else if target.contains("image") {
        Some("image    ".to_string())
    } else if target.contains("release") || target.contains("metadata") {
        Some("release  ".to_string())
    } else if target.contains("bootstrap") {
        Some("workspace".to_string())
    } else {
        None
    }
}

fn format_console_line(stage: Option<&str>, level: tracing::Level, message: &str) -> String {
    match stage {
        Some(s) => {
            match level {
                tracing::Level::ERROR => {
                    format!("[{}] ERROR: {}", s, message)
                }
                tracing::Level::WARN => {
                    format!("[{}] WARN: {}", s, message)
                }
                tracing::Level::INFO => {
                    format!("[{}] {}", s, message)
                }
                _ => {
                    format!("[\x1b[90m{}\x1b[0m] \x1b[90m{}\x1b[0m", s, message)
                }
            }
        }
        None => {
            message.to_string()
        }
    }
}
