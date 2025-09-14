//! Core traits for the WASM-first streaming migration architecture

use crate::{console_debug, console_error, console_info, console_warn};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{Future, Stream};
use std::error::Error;
use std::pin::Pin;
use tokio::sync::mpsc;

#[cfg(target_arch = "wasm32")]
use gloo_timers::future::TimeoutFuture;
use js_sys::{Reflect, Uint8Array};
use std::task::{Context, Poll};
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::{timeout, Duration};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;

/// Generic data chunk that can represent either repo or blob data
#[derive(Clone, Debug)]
pub struct DataChunk {
    /// Identifier (DID for repo, CID for blob)
    pub id: String,
    /// The actual data bytes
    pub data: Bytes,
    /// Offset within the stream
    pub offset: usize,
    /// Total size if known
    pub total_size: Option<usize>,
}

/// Browser stream wrapper for ReadableStreamDefaultReader
pub struct BrowserStream {
    reader: web_sys::ReadableStreamDefaultReader,
    /// Persistent future for the current read operation - reused across poll calls
    current_read: Option<Pin<Box<JsFuture>>>,
}

impl BrowserStream {
    pub fn from_response(response: Response) -> Result<Self, JsValue> {
        console_info!("[BrowserStream] Extracting body from Response");

        let body = response.body().ok_or_else(|| {
            console_error!("[BrowserStream] No body in response");
            JsValue::from_str("No body in response")
        })?;

        console_debug!("[BrowserStream] Body extracted successfully, creating reader");

        let reader = body.get_reader().unchecked_into();

        console_info!("[BrowserStream] ReadableStreamDefaultReader initialized successfully");

        Ok(Self {
            reader,
            current_read: None,
        })
    }

    /// Fallback method using arrayBuffer() instead of ReadableStream
    /// Use this if ReadableStream continues to hang
    pub async fn from_response_array_buffer(response: Response) -> Result<Vec<u8>, JsValue> {
        console_info!("[BrowserStream] Using arrayBuffer fallback");

        let array_buffer_promise = response.array_buffer().map_err(|e| {
            console_error!("[BrowserStream] Failed to get arrayBuffer: {:?}", e);
            e
        })?;

        let array_buffer_js = JsFuture::from(array_buffer_promise).await?;
        let uint8_array = Uint8Array::new(&array_buffer_js);
        let bytes = uint8_array.to_vec();

        console_info!(
            "[BrowserStream] ArrayBuffer fallback completed: {} bytes",
            bytes.len()
        );
        Ok(bytes)
    }
}

impl Stream for BrowserStream {
    type Item = Result<Bytes, String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        console_debug!("[BrowserStream] Starting poll_next");

        // Create future only if we don't have one active
        if self.current_read.is_none() {
            console_debug!("[BrowserStream] Creating new read future");
            let future = JsFuture::from(self.reader.read());
            self.current_read = Some(Box::pin(future));
        }

        // Poll the existing future
        let current_read = self.current_read.as_mut().unwrap();
        match current_read.as_mut().poll(cx) {
            Poll::Ready(Ok(value)) => {
                console_debug!("[BrowserStream] Poll ready with value");

                // Clear the future since this read is complete
                self.current_read = None;

                let done = Reflect::get(&value, &"done".into())
                    .unwrap_or_else(|e| {
                        console_error!("[BrowserStream] Error getting 'done' field: {:?}", e);
                        JsValue::from(false)
                    })
                    .as_bool()
                    .unwrap_or(false);

                if done {
                    console_info!("[BrowserStream] Stream completed (done=true)");
                    Poll::Ready(None)
                } else {
                    match Reflect::get(&value, &"value".into()) {
                        Ok(chunk) => match chunk.dyn_into::<Uint8Array>() {
                            Ok(uint8_array) => {
                                let bytes = uint8_array.to_vec();
                                let chunk_size = bytes.len();
                                console_debug!("[BrowserStream] Read chunk: {} bytes", chunk_size);
                                Poll::Ready(Some(Ok(Bytes::from(bytes))))
                            }
                            Err(e) => {
                                let error_msg = format!(
                                    "[BrowserStream] Failed to convert chunk to Uint8Array: {:?}",
                                    e
                                );
                                console_error!("{}", error_msg);
                                Poll::Ready(Some(Err(error_msg)))
                            }
                        },
                        Err(e) => {
                            let error_msg =
                                format!("[BrowserStream] Error getting 'value' field: {:?}", e);
                            console_error!("{}", error_msg);
                            Poll::Ready(Some(Err(error_msg)))
                        }
                    }
                }
            }
            Poll::Ready(Err(e)) => {
                console_error!("[BrowserStream] Read error: {:?}", e);

                // Clear the future since this read failed
                self.current_read = None;

                let error_msg = format!("[BrowserStream] Read error: {:?}", e);
                Poll::Ready(Some(Err(error_msg)))
            }
            Poll::Pending => {
                console_debug!("[BrowserStream] Poll pending - waiting for more data");
                Poll::Pending
            }
        }
    }
}

/// Trait for source operations (fetching data) - WASM-only
#[async_trait(?Send)]
pub trait DataSource {
    type Item;

    /// List all items available from this source
    async fn list_items(&self) -> Result<Vec<Self::Item>, Box<dyn Error>>;

    /// Fetch a stream of bytes for a specific item
    async fn fetch_stream(&self, item: &Self::Item) -> Result<BrowserStream, Box<dyn Error>>;
}

/// Trait for target operations (uploading data) - WASM-only  
#[async_trait(?Send)]
pub trait DataTarget {
    /// Upload data for a specific ID
    async fn upload_data(
        &self,
        id: String,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<(), Box<dyn Error>>;

    /// Upload a chunk of data for streaming uploads (default fallback to upload_data)
    async fn upload_chunk(
        &self,
        id: String,
        chunk: Vec<u8>,
        _offset: usize,
        is_final: bool,
        content_type: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Default implementation - collect chunks and upload at the end
        // Implementations should override this for true streaming
        if is_final {
            self.upload_data(id, chunk, content_type).await
        } else {
            // For now, just accumulate - real implementations should stream
            Ok(())
        }
    }

    /// List items that are missing and need to be uploaded
    async fn list_missing(&self) -> Result<Vec<String>, Box<dyn Error>>;
}

/// Trait for storage operations - WASM-only
#[async_trait(?Send)]
pub trait StorageBackend {
    /// Write a chunk of data to storage
    async fn write_chunk(&mut self, chunk: &DataChunk) -> Result<(), Box<dyn Error>>;

    /// Finalize storage for a specific item (flush buffers, close files, etc.)
    async fn finalize(&mut self, id: &str) -> Result<(), Box<dyn Error>>;

    /// Read back a stored item as bytes (for uploads)
    async fn read_data(&self, id: &str) -> Result<Vec<u8>, Box<dyn Error>>;
}

/// Channel tee pattern - duplicates stream data to multiple channels (WASM-compatible)
pub struct ChannelTee<const CAPACITY: usize> {
    channels: Vec<mpsc::Sender<DataChunk>>,
}

impl<const CAPACITY: usize> ChannelTee<CAPACITY> {
    /// Create a new channel tee with specified number of output channels
    pub fn new(num_outputs: usize) -> (Self, Vec<mpsc::Receiver<DataChunk>>) {
        let mut senders = Vec::new();
        let mut receivers = Vec::new();

        for _ in 0..num_outputs {
            let (tx, rx) = mpsc::channel(CAPACITY);
            senders.push(tx);
            receivers.push(rx);
        }

        (Self { channels: senders }, receivers)
    }

    /// Send a data chunk to all output channels with backpressure handling and timeout detection
    pub async fn send(&self, chunk: DataChunk) -> Result<(), Box<dyn Error>> {
        console_debug!(
            "[ChannelTee] Sending chunk for {} ({} bytes) to {} channels",
            chunk.id,
            chunk.data.len(),
            self.channels.len()
        );

        for (i, tx) in self.channels.iter().enumerate() {
            // WASM version: Use try_send for immediate backpressure detection
            match tx.try_send(chunk.clone()) {
                Ok(()) => {
                    console_debug!(
                        "[ChannelTee] Successfully sent chunk to channel {} immediately",
                        i
                    );
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    console_warn!(
                        "[ChannelTee] Backpressure detected on channel {}, consumer may be slow",
                        i
                    );

                    // Wait briefly and try blocking send
                    #[cfg(target_arch = "wasm32")]
                    {
                        TimeoutFuture::new(100).await;

                        // Add timeout detection for blocking send
                        let send_start = js_sys::Date::now();
                        match tx.send(chunk.clone()).await {
                            Ok(()) => {
                                let send_duration = js_sys::Date::now() - send_start;
                                console_info!(
                                    "[ChannelTee] Recovered from backpressure on channel {} in {:.1}ms",
                                    i,
                                    send_duration
                                );
                            }
                            Err(_) => {
                                console_error!(
                                    "[ChannelTee] Channel {} closed during backpressure recovery",
                                    i
                                );
                                return Err(format!(
                                    "Channel {} closed during backpressure recovery",
                                    i
                                )
                                .into());
                            }
                        }

                        // Check if send took too long (possible stall)
                        let total_duration = js_sys::Date::now() - send_start;
                        if total_duration > 5000.0 {
                            // 5 seconds
                            console_warn!(
                                "[ChannelTee] Slow channel send detected: {:.1}ms for channel {}",
                                total_duration,
                                i
                            );
                        }
                    }

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        use tokio::time::{timeout, Duration};

                        // Try again with a longer timeout
                        match timeout(Duration::from_millis(1000), tx.send(chunk.clone())).await {
                            Ok(Ok(())) => {
                                console_info!(
                                    "[ChannelTee] Recovered from backpressure on channel {}",
                                    i
                                );
                            }
                            Ok(Err(_)) => {
                                return Err(format!(
                                    "Channel {} closed during backpressure recovery",
                                    i
                                )
                                .into());
                            }
                            Err(_) => {
                                return Err(format!(
                                    "Persistent backpressure on channel {}, aborting",
                                    i
                                )
                                .into());
                            }
                        }
                    }
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    console_error!("[ChannelTee] Channel {} closed unexpectedly", i);
                    return Err(format!("Channel {} closed unexpectedly", i).into());
                }
            }
        }
        Ok(())
    }

    /// Close all channels
    pub fn close(&mut self) {
        self.channels.clear();
    }
}

impl<const CAPACITY: usize> Clone for ChannelTee<CAPACITY> {
    fn clone(&self) -> Self {
        Self {
            channels: self.channels.clone(),
        }
    }
}
