//! Migration progress events and event handling

/// Events that can occur during migration
#[derive(Debug, Clone)]
pub enum MigrationEvent {
    Started,
    StepBegun { step: String },
    StepCompleted { step: String, duration_ms: u64 },
    BlobProcessed { cid: String, bytes: u64 },
    BlobFailed { cid: String, error: String },
    Warning { message: String },
    Error { message: String },
    Completed { success: bool },
}

/// Event handler for migration events
pub trait MigrationEventHandler {
    fn handle_event(&self, event: MigrationEvent);
}

/// Composite event handler that forwards events to multiple handlers
pub struct CompositeEventHandler {
    handlers: Vec<Box<dyn MigrationEventHandler>>,
}

impl Default for CompositeEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CompositeEventHandler {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    pub fn add_handler<H: MigrationEventHandler + 'static>(&mut self, handler: H) {
        self.handlers.push(Box::new(handler));
    }
}

impl MigrationEventHandler for CompositeEventHandler {
    fn handle_event(&self, event: MigrationEvent) {
        for handler in &self.handlers {
            handler.handle_event(event.clone());
        }
    }
}

/// Simple logging event handler
pub struct LoggingEventHandler;

impl MigrationEventHandler for LoggingEventHandler {
    fn handle_event(&self, event: MigrationEvent) {
        use crate::{console_debug, console_error, console_info, console_warn};

        match event {
            MigrationEvent::Started => {
                console_info!("[Event] 🚀 Migration started");
            }
            MigrationEvent::StepBegun { step } => {
                console_info!("{}", format!("[Event] 📋 Step begun: {}", step));
            }
            MigrationEvent::StepCompleted { step, duration_ms } => {
                console_info!(
                    "{}",
                    format!("[Event] ✅ Step completed: {} ({}ms)", step, duration_ms)
                );
            }
            MigrationEvent::BlobProcessed { cid, bytes } => {
                console_debug!("[Event] 📦 Blob processed: {} ({} bytes)", cid, bytes);
            }
            MigrationEvent::BlobFailed { cid, error } => {
                console_warn!("{}", format!("[Event] ❌ Blob failed: {} - {}", cid, error));
            }
            MigrationEvent::Warning { message } => {
                console_warn!("{}", format!("[Event] ⚠️ Warning: {}", message));
            }
            MigrationEvent::Error { message } => {
                console_error!("{}", format!("[Event] ❌ Error: {}", message));
            }
            MigrationEvent::Completed { success } => {
                if success {
                    console_info!("[Event] 🎉 Migration completed successfully");
                } else {
                    console_error!("[Event] ❌ Migration failed");
                }
            }
        }
    }
}
