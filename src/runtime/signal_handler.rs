use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::sync::Mutex;
use anyhow::Result;
use tracing::{info, warn, error};

use super::mount_manager::MountManager;
use crate::infra::cleanup::CleanupRecovery;

pub struct SignalHandler {
    interrupted: Arc<AtomicBool>,
    cleanup_callback: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl SignalHandler {
    pub fn new() -> Self {
        SignalHandler {
            interrupted: Arc::new(AtomicBool::new(false)),
            cleanup_callback: Mutex::new(None),
        }
    }

    pub fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }

    pub fn set_cleanup_callback<F>(&self, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let mut cb = self.cleanup_callback.lock().unwrap();
        *cb = Some(Arc::new(callback));
    }

    pub fn register_signals(&self) -> Result<()> {
        info!(target: "lmforge_signal", "registering signal handlers");

        let interrupted_clone = self.interrupted.clone();
        
        #[cfg(unix)]
        {
            use signal_hook::consts::signal;
            use signal_hook::iterator::Signals;

            let mut signals = Signals::new([
                signal::SIGINT,
                signal::SIGTERM,
                signal::SIGHUP,
            ])?;

            let callback_clone = self.cleanup_callback.lock().unwrap().clone();

            std::thread::spawn(move || {
                info!(target: "lmforge_signal", "signal handler thread started");

                for sig in signals.forever() {
                    match sig {
                        signal::SIGINT => {
                            warn!(
                                target: "lmforge_signal",
                                signal = "SIGINT",
                                "received SIGINT (Ctrl+C), initiating graceful shutdown"
                            );
                        }
                        signal::SIGTERM => {
                            warn!(
                                target: "lmforge_signal",
                                signal = "SIGTERM",
                                "received SIGTERM, initiating graceful shutdown"
                            );
                        }
                        signal::SIGHUP => {
                            warn!(
                                target: "lmforge_signal",
                                signal = "SIGHUP",
                                "received SIGHUP, initiating graceful shutdown"
                            );
                        }
                        _ => {
                            warn!(
                                target: "lmforge_signal",
                                signal = sig,
                                "received unknown signal"
                            );
                            continue;
                        }
                    }

                    interrupted_clone.store(true, Ordering::SeqCst);

                    if let Some(ref cb) = callback_clone {
                        info!(target: "lmforge_signal", "executing cleanup callback");
                        cb();
                    } else {
                        warn!(target: "lmforge_signal", "no cleanup callback registered");
                    }

                    break;
                }

                info!(target: "lmforge_signal", "signal handler thread exiting");
            });
        }

        #[cfg(not(unix))]
        {
            warn!(target: "lmforge_signal", "signal handling not supported on this platform");
        }

        info!(target: "lmforge_signal", "signal handlers registered successfully");
        Ok(())
    }

    pub fn check_interrupted(&self) -> Result<()> {
        if self.is_interrupted() {
            Err(anyhow::anyhow!("Build interrupted by signal"))
        } else {
            Ok(())
        }
    }
}

impl Default for SignalHandler {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BuildInterruptionGuard {
    signal_handler: SignalHandler,
    mount_manager: Option<Arc<MountManager>>,
    cleanup_recovery: Option<CleanupRecovery>,
}

impl BuildInterruptionGuard {
    pub fn new(_build_id: &str) -> Self {
        BuildInterruptionGuard {
            signal_handler: SignalHandler::new(),
            mount_manager: None,
            cleanup_recovery: None,
        }
    }

    pub fn with_mount_manager(mut self, manager: Arc<MountManager>) -> Self {
        self.mount_manager = Some(manager);
        self
    }

    pub fn with_cleanup_recovery(mut self, recovery: CleanupRecovery) -> Self {
        self.cleanup_recovery = Some(recovery);
        self
    }

    pub fn initialize(&mut self) -> Result<()> {
        info!(target: "lmforge_interruption", "initializing build interruption guard");

        let mount_manager_clone = self.mount_manager.clone();
        let cleanup_clone = self.cleanup_recovery.clone();

        self.signal_handler.set_cleanup_callback(move || {
            error!(target: "lmforge_interruption", "BUILD INTERRUPTED - executing emergency cleanup");

            if let Some(ref mm) = mount_manager_clone {
                warn!(target: "lmforge_interruption", "force unmounting all mounts");
                if let Err(e) = mm.force_cleanup_all() {
                    error!(target: "lmforge_interruption", error = %e, "failed to cleanup mounts");
                }
            }

            if let Some(ref cr) = cleanup_clone {
                warn!(target: "lmforge_interruption", "running full cleanup");
                if let Err(e) = cr.full_cleanup() {
                    error!(target: "lmforge_interruption", error = %e, "failed to run full cleanup");
                }
            }

            error!(target: "lmforge_interruption", "emergency cleanup completed, exiting");
            std::process::exit(130);
        });

        self.signal_handler.register_signals()?;

        info!(target: "lmforge_interruption", "build interruption guard initialized");
        Ok(())
    }

    pub fn check(&self) -> Result<()> {
        self.signal_handler.check_interrupted()
    }

    pub fn is_interrupted(&self) -> bool {
        self.signal_handler.is_interrupted()
    }

    pub fn signal_handler(&self) -> &SignalHandler {
        &self.signal_handler
    }
}
