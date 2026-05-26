use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::io::Write;
use chrono::Utc;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub stage: Option<String>,
    pub message: String,
    pub build_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

pub struct FileLogLayer {
    build_id: String,
    log_dir: PathBuf,
    main_log: Mutex<std::fs::File>,
    jsonl_log: Mutex<std::fs::File>,
    stage_logs: Mutex<std::collections::HashMap<String, std::fs::File>>,
}

impl FileLogLayer {
    pub fn new(build_id: &str, log_dir: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(log_dir)?;
        
        let stages_dir = log_dir.join("stages");
        std::fs::create_dir_all(&stages_dir)?;

        let main_log_path = log_dir.join("build.log");
        let jsonl_log_path = log_dir.join("build.jsonl");

        let main_log = std::fs::File::create(&main_log_path)?;
        let jsonl_log = std::fs::File::create(&jsonl_log_path)?;

        Ok(FileLogLayer {
            build_id: build_id.to_string(),
            log_dir: log_dir.to_path_buf(),
            main_log: Mutex::new(main_log),
            jsonl_log: Mutex::new(jsonl_log),
            stage_logs: Mutex::new(std::collections::HashMap::new()),
        })
    }

    fn get_or_create_stage_log(&self, stage: &str) -> anyhow::Result<std::fs::File> {
        let mut logs = self.stage_logs.lock().unwrap();
        
        if let Some(file) = logs.get(stage) {
            return Ok(file.try_clone()?);
        }

        let stage_log_path = self.log_dir
            .join("stages")
            .join(format!("{}.log", stage));
        
        let file = std::fs::File::create(&stage_log_path)?;
        logs.insert(stage.to_string(), file.try_clone()?);
        
        Ok(file)
    }
}

impl<S> tracing_subscriber::Layer<S> for FileLogLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let timestamp = Utc::now().to_rfc3339();
        let level = format!("{:?}", metadata.level());
        let target = metadata.target().to_string();
        
        let mut visitor = LogVisitor::new();
        event.record(&mut visitor);

        let message = visitor.message.unwrap_or_else(|| event.metadata().target().to_string());
        let stage = extract_stage_from_target(&target);

        let entry = LogEntry {
            timestamp: timestamp.clone(),
            level: level.clone(),
            stage: stage.clone(),
            message: message.clone(),
            build_id: self.build_id.clone(),
            target: Some(target),
            module: metadata.module_path().map(|m| m.to_string()),
            file: metadata.file().map(|f| f.to_string()),
            line: metadata.line(),
        };

        let text_line = format!(
            "{} {} {}: {}\n",
            timestamp,
            level.to_uppercase(),
            stage.as_deref().unwrap_or("system"),
            message
        );

        if let Ok(mut main_log) = self.main_log.lock() {
            let _ = main_log.write_all(text_line.as_bytes());
            let _ = main_log.flush();
        }

        if let Ok(json_str) = serde_json::to_string(&entry) {
            let jsonl_line = format!("{}\n", json_str);
            
            if let Ok(mut jsonl_log) = self.jsonl_log.lock() {
                let _ = jsonl_log.write_all(jsonl_line.as_bytes());
                let _ = jsonl_log.flush();
            }
        }

        if let Some(stage_name) = &stage {
            if let Ok(mut stage_log) = self.get_or_create_stage_log(stage_name) {
                let _ = stage_log.write_all(text_line.as_bytes());
                let _ = stage_log.flush();
            }
        }
    }
}

struct LogVisitor {
    message: Option<String>,
}

impl LogVisitor {
    fn new() -> Self {
        LogVisitor { message: None }
    }
}

impl tracing::field::Visit for LogVisitor {
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

fn extract_stage_from_target(target: &str) -> Option<String> {
    if target.contains("workspace") || target.contains("bootstrap") {
        Some("workspace".to_string())
    } else if target.contains("packages") || target.contains("runtime") || target.contains("chroot") {
        Some("packages".to_string())
    } else if target.contains("overlay") {
        Some("overlay".to_string())
    } else if target.contains("image") || target.contains("binary") {
        Some("image".to_string())
    } else if target.contains("release") || target.contains("metadata") || target.contains("finalize") {
        Some("release".to_string())
    } else {
        None
    }
}
