use dioxus::prelude::*;

use crate::features::migration::*;

#[derive(Props, PartialEq, Clone)]
pub struct BlobProgressDisplayProps {
    pub blob_progress: BlobProgress,
    pub migration_step: String,
}

#[component]
pub fn BlobProgressDisplay(props: BlobProgressDisplayProps) -> Element {
    let blob_progress = &props.blob_progress;
    let migration_step = &props.migration_step;

    // Calculate progress percentage
    let progress_percentage = if blob_progress.total_blobs > 0 {
        (blob_progress.processed_blobs as f64 / blob_progress.total_blobs as f64) * 100.0
    } else {
        0.0
    };

    // Format data sizes
    let total_mb = blob_progress.total_bytes as f64 / 1_048_576.0;
    let processed_mb = blob_progress.processed_bytes as f64 / 1_048_576.0;

    rsx! {
        div {
            class: "blob-progress-container",

            h4 {
                class: "blob-progress-title",
                "Blob Transfer Progress"
            }

            // Overall progress bar
            div {
                class: "progress-bar-container",
                div {
                    class: "progress-bar-background",
                    div {
                        class: "progress-bar-fill",
                        style: format!("width: {}%", progress_percentage),
                    }
                }
                span {
                    class: "progress-percentage",
                    "{progress_percentage:.1}%"
                }
            }

            // Status text
            div {
                class: "blob-status-text",
                "{migration_step}"
            }

            // Detailed statistics
            div {
                class: "blob-stats",
                div {
                    class: "blob-stat-item",
                    span { class: "stat-label", "Blobs:" }
                    span { class: "stat-value", "{blob_progress.processed_blobs}/{blob_progress.total_blobs}" }
                }
                div {
                    class: "blob-stat-item",
                    span { class: "stat-label", "Size:" }
                    span { class: "stat-value", "{processed_mb:.1}/{total_mb:.1} MB" }
                }
            }

            // Current blob being processed
            if let Some(current_cid) = &blob_progress.current_blob_cid {
                div {
                    class: "current-blob",
                    div {
                        class: "current-blob-label",
                        "Currently processing:"
                    }
                    div {
                        class: "current-blob-cid",
                        "{current_cid}"
                    }
                    if let Some(current_progress) = blob_progress.current_blob_progress {
                        div {
                            class: "current-blob-progress",
                            div {
                                class: "mini-progress-bar",
                                div {
                                    class: "mini-progress-fill",
                                    style: format!("width: {}%", current_progress),
                                }
                            }
                            span {
                                class: "mini-progress-text",
                                "{current_progress:.0}%"
                            }
                        }
                    }
                }
            }

            // Error display if any
            if let Some(error) = &blob_progress.error {
                div {
                    class: "blob-error",
                    "⚠️ {error}"
                }
            }

            // Recent blobs list (simulated for now)
            if blob_progress.processed_blobs > 0 {
                div {
                    class: "recent-blobs-section",
                    h5 {
                        class: "recent-blobs-title",
                        "Recently Processed Blobs:"
                    }
                    div {
                        class: "recent-blobs-list",
                        // Show up to 5 recent "blobs" (simulated)
                        {(1..=(blob_progress.processed_blobs.min(5))).map(|i| rsx! {
                            div {
                                key: "{i}",
                                class: "recent-blob-item",
                                span {
                                    class: "blob-status-icon",
                                    "✓"
                                }
                                span {
                                    class: "blob-description",
                                    "Blob #{i} transferred successfully"
                                }
                            }
                        })}
                        {if blob_progress.processed_blobs > 5 {
                            let remaining = blob_progress.processed_blobs - 5;
                            rsx! {
                                div {
                                    class: "more-blobs-indicator",
                                    "... and {remaining} more blobs"
                                }
                            }
                        } else {
                            rsx! {}
                        }}
                    }
                }
            }
        }
    }
}
