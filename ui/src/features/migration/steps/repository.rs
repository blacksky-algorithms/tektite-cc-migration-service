//! Repository migration step

#[cfg(feature = "web")]
use crate::services::client::{ClientSessionCredentials, PdsClient};
use dioxus::prelude::*;
use crate::{console_info};

use crate::features::migration::types::*;

/// Migrate repository from old PDS to new PDS
// NEWBOLD.md Steps: goat repo export $ACCOUNTDID (line 76) + goat repo import ./did:plc:do2ar6uqzrvyzq3wevji6fbe.20250625142552.car (line 81)
// Implements: Complete repository migration using CAR file export/import
pub async fn migrate_repository_client_side(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
) -> Result<(), String> {
    // Step 7: Export repository from old PDS
    // NEWBOLD.md Step: goat repo export $ACCOUNTDID (line 76)
    // Implements: Exports repository as CAR file for migration
    console_info!("[Migration] Step 7: Exporting repository from old PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Exporting repository from old PDS...".to_string(),
    ));

    let pds_client = PdsClient::new();

    let car_data = match pds_client.export_repository(old_session).await {
        Ok(response) => {
            if response.success {
                let car_size = response.car_size.unwrap_or(0);
                console_info!(
                    "[Migration] Repository exported successfully, size: {} bytes",
                    car_size.to_string()
                );

                // Update repo progress
                let repo_progress = RepoProgress {
                    export_complete: true,
                    car_size,
                    ..Default::default()
                };
                dispatch.call(MigrationAction::SetRepoProgress(repo_progress));

                response.car_data.unwrap_or_default()
            } else {
                return Err(response.message);
            }
        }
        Err(e) => return Err(format!("Failed to export repository: {}", e)),
    };

    // Step 8: Import repository to new PDS
    // NEWBOLD.md Step: goat repo import ./did:plc:do2ar6uqzrvyzq3wevji6fbe.20250625142552.car (line 81)
    // Implements: Imports repository CAR file to new PDS
    console_info!("[Migration] Step 8: Importing repository to new PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Importing repository to new PDS...".to_string(),
    ));

    let car_size = car_data.len() as u64;
    match pds_client.import_repository(new_session, car_data).await {
        Ok(response) => {
            if response.success {
                console_info!("[Migration] Repository imported successfully");

                // Update repo progress
                let repo_progress = RepoProgress {
                    export_complete: true,
                    import_complete: true,
                    car_size,
                    error: None,
                };
                dispatch.call(MigrationAction::SetRepoProgress(repo_progress));

                Ok(())
            } else {
                Err(response.message)
            }
        }
        Err(e) => Err(format!("Failed to import repository: {}", e)),
    }
}
