//! CID (Content Identifier) validation and type safety for AT Protocol
//! 
//! This module provides CID validation matching the Go atproto/syntax implementation
//! to ensure compatibility and security.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter};
use regex::Regex;

/// A validated CID (Content Identifier) string for AT Protocol
/// 
/// This type ensures CIDs conform to AT Protocol requirements:
/// - Length between 8-256 characters
/// - Valid base58/base32 characters only  
/// - CIDv1 format (no CIDv0 allowed)
/// 
/// Always use `Cid::parse()` instead of creating directly from strings,
/// especially with network input.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cid(String);

impl Cid {
    /// Parse and validate a CID string
    /// 
    /// Returns an error if the CID doesn't meet AT Protocol requirements:
    /// - Must be 8-256 characters long
    /// - Must match CID character pattern
    /// - Must not be CIDv0 (starting with "Qmb")
    pub fn parse(raw: &str) -> Result<Self, CidError> {
        if raw.is_empty() {
            return Err(CidError::Empty);
        }
        
        if raw.len() > 256 {
            return Err(CidError::TooLong);
        }
        
        if raw.len() < 8 {
            return Err(CidError::TooShort);
        }
        
        // Validate CID character pattern (matches Go regex: ^[a-zA-Z0-9+=]{8,256}$)
        if !is_valid_cid_pattern(raw) {
            return Err(CidError::InvalidFormat);
        }
        
        // Reject CIDv0 format (security requirement from Go implementation)
        if raw.starts_with("Qmb") {
            return Err(CidError::CidV0NotAllowed);
        }
        
        Ok(Cid(raw.to_string()))
    }
    
    /// Get the CID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
    
    /// Convert to owned string
    pub fn into_string(self) -> String {
        self.0
    }
}

/// CID validation errors matching Go atproto/syntax behavior
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CidError {
    Empty,
    TooShort,
    TooLong,
    InvalidFormat,
    CidV0NotAllowed,
}

impl Display for CidError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CidError::Empty => write!(f, "expected CID, got empty string"),
            CidError::TooShort => write!(f, "CID is too short (8 chars min)"),
            CidError::TooLong => write!(f, "CID is too long (256 chars max)"),
            CidError::InvalidFormat => write!(f, "CID syntax didn't validate via regex"),
            CidError::CidV0NotAllowed => write!(f, "CIDv0 not allowed in this version of atproto"),
        }
    }
}

impl std::error::Error for CidError {}

impl Display for Cid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for Cid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Cid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Cid::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Validate CID character pattern (equivalent to Go regex: ^[a-zA-Z0-9+=]{8,256}$)
fn is_valid_cid_pattern(s: &str) -> bool {
    // Use lazy_static or once_cell in production for regex compilation
    let regex = Regex::new(r"^[a-zA-Z0-9+=]{8,256}$").unwrap();
    regex.is_match(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_cid() {
        let valid_cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        assert!(Cid::parse(valid_cid).is_ok());
    }

    #[test]
    fn test_empty_cid() {
        assert_eq!(Cid::parse(""), Err(CidError::Empty));
    }

    #[test]
    fn test_too_short_cid() {
        assert_eq!(Cid::parse("abc"), Err(CidError::TooShort));
    }

    #[test]
    fn test_too_long_cid() {
        let long_cid = "a".repeat(257);
        assert_eq!(Cid::parse(&long_cid), Err(CidError::TooLong));
    }

    #[test]
    fn test_invalid_characters() {
        assert_eq!(Cid::parse("invalid@cid#here!"), Err(CidError::InvalidFormat));
    }

    #[test]
    fn test_cidv0_rejected() {
        assert_eq!(Cid::parse("QmbWqxBEKC3P8tqsKc98xmWNzrzDtRLMiMPL8wBuTGsMnR"), Err(CidError::CidV0NotAllowed));
    }

    #[test]
    fn test_serde_roundtrip() {
        let original = Cid::parse("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Cid = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }
}