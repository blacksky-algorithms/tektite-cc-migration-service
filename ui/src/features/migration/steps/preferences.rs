//! Preferences migration step

#[cfg(feature = "web")]
use crate::services::client::{ClientSessionCredentials, PdsClient};
use dioxus::prelude::*;
use gloo_console as console;

use crate::features::migration::types::*;

/// Migrate preferences from old PDS to new PDS
// NEWBOLD.md Steps: goat bsky prefs export > prefs.json (line 115) + goat bsky prefs import prefs.json (line 118)
// Implements: Complete preferences migration for Bluesky app settings
pub async fn migrate_preferences_client_side(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    // Step 14: Export preferences from old PDS
    // NEWBOLD.md Step: goat bsky prefs export > prefs.json (line 115)
    // Implements: Exports Bluesky app preferences as JSON
    console::info!("[Migration] Step 14: Exporting preferences from old PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Exporting preferences from old PDS...".to_string(),
    ));

    let pds_client = PdsClient::new();

    let preferences_json = match pds_client.export_preferences(old_session).await {
        Ok(response) => {
            if response.success {
                console::info!("[Migration] Preferences exported successfully");

                // Update preferences progress
                let prefs_progress = PreferencesProgress {
                    export_complete: true,
                    ..Default::default()
                };
                dispatch.call(MigrationAction::SetPreferencesProgress(prefs_progress));

                response.preferences_json.unwrap_or_default()
            } else {
                return Err(response.message);
            }
        }
        Err(e) => return Err(format!("Failed to export preferences: {}", e)),
    };

    // Step 15: Import preferences to new PDS
    // NEWBOLD.md Step: goat bsky prefs import prefs.json (line 118)
    // Implements: Imports Bluesky app preferences to new PDS
    console::info!("[Migration] Step 15: Importing preferences to new PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Importing preferences to new PDS...".to_string(),
    ));

    match pds_client.import_preferences(new_session, preferences_json).await {
        Ok(response) => {
            if response.success {
                console::info!("[Migration] Preferences imported successfully");

                // Update preferences progress
                let mut prefs_progress = state.preferences_progress.clone();
                prefs_progress.import_complete = true;
                dispatch.call(MigrationAction::SetPreferencesProgress(prefs_progress));

                // Update migration progress
                let mut migration_progress = state.migration_progress.clone();
                migration_progress.preferences_exported = true;
                migration_progress.preferences_imported = true;
                dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));

                Ok(())
            } else {
                Err(response.message)
            }
        }
        Err(e) => Err(format!("Failed to import preferences: {}", e)),
    }
}