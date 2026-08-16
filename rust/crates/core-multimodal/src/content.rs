//! Multimodal content types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Content modality.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Modality {
    Text,
    Image,
    Audio,
    Video,
    Document,
}

/// Content metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentMetadata {
    pub mime_type: String,
    pub size_bytes: u64,
    pub dimensions: Option<(u32, u32)>,
    pub duration_secs: Option<f64>,
    pub extra: HashMap<String, serde_json::Value>,
}

/// Multimodal content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultimodalContent {
    pub id: String,
    pub modality: Modality,
    pub data: Vec<u8>,
    pub metadata: ContentMetadata,
    pub embedding: Option<Vec<f64>>,
}

impl MultimodalContent {
    pub fn text(text: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            modality: Modality::Text,
            data: text.as_bytes().to_vec(),
            metadata: ContentMetadata {
                mime_type: "text/plain".into(),
                size_bytes: text.len() as u64,
                dimensions: None,
                duration_secs: None,
                extra: HashMap::new(),
            },
            embedding: None,
        }
    }

    pub fn image(data: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            modality: Modality::Image,
            data,
            metadata: ContentMetadata {
                mime_type: "image/png".into(),
                size_bytes: 0,
                dimensions: Some((width, height)),
                duration_secs: None,
                extra: HashMap::new(),
            },
            embedding: None,
        }
    }

    pub fn audio(data: Vec<u8>, duration_secs: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            modality: Modality::Audio,
            data,
            metadata: ContentMetadata {
                mime_type: "audio/wav".into(),
                size_bytes: 0,
                dimensions: None,
                duration_secs: Some(duration_secs),
                extra: HashMap::new(),
            },
            embedding: None,
        }
    }

    pub fn text_content(&self) -> Option<&str> {
        if self.modality == Modality::Text {
            std::str::from_utf8(&self.data).ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_content() {
        let content = MultimodalContent::text("hello world");
        assert_eq!(content.modality, Modality::Text);
        assert_eq!(content.text_content(), Some("hello world"));
    }

    #[test]
    fn image_content() {
        let content = MultimodalContent::image(vec![0; 100], 10, 10);
        assert_eq!(content.modality, Modality::Image);
        assert_eq!(content.metadata.dimensions, Some((10, 10)));
    }
}
