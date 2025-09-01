use crate::{console_debug, console_log, migration::*};
use dioxus::prelude::*;

// Main component that orchestrates all sub-components
#[derive(Props, PartialEq, Clone, Debug)]
pub struct BlobProgressDisplayProps {
    pub blob_progress: BlobProgress,
    pub migration_step: String,
}

#[component]
pub fn BlobProgressDisplay(props: BlobProgressDisplayProps) -> Element {
    let blob_progress = &props.blob_progress;
    let migration_step = &props.migration_step;

    crate::console_info!(
        "[COMPONENT] BlobProgressDisplay RENDERING - step='{}', processed={}/{}, bytes={}/{}",
        migration_step,
        blob_progress.processed_blobs,
        blob_progress.total_blobs,
        blob_progress.processed_bytes,
        blob_progress.total_bytes
    );
    console_debug!(
        "[DEBUG BlobProgressDisplay] Blob Progress: {:?}",
        blob_progress.clone()
    );
    console_debug!(
        "[DEBUG BlobProgressDisplay] Migration Step: {:?}",
        migration_step.clone()
    );

    rsx! {
        div {
            class: "blob-progress-container",

            h4 {
                class: "blob-progress-title",
                "Blob Transfer Progress"
            }

            ProgressBar {
                processed: blob_progress.processed_blobs,
                total: blob_progress.total_blobs,
            }

            StatusText {
                text: migration_step.clone(),
            }

            DetailedStats {
                blob_progress: blob_progress.clone(),
            }

            if let Some(current_cid) = &blob_progress.current_blob_cid {
                CurrentBlobDisplay {
                    cid: current_cid.clone(),
                    progress: blob_progress.current_blob_progress,
                }
            }

            if let Some(error) = &blob_progress.error {
                ErrorDisplay {
                    error: error.clone(),
                }
            }

            if blob_progress.processed_blobs > 0 {
                RecentBlobsList {
                    processed_blobs: blob_progress.processed_blobs,
                }
            }
        }
    }
}

// Progress bar component
#[derive(Props, PartialEq, Clone)]
struct ProgressBarProps {
    processed: u32,
    total: u32,
}

#[component]
fn ProgressBar(props: ProgressBarProps) -> Element {
    let progress_percentage = if props.total > 0 {
        (props.processed as f64 / props.total as f64) * 100.0
    } else {
        0.0
    };

    console_log!(
        "[DEBUG ProgressBar] Rendering: {}/{} blobs ({:.1}%)",
        props.processed,
        props.total,
        progress_percentage
    );

    rsx! {
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
    }
}

// Status text component
#[derive(Props, PartialEq, Clone)]
struct StatusTextProps {
    text: String,
}

#[component]
fn StatusText(props: StatusTextProps) -> Element {
    rsx! {
        div {
            class: "blob-status-text",
            "{props.text}"
        }
    }
}

// Detailed statistics component
#[derive(Props, PartialEq, Clone)]
struct DetailedStatsProps {
    blob_progress: BlobProgress,
}

#[component]
fn DetailedStats(props: DetailedStatsProps) -> Element {
    let total_mb = props.blob_progress.total_bytes as f64 / 1_048_576.0;
    let processed_mb = props.blob_progress.processed_bytes as f64 / 1_048_576.0;

    rsx! {
        div {
            class: "blob-stats",
            StatItem {
                label: "Blobs:".to_string(),
                value: format!("{}/{}", props.blob_progress.processed_blobs, props.blob_progress.total_blobs),
            }
            StatItem {
                label: "Size:".to_string(),
                value: format!("{:.1}/{:.1} MB", processed_mb, total_mb),
            }
        }
    }
}

// Individual stat item component
#[derive(Props, PartialEq, Clone)]
struct StatItemProps {
    label: String,
    value: String,
}

#[component]
fn StatItem(props: StatItemProps) -> Element {
    rsx! {
        div {
            class: "blob-stat-item",
            span { class: "stat-label", "{props.label}" }
            span { class: "stat-value", "{props.value}" }
        }
    }
}

// Current blob display component
#[derive(Props, PartialEq, Clone)]
struct CurrentBlobDisplayProps {
    cid: String,
    progress: Option<f64>,
}

#[component]
fn CurrentBlobDisplay(props: CurrentBlobDisplayProps) -> Element {
    rsx! {
        div {
            class: "current-blob",
            div {
                class: "current-blob-label",
                "Currently processing:"
            }
            div {
                class: "current-blob-cid",
                "{props.cid}"
            }
            if let Some(current_progress) = props.progress {
                MiniProgressBar {
                    progress: current_progress,
                }
            }
        }
    }
}

// Mini progress bar component
#[derive(Props, PartialEq, Clone)]
struct MiniProgressBarProps {
    progress: f64,
}

#[component]
fn MiniProgressBar(props: MiniProgressBarProps) -> Element {
    rsx! {
        div {
            class: "current-blob-progress",
            div {
                class: "mini-progress-bar",
                div {
                    class: "mini-progress-fill",
                    style: format!("width: {}%", props.progress),
                }
            }
            span {
                class: "mini-progress-text",
                "{props.progress:.0}%"
            }
        }
    }
}

// Error display component
#[derive(Props, PartialEq, Clone)]
struct ErrorDisplayProps {
    error: String,
}

#[component]
fn ErrorDisplay(props: ErrorDisplayProps) -> Element {
    rsx! {
        div {
            class: "blob-error",
            "⚠️ {props.error}"
        }
    }
}

// Recent blobs list component
#[derive(Props, PartialEq, Clone)]
struct RecentBlobsListProps {
    processed_blobs: u32,
}

#[component]
fn RecentBlobsList(props: RecentBlobsListProps) -> Element {
    let max_visible = 3;
    let visible_count = props.processed_blobs.min(max_visible);

    rsx! {
        div {
            class: "recent-blobs-section",
            h5 {
                class: "recent-blobs-title",
                "Recently Processed Blobs:"
            }
            div {
                class: "recent-blobs-list",
                {(1..=visible_count).map(|i| rsx! {
                    RecentBlobItem {
                        key: "{i}",
                        index: i,
                    }
                })}
                if props.processed_blobs > max_visible {
                    MoreBlobsIndicator {
                        remaining: props.processed_blobs - max_visible,
                    }
                }
            }
        }
    }
}

// Individual recent blob item component
#[derive(Props, PartialEq, Clone)]
struct RecentBlobItemProps {
    index: u32,
}

#[component]
fn RecentBlobItem(props: RecentBlobItemProps) -> Element {
    rsx! {
        div {
            class: "recent-blob-item",
            span {
                class: "blob-status-icon",
                "✓"
            }
            span {
                class: "blob-description",
                "Blob #{props.index} transferred successfully"
            }
        }
    }
}

// More blobs indicator component
#[derive(Props, PartialEq, Clone)]
struct MoreBlobsIndicatorProps {
    remaining: u32,
}

#[component]
fn MoreBlobsIndicator(props: MoreBlobsIndicatorProps) -> Element {
    rsx! {
        div {
            class: "more-blobs-indicator",
            "... and {props.remaining} more blobs"
        }
    }
}
