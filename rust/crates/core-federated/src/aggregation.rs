//! Federated learning aggregation.

use serde::{Deserialize, Serialize};

/// A gradient update from a participant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientUpdate {
    pub participant_id: String,
    pub round_id: u64,
    pub gradients: Vec<f32>,
    pub samples: u64,
}

/// A federated learning round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedRound {
    pub round_id: u64,
    pub participants: Vec<String>,
    pub aggregated_gradients: Vec<f32>,
    pub status: RoundStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RoundStatus {
    Collecting,
    Aggregating,
    Completed,
    Failed,
}

/// Model aggregator — aggregates gradient updates.
pub struct ModelAggregator {
    current_round: u64,
    updates: Vec<GradientUpdate>,
}

impl ModelAggregator {
    pub fn new() -> Self {
        Self {
            current_round: 0,
            updates: Vec::new(),
        }
    }

    /// Submit a gradient update.
    pub fn submit_update(&mut self, update: GradientUpdate) {
        self.updates.push(update);
    }

    /// Aggregate using weighted averaging.
    pub fn aggregate(&mut self) -> FederatedRound {
        self.current_round += 1;

        let total_samples: u64 = self.updates.iter().map(|u| u.samples).sum();
        let dim = self.updates.first().map(|u| u.gradients.len()).unwrap_or(0);

        let mut aggregated = vec![0.0f32; dim];
        for update in &self.updates {
            let weight = update.samples as f32 / total_samples as f32;
            for (i, g) in update.gradients.iter().enumerate() {
                aggregated[i] += g * weight;
            }
        }

        let participants: Vec<String> = self
            .updates
            .iter()
            .map(|u| u.participant_id.clone())
            .collect();
        let round = FederatedRound {
            round_id: self.current_round,
            participants,
            aggregated_gradients: aggregated,
            status: RoundStatus::Completed,
        };

        self.updates.clear();
        round
    }

    pub fn current_round(&self) -> u64 {
        self.current_round
    }
    pub fn pending_updates(&self) -> usize {
        self.updates.len()
    }
}

impl Default for ModelAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_aggregation() {
        let mut agg = ModelAggregator::new();
        agg.submit_update(GradientUpdate {
            participant_id: "p1".into(),
            round_id: 1,
            gradients: vec![1.0, 2.0],
            samples: 100,
        });
        agg.submit_update(GradientUpdate {
            participant_id: "p2".into(),
            round_id: 1,
            gradients: vec![3.0, 4.0],
            samples: 100,
        });

        let round = agg.aggregate();
        assert_eq!(round.round_id, 1);
        assert_eq!(round.aggregated_gradients, vec![2.0, 3.0]); // equal weight average
        assert_eq!(round.participants.len(), 2);
    }

    #[test]
    fn weighted_by_samples() {
        let mut agg = ModelAggregator::new();
        agg.submit_update(GradientUpdate {
            participant_id: "p1".into(),
            round_id: 1,
            gradients: vec![1.0],
            samples: 300,
        });
        agg.submit_update(GradientUpdate {
            participant_id: "p2".into(),
            round_id: 1,
            gradients: vec![3.0],
            samples: 100,
        });

        let round = agg.aggregate();
        // p1 has 3/4 weight: 1.0 * 0.75 + 3.0 * 0.25 = 1.5
        assert!((round.aggregated_gradients[0] - 1.5).abs() < 0.01);
    }
}
