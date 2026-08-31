//! Generated compatibility metadata for external Oracle presentation.
//!
//! This catalog intentionally contains no Oracle prose. It binds authored card/face identity to
//! display lookup names plus a SHA-256 of the normalized complete external face, allowing clients
//! to reject valid-but-shifted line mappings. It is excluded from [`crate::CardRegistry::content_hash`]
//! because external wording and cache refreshes are presentation-only.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationFaceMetadata {
    pub card_name: String,
    pub face_name: String,
    pub oracle_text_sha256: String,
}
