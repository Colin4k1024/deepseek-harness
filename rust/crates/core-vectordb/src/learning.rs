//! Online learning model — incremental updates from streaming data.

use serde::{Deserialize, Serialize};

/// A model update from a single training example.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUpdate {
    pub feature_id: String,
    pub gradient: f64,
    pub weight: f64,
}

/// Online learning model — updates weights incrementally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineModel {
    pub name: String,
    pub weights: HashMap<String, f64>,
    pub learning_rate: f64,
    pub update_count: u64,
}

use std::collections::HashMap;

impl OnlineModel {
    pub fn new(name: impl Into<String>, learning_rate: f64) -> Self {
        Self {
            name: name.into(),
            weights: HashMap::new(),
            learning_rate,
            update_count: 0,
        }
    }

    /// Apply a gradient update.
    pub fn update(&mut self, update: &ModelUpdate) {
        let weight = self.weights.entry(update.feature_id.clone()).or_insert(0.0);
        *weight -= self.learning_rate * update.gradient;
        self.update_count += 1;
    }

    /// Get weight for a feature.
    pub fn predict(&self, feature_id: &str) -> f64 {
        self.weights.get(feature_id).copied().unwrap_or(0.0)
    }

    /// Number of features.
    pub fn feature_count(&self) -> usize {
        self.weights.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_weights() {
        let mut model = OnlineModel::new("test", 0.1);
        model.update(&ModelUpdate {
            feature_id: "f1".into(),
            gradient: 1.0,
            weight: 1.0,
        });

        assert!((model.predict("f1") + 0.1).abs() < 1e-10); // -0.1 * 1.0
        assert_eq!(model.update_count, 1);
    }

    #[test]
    fn multiple_updates() {
        let mut model = OnlineModel::new("test", 0.01);
        for i in 0..100 {
            model.update(&ModelUpdate {
                feature_id: "f1".into(),
                gradient: 1.0,
                weight: i as f64,
            });
        }
        assert_eq!(model.update_count, 100);
        assert_eq!(model.feature_count(), 1);
    }
}
