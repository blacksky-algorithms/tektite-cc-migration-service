//! Progress reporting abstraction for migration operations

use crate::migration::types::*;
use crate::{console_error, console_info};

/// Trait for reporting migration progress
pub trait ProgressReporter {
    fn report_step(&self, step: MigrationStep);
    fn report_blob_progress(&self, progress: BlobProgress);
    fn report_error(&self, error: &str);
    fn report_completion(&self, result: MigrationResult);
}

/// Migration steps for progress reporting
#[derive(Debug, Clone)]
pub enum MigrationStep {
    RepositoryExport,
    RepositoryImport,
    BlobDiscovery,
    BlobMigration,
    PreferencesExport,
    PreferencesImport,
    PlcRecommendation,
    PlcTokenRequest,
}

/// Migration result summary
#[derive(Debug, Clone)]
pub struct MigrationResult {
    pub success: bool,
    pub total_blobs_migrated: u32,
    pub total_bytes_processed: u64,
    pub errors: Vec<String>,
    pub duration_seconds: u64,
}

/// Console-based progress reporter for debugging
pub struct ConsoleProgressReporter;

impl ProgressReporter for ConsoleProgressReporter {
    fn report_step(&self, step: MigrationStep) {
        match step {
            MigrationStep::RepositoryExport => {
                console_info!("[Progress] 📦 Repository Export: Exporting data from old PDS");
            }
            MigrationStep::RepositoryImport => {
                console_info!("[Progress] 📥 Repository Import: Importing data to new PDS");
            }
            MigrationStep::BlobDiscovery => {
                console_info!("[Progress] 🔍 Blob Discovery: Finding missing blobs");
            }
            MigrationStep::BlobMigration => {
                console_info!("[Progress] 🚛 Blob Migration: Transferring blob data");
            }
            MigrationStep::PreferencesExport => {
                console_info!("[Progress] ⚙️ Preferences Export: Exporting user preferences");
            }
            MigrationStep::PreferencesImport => {
                console_info!("[Progress] ⚙️ Preferences Import: Importing user preferences");
            }
            MigrationStep::PlcRecommendation => {
                console_info!("[Progress] 📋 PLC Recommendation: Getting PLC transition details");
            }
            MigrationStep::PlcTokenRequest => {
                console_info!("[Progress] 🎫 PLC Token Request: Requesting verification token");
            }
        }
    }

    fn report_blob_progress(&self, progress: BlobProgress) {
        if let Some(current_cid) = &progress.current_blob_cid {
            console_info!(
                "[Progress] 📊 Blob Progress: {}/{} blobs ({:.1}% complete) - Current: {}",
                progress.processed_blobs,
                progress.total_blobs,
                (progress.processed_blobs as f64 / progress.total_blobs as f64) * 100.0,
                current_cid
            );
        } else {
            console_info!(
                "[Progress] 📊 Blob Progress: {}/{} blobs ({:.1}% complete)",
                progress.processed_blobs,
                progress.total_blobs,
                (progress.processed_blobs as f64 / progress.total_blobs as f64) * 100.0
            );
        }
    }

    fn report_error(&self, error: &str) {
        console_error!("{}", format!("[Progress] ❌ Migration Error: {}", error));
    }

    fn report_completion(&self, result: MigrationResult) {
        if result.success {
            console_info!(
                "[Progress] ✅ Migration Complete: {} blobs migrated ({:.1} MB) in {}s",
                result.total_blobs_migrated,
                result.total_bytes_processed as f64 / 1_048_576.0,
                result.duration_seconds
            );
        } else {
            console_error!(
                "[Progress] ❌ Migration Failed: {} errors occurred",
                result.errors.len()
            );
            for error in &result.errors {
                console_error!("{}", format!("[Progress]   - {}", error));
            }
        }
    }
}

/// UI-based progress reporter that dispatches to the application state
pub struct UiProgressReporter<F>
where
    F: Fn(MigrationAction) + Clone,
{
    dispatch: F,
}

impl<F> UiProgressReporter<F>
where
    F: Fn(MigrationAction) + Clone,
{
    pub fn new(dispatch: F) -> Self {
        Self { dispatch }
    }
}

impl<F> ProgressReporter for UiProgressReporter<F>
where
    F: Fn(MigrationAction) + Clone,
{
    fn report_step(&self, step: MigrationStep) {
        let message = match step {
            MigrationStep::RepositoryExport => "Exporting repository from old PDS...",
            MigrationStep::RepositoryImport => "Importing repository to new PDS...",
            MigrationStep::BlobDiscovery => "Discovering missing blobs...",
            MigrationStep::BlobMigration => "Migrating blob data...",
            MigrationStep::PreferencesExport => "Exporting preferences...",
            MigrationStep::PreferencesImport => "Importing preferences...",
            MigrationStep::PlcRecommendation => "Getting PLC recommendation...",
            MigrationStep::PlcTokenRequest => "Requesting PLC token...",
        };

        (self.dispatch)(MigrationAction::SetMigrationStep(message.to_string()));
    }

    fn report_blob_progress(&self, progress: BlobProgress) {
        (self.dispatch)(MigrationAction::SetBlobProgress(progress));
    }

    fn report_error(&self, error: &str) {
        (self.dispatch)(MigrationAction::SetMigrationError(Some(error.to_string())));
    }

    fn report_completion(&self, _result: MigrationResult) {
        (self.dispatch)(MigrationAction::SetMigrationCompleted(true));
    }
}
