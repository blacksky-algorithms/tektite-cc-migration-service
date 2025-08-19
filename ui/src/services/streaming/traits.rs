//! Core traits for the WASM-first streaming migration architecture

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{Stream, Future};
use std::error::Error;
use std::pin::Pin;
use tokio::sync::mpsc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;
use js_sys::{Uint8Array, Reflect};
use std::task::{Context, Poll};

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
}

impl BrowserStream {
    pub fn from_response(response: Response) -> Result<Self, JsValue> {
        let body = response.body().ok_or_else(|| JsValue::from_str("No body in response"))?;
        let reader = body.get_reader().unchecked_into();
        Ok(Self { reader })
    }
}

impl Stream for BrowserStream {
    type Item = Result<Bytes, String>;
    
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let future = JsFuture::from(self.reader.read());
        let mut future = Box::pin(future);
        
        match future.as_mut().poll(cx) {
            Poll::Ready(Ok(value)) => {
                let done = Reflect::get(&value, &"done".into())
                    .unwrap()
                    .as_bool()
                    .unwrap_or(false);
                    
                if done {
                    Poll::Ready(None)
                } else {
                    let chunk = Reflect::get(&value, &"value".into()).unwrap();
                    let uint8_array: Uint8Array = chunk.dyn_into().unwrap();
                    let bytes = uint8_array.to_vec();
                    Poll::Ready(Some(Ok(Bytes::from(bytes))))
                }
            }
            Poll::Ready(Err(e)) => {
                Poll::Ready(Some(Err(format!("Read error: {:?}", e))))
            }
            Poll::Pending => Poll::Pending,
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
    async fn upload_data(&self, id: String, data: Vec<u8>, content_type: &str) -> Result<(), Box<dyn Error>>;
    
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
pub struct ChannelTee {
    channels: Vec<mpsc::Sender<DataChunk>>,
}

impl ChannelTee {
    /// Create a new channel tee with specified capacity and number of output channels
    pub fn new(capacity: usize, num_outputs: usize) -> (Self, Vec<mpsc::Receiver<DataChunk>>) {
        let mut senders = Vec::new();
        let mut receivers = Vec::new();
        
        for _ in 0..num_outputs {
            let (tx, rx) = mpsc::channel(capacity);
            senders.push(tx);
            receivers.push(rx);
        }
        
        (Self { channels: senders }, receivers)
    }
    
    /// Send a data chunk to all output channels
    pub async fn send(&self, chunk: DataChunk) -> Result<(), mpsc::error::SendError<DataChunk>> {
        for tx in &self.channels {
            tx.send(chunk.clone()).await?;
        }
        Ok(())
    }
    
    /// Close all channels
    pub fn close(&mut self) {
        self.channels.clear();
    }
}

impl Clone for ChannelTee {
    fn clone(&self) -> Self {
        Self {
            channels: self.channels.clone(),
        }
    }
}