//! Media processor — image and audio processing.

use serde::{Deserialize, Serialize};

/// Image information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub channels: u32,
}

/// Audio information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub duration_secs: f64,
    pub format: String,
}

/// Trait for media processors.
pub trait MediaProcessor: Send + Sync {
    /// Process image data, returning image info.
    fn process_image(&self, data: &[u8]) -> Result<ImageInfo, String>;

    /// Process audio data, returning audio info.
    fn process_audio(&self, data: &[u8]) -> Result<AudioInfo, String>;

    /// Extract features from media (returns embedding vector).
    fn extract_features(&self, data: &[u8], mime_type: &str) -> Result<Vec<f64>, String>;
}

/// Simple media processor (header-based detection).
pub struct SimpleMediaProcessor;

impl MediaProcessor for SimpleMediaProcessor {
    fn process_image(&self, data: &[u8]) -> Result<ImageInfo, String> {
        if data.len() < 8 {
            return Err("Data too short for image".into());
        }
        // Check PNG magic bytes
        let format = if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
            "png"
        } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            "jpeg"
        } else {
            "unknown"
        };
        Ok(ImageInfo {
            width: 0, // Would need actual parsing
            height: 0,
            format: format.into(),
            channels: 3,
        })
    }

    fn process_audio(&self, data: &[u8]) -> Result<AudioInfo, String> {
        if data.len() < 4 {
            return Err("Data too short for audio".into());
        }
        let format = if data.starts_with(b"RIFF") {
            "wav"
        } else if data.starts_with(b"ID3") || data.starts_with(&[0xFF, 0xFB]) {
            "mp3"
        } else {
            "unknown"
        };
        Ok(AudioInfo {
            sample_rate: 44100,
            channels: 2,
            duration_secs: 0.0,
            format: format.into(),
        })
    }

    fn extract_features(&self, data: &[u8], _mime_type: &str) -> Result<Vec<f64>, String> {
        // Simple hash-based feature extraction
        let mut features = vec![0.0; 64];
        for (i, byte) in data.iter().enumerate().take(1024) {
            features[i % 64] += *byte as f64 / 255.0;
        }
        let norm: f64 = features.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 0.0 {
            features.iter_mut().for_each(|x| *x /= norm);
        }
        Ok(features)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_png() {
        let processor = SimpleMediaProcessor;
        let mut data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        data.extend_from_slice(&[0; 100]);
        let info = processor.process_image(&data).unwrap();
        assert_eq!(info.format, "png");
    }

    #[test]
    fn process_wav() {
        let processor = SimpleMediaProcessor;
        let mut data = b"RIFF".to_vec();
        data.extend_from_slice(&[0; 100]);
        let info = processor.process_audio(&data).unwrap();
        assert_eq!(info.format, "wav");
    }

    #[test]
    fn extract_features() {
        let processor = SimpleMediaProcessor;
        let features = processor
            .extract_features(&[1, 2, 3, 4, 5], "image/png")
            .unwrap();
        assert_eq!(features.len(), 64);
    }
}
