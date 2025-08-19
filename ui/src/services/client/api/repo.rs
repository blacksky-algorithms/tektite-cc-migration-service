//! Repository API operations for ATProto PDS
//!
//! This module contains repository-related API operations extracted from PdsClient
//! for better code organization. All functions take PdsClient as first parameter
//! and implement repository management functionality.

use anyhow::Result;
use reqwest::header;
use tracing::{error, info, instrument};
use cid::Cid;
use std::convert::TryFrom;

// Import console macros from our crate
use crate::console_debug;

use crate::services::client::errors::ClientError;
use crate::services::client::PdsClient;
use crate::services::client::types::{
    ClientSessionCredentials,
    ClientRepoExportResponse,
    ClientRepoImportResponse,
    ClientMissingBlobsResponse,
    ClientMissingBlob,
    ClientSyncListBlobsResponse,
};

/// Export repository from PDS as CAR file
// NEWBOLD.md Step: goat repo export $ACCOUNTDID (line 76)
// Implements: Exports repository as CAR file for migration
#[instrument(skip(client), err)]
pub async fn export_repository_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
) -> Result<ClientRepoExportResponse, ClientError> {
    info!("Exporting repository for DID: {}", session.did);

    // NEWBOLD.md: com.atproto.sync.getRepo for repository export
    let export_url = format!(
        "{}/xrpc/com.atproto.sync.getRepo?did={}",
        session.pds, session.did
    );

    let response = client
        .http_client
        .get(&export_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        // Tell server we accept gzip compression
        .header(header::ACCEPT_ENCODING, "gzip, deflate")
        .send()
        .await
        .map_err(|e| ClientError::NetworkError {
            message: format!("Failed to export repository: {}", e),
        })?;

    if response.status().is_success() {
        // reqwest automatically handles decompression when Accept-Encoding is set
        // The response.bytes() will give us decompressed data
        let car_bytes = response
            .bytes()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to read CAR data: {}", e),
            })?;

        let car_data = car_bytes.to_vec();
        let car_size = car_data.len() as u64;

        console_debug!(
            "Received CAR file: {} bytes (after decompression)",
            car_size
        );

        info!(
            "Repository exported successfully, size: {} bytes",
            car_size.to_string()
        );

        Ok(ClientRepoExportResponse {
            success: true,
            message: "Repository exported successfully".to_string(),
            car_data: Some(car_data),
            car_size: Some(car_size),
        })
    } else {
        let error_text = response.text().await.unwrap_or_default();
        error!("Repository export failed: {}", error_text);

        Ok(ClientRepoExportResponse {
            success: false,
            message: format!("Repository export failed: {}", error_text),
            car_data: None,
            car_size: None,
        })
    }
}

/// Import repository to PDS from CAR file
// NEWBOLD.md Step: goat repo import ./did:plc:do2ar6uqzrvyzq3wevji6fbe.20250625142552.car (line 81)
// Implements: Imports repository CAR file to new PDS
#[instrument(skip(client), err)]
pub async fn import_repository_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
    car_data: Vec<u8>,
) -> Result<ClientRepoImportResponse, ClientError> {
    info!(
        "Importing repository for DID: {}, CAR size: {} bytes",
        session.did,
        car_data.len()
    );

    // NEWBOLD.md: com.atproto.repo.importRepo for CAR file import
    let import_url = format!("{}/xrpc/com.atproto.repo.importRepo", session.pds);

    // Don't compress - server expects raw CAR data
    // Server will compress the response if needed
    let response = client
        .http_client
        .post(&import_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .header("Content-Type", "application/vnd.ipld.car")
        .header("Content-Length", car_data.len().to_string()) // Required!
        .body(car_data)
        .send()
        .await
        .map_err(|e| ClientError::NetworkError {
            message: format!("Failed to import repository: {}", e),
        })?;

    if response.status().is_success() {
        info!("Repository imported successfully");

        Ok(ClientRepoImportResponse {
            success: true,
            message: "Repository imported successfully".to_string(),
        })
    } else {
        let error_text = response.text().await.unwrap_or_default();
        error!("Repository import failed: {}", error_text);

        Ok(ClientRepoImportResponse {
            success: false,
            message: format!("Repository import failed: {}", error_text),
        })
    }
}

/// Get list of missing blobs for account
// NEWBOLD.md Step: goat account missing-blobs (line 86)
// Implements: Lists missing blobs that need migration to new PDS
#[instrument(skip(client), err)]
pub async fn get_missing_blobs_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
    cursor: Option<String>,
    limit: Option<i64>,
) -> Result<ClientMissingBlobsResponse, ClientError> {
    info!("Getting missing blobs for DID: {}", session.did);

    // NEWBOLD.md: com.atproto.repo.listMissingBlobs for migration-specific blob enumeration
    let mut missing_blobs_url =
        format!("{}/xrpc/com.atproto.repo.listMissingBlobs", session.pds);
    let mut query_params = Vec::new();

    if let Some(cursor) = cursor {
        query_params.push(format!("cursor={}", cursor));
    }
    if let Some(limit) = limit {
        query_params.push(format!("limit={}", limit));
    }

    if !query_params.is_empty() {
        missing_blobs_url.push('?');
        missing_blobs_url.push_str(&query_params.join("&"));
    }

    let response = client
        .http_client
        .get(&missing_blobs_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .send()
        .await
        .map_err(|e| ClientError::NetworkError {
            message: format!("Failed to get missing blobs: {}", e),
        })?;

    if response.status().is_success() {
        let blobs_data: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse missing blobs response: {}", e),
                })?;

        // Parse the blobs from the response using proper deserialization
        let missing_blobs =
            if let Some(blobs_array) = blobs_data.get("blobs").and_then(|b| b.as_array()) {
                blobs_array
                    .iter()
                    .filter_map(|blob| {
                        serde_json::from_value::<ClientMissingBlob>(blob.clone()).ok()
                    })
                    .collect()
            } else {
                Vec::new()
            };

        let cursor = blobs_data
            .get("cursor")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        info!("Found {} missing blobs", missing_blobs.len().to_string());

        Ok(ClientMissingBlobsResponse {
            success: true,
            message: format!("Found {} missing blobs", missing_blobs.len()),
            missing_blobs: Some(missing_blobs),
            cursor,
        })
    } else {
        let error_text = response.text().await.unwrap_or_default();
        error!("Failed to get missing blobs: {}", error_text);

        Ok(ClientMissingBlobsResponse {
            success: false,
            message: format!("Failed to get missing blobs: {}", error_text),
            missing_blobs: None,
            cursor: None,
        })
    }
}

/// List all blobs in repository using com.atproto.sync.listBlobs (matches Go goat)
/// This method provides full blob enumeration like the Go SyncListBlobs implementation
// NEWBOLD.md Compatible: Matches goat blob export enumeration pattern for full repository listing
// Implements: Full blob enumeration using com.atproto.sync.listBlobs (Go goat compatible)
#[instrument(skip(client), err)]
pub async fn sync_list_blobs_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
    did: &str,
    cursor: Option<String>,
    limit: Option<i64>,
    since: Option<String>,
) -> Result<ClientSyncListBlobsResponse, ClientError> {
    info!("Listing all blobs for DID: {} (sync.listBlobs)", did);

    // NEWBOLD.md: com.atproto.sync.listBlobs for Go goat compatible full blob enumeration
    let mut list_blobs_url = format!("{}/xrpc/com.atproto.sync.listBlobs", session.pds);
    let mut query_params = Vec::new();

    // Required parameter
    query_params.push(format!("did={}", did));

    // Optional parameters
    if let Some(cursor) = cursor {
        query_params.push(format!("cursor={}", cursor));
    }
    if let Some(limit) = limit {
        query_params.push(format!("limit={}", limit));
    }
    if let Some(since) = since {
        query_params.push(format!("since={}", since));
    }

    list_blobs_url.push('?');
    list_blobs_url.push_str(&query_params.join("&"));

    let response = client
        .http_client
        .get(&list_blobs_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .send()
        .await
        .map_err(|e| ClientError::NetworkError {
            message: format!("Failed to list blobs: {}", e),
        })?;

    if response.status().is_success() {
        let blobs_data: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse list blobs response: {}", e),
                })?;

        // Parse the CIDs array from the response and validate each CID
        let cids = if let Some(cids_array) = blobs_data.get("cids").and_then(|c| c.as_array()) {
            cids_array
                .iter()
                .filter_map(|cid| {
                    cid.as_str()
                        .and_then(|s| Cid::try_from(s).ok())
                })
                .collect()
        } else {
            Vec::new()
        };

        let cursor = blobs_data
            .get("cursor")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        info!("Found {} blobs in repository", cids.len());

        Ok(ClientSyncListBlobsResponse {
            success: true,
            message: format!("Found {} blobs", cids.len()),
            cids: Some(cids),
            cursor,
        })
    } else {
        let error_text = response.text().await.unwrap_or_default();
        error!("Failed to list blobs: {}", error_text);

        Ok(ClientSyncListBlobsResponse {
            success: false,
            message: format!("Failed to list blobs: {}", error_text),
            cids: None,
            cursor: None,
        })
    }
}

/// List ALL blobs from source PDS with automatic pagination (Go goat runBlobExport compatible)
/// This method provides complete blob enumeration like the Go SyncListBlobs with pagination
// NEWBOLD.md Compatible: Full blob enumeration with pagination like Go goat blob export
// Implements: Complete source blob inventory using com.atproto.sync.listBlobs with auto-pagination
#[instrument(skip(client), err)]
pub async fn list_all_source_blobs_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
    did: &str,
) -> Result<Vec<Cid>, ClientError> {
    info!("Listing ALL blobs for DID: {} (complete enumeration)", did);

    let mut all_cids = Vec::new();
    let mut cursor: Option<String> = None;

    // Paginate through all blobs, matching Go goat runBlobExport pattern
    loop {
        console_debug!(
            "[PdsClient] Fetching blob list batch with cursor: {:?}",
            cursor.as_ref().unwrap_or(&"<none>".to_string())
        );

        match sync_list_blobs_impl(client, session, did, cursor.clone(), Some(500), None)
            .await
        {
            Ok(response) => {
                if response.success {
                    if let Some(mut batch_cids) = response.cids {
                        console_debug!(
                            "[PdsClient] Received {} CIDs in this batch",
                            batch_cids.len()
                        );
                        all_cids.append(&mut batch_cids);

                        // Check for pagination continuation - matches Go goat pattern:
                        // if resp.Cursor != nil && *resp.Cursor != ""
                        cursor = if let Some(next_cursor) = response.cursor {
                            if !next_cursor.is_empty() {
                                Some(next_cursor) // Continue with next cursor
                            } else {
                                break; // Empty cursor means no more pages
                            }
                        } else {
                            break; // No cursor means no more pages
                        };
                    } else {
                        break; // No CIDs returned
                    }
                } else {
                    return Err(ClientError::ApiError {
                        message: format!("Failed to list source blobs: {}", response.message),
                    });
                }
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    info!(
        "Completed source blob enumeration: {} total blobs found",
        all_cids.len()
    );

    Ok(all_cids)
}

/// List ALL blobs from target PDS with automatic pagination (for reconciliation)
/// This method provides complete blob enumeration for the target PDS to compare with source
// Implements: Complete target blob inventory using com.atproto.sync.listBlobs with auto-pagination
#[instrument(skip(client), err)]
pub async fn list_all_target_blobs_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
    did: &str,
) -> Result<Vec<Cid>, ClientError> {
    info!(
        "Listing ALL target blobs for DID: {} (complete enumeration)",
        did
    );

    let mut all_cids = Vec::new();
    let mut cursor: Option<String> = None;

    // Paginate through all blobs, using the same pattern as source enumeration
    loop {
        console_debug!(
            "[PdsClient] Fetching target blob list batch with cursor: {:?}",
            cursor.as_ref().unwrap_or(&"<none>".to_string())
        );

        match sync_list_blobs_impl(client, session, did, cursor.clone(), Some(500), None)
            .await
        {
            Ok(response) => {
                if response.success {
                    if let Some(mut batch_cids) = response.cids {
                        console_debug!(
                            "[PdsClient] Received {} target CIDs in this batch",
                            batch_cids.len()
                        );
                        all_cids.append(&mut batch_cids);

                        // Check for pagination continuation - matches Go goat pattern:
                        // if resp.Cursor != nil && *resp.Cursor != ""
                        cursor = if let Some(next_cursor) = response.cursor {
                            if !next_cursor.is_empty() {
                                Some(next_cursor) // Continue with next cursor
                            } else {
                                break; // Empty cursor means no more pages
                            }
                        } else {
                            break; // No cursor means no more pages
                        };
                    } else {
                        break; // No CIDs returned
                    }
                } else {
                    return Err(ClientError::ApiError {
                        message: format!("Failed to list target blobs: {}", response.message),
                    });
                }
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    info!(
        "Completed target blob enumeration: {} total blobs found",
        all_cids.len()
    );

    Ok(all_cids)
}

/// Verify that specific blobs exist on the target PDS using direct getBlob calls
/// This is more reliable than enumeration for recently uploaded blobs due to eventual consistency
pub async fn verify_blobs_exist_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
    cids: &[Cid],
) -> Result<Vec<Cid>, ClientError> {
    info!(
        "Verifying {} blobs exist on target PDS using direct getBlob calls",
        cids.len()
    );

    let mut existing_blobs = Vec::new();

    for cid in cids {
        console_debug!("[PdsClient] Verifying blob exists: {}", cid);

        // Use sync.getBlob to directly check if blob exists
        let url = format!("{}/xrpc/com.atproto.sync.getBlob", &session.pds);

        let response = client
            .http_client
            .get(&url)
            .bearer_auth(&session.access_jwt)
            .query(&[("did", &session.did), ("cid", &cid.to_string())])
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to verify blob {}: {}", cid, e),
            })?;

        if response.status().is_success() {
            console_debug!("[PdsClient] ✅ Blob {} verified as existing", cid);
            existing_blobs.push(*cid);
        } else {
            console_debug!(
                "{}",
                format!(
                    "[PdsClient] ❌ Blob {} not found (status: {})",
                    cid,
                    response.status()
                )
            );
        }
    }

    info!(
        "Blob verification complete: {}/{} blobs confirmed existing",
        existing_blobs.len(),
        cids.len()
    );

    Ok(existing_blobs)
}