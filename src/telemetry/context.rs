use tracing::{info_span, Span};

pub struct BuildContextExt;

impl BuildContextExt {
    pub fn stage_span(stage: &str) -> Span {
        info_span!("stage", stage)
    }

    pub fn process_span(command: &str) -> Span {
        info_span!("process", command)
    }

    pub fn mount_span(operation: &str, target: &str) -> Span {
        info_span!("mount", operation, target)
    }
}

#[macro_export]
macro_rules! stage_info {
    ($stage:expr, $($arg:tt)+) => {
        tracing::info!(
            stage = $stage,
            $($arg)+
        )
    };
}

#[macro_export]
macro_rules! stage_warn {
    ($stage:expr, $($arg:tt)+) => {
        tracing::warn!(
            stage = $stage,
            $($arg)+
        )
    };
}

#[macro_export]
macro_rules! stage_error {
    ($stage:expr, $($arg:tt)+) => {
        tracing::error!(
            stage = $stage,
            $($arg)+
        )
    };
}

#[macro_export]
macro_rules! runtime_info {
    ($($arg:tt)+) => {
        tracing::info!(
            target: "lmforge_runtime",
            $($arg)+
        )
    };
}

#[macro_export]
macro_rules! runtime_debug {
    ($($arg:tt)+) => {
        tracing::debug!(
            target: "lmforge_runtime",
            $($arg)+
        )
    };
}
