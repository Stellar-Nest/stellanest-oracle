use serde::Serialize;
use tracing::debug;

use crate::sources::DataPoint;

/// Aggregated price result for a city.
#[derive(Debug, Clone, Serialize)]
pub struct AggregatedResult {
    pub city_code: String,
    pub price: f64,
    pub confidence: f64,
    pub source_count: usize,
    pub is_stale: bool,
}

/// Aggregator removes outliers and computes weighted averages.
pub struct Aggregator {
    outlier_stddev: f64,
    pub min_sources: usize,
}

impl Aggregator {
    pub fn new(outlier_stddev: f64, min_sources: usize) -> Self {
        Self { outlier_stddev, min_sources }
    }

    /// Aggregate data points for a city into a single price estimate.
    pub fn aggregate(&self, city: &str, points: &[DataPoint]) -> AggregatedResult {
        if points.is_empty() {
            return AggregatedResult {
                city_code: city.to_string(),
                price: 0.0,
                confidence: 0.0,
                source_count: 0,
                is_stale: true,
            };
        }

        // Remove outliers
        let filtered = self.remove_outliers(points);
        if filtered.is_empty() {
            tracing::warn!("all data points flagged as outliers for city, marking stale");
            return AggregatedResult {
                city_code: city.to_string(),
                price: 0.0,
                confidence: 0.0,
                source_count: points.len(),
                is_stale: true,
            };
        }
        let working = &filtered;

        // Weighted average (weight = confidence)
        let total_weight: f64 = working.iter().map(|p| p.confidence).sum();
        let price = if total_weight > 0.0 {
            working.iter().map(|p| p.value * p.confidence).sum::<f64>() / total_weight
        } else {
            0.0
        };

        let avg_confidence: f64 = working.iter().map(|p| p.confidence).sum::<f64>() / working.len() as f64;

        debug!(
            city,
            price,
            confidence = avg_confidence,
            sources = working.len(),
            raw = points.len(),
            "aggregated"
        );

        AggregatedResult {
            city_code: city.to_string(),
            price,
            confidence: avg_confidence,
            source_count: working.len(),
            is_stale: false,
        }
    }

    /// Remove data points beyond N standard deviations from the mean.
    fn remove_outliers(&self, points: &[DataPoint]) -> Vec<DataPoint> {
        if points.len() <= 2 {
            return points.to_vec();
        }

        let values: Vec<f64> = points.iter().map(|p| p.value).collect();
        let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
        let variance: f64 = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let stddev = variance.sqrt();
        let threshold = stddev * self.outlier_stddev;

        points
            .iter()
            .filter(|p| (p.value - mean).abs() <= threshold)
            .cloned()
            .collect()
    }
}

/// Calculate the confidence-weighted median price.
pub fn weighted_median(points: &[DataPoint]) -> f64 {
    if points.is_empty() {
        return 0.0;
    }

    let mut sorted = points.to_vec();
    sorted.retain(|dp| !dp.value.is_nan());
    sorted.sort_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal));

    let total_weight: f64 = sorted.iter().map(|p| p.confidence).sum();
    let mut cumulative = 0.0;
    let half = total_weight / 2.0;

    for p in &sorted {
        cumulative += p.confidence;
        if cumulative >= half {
            return p.value;
        }
    }

    sorted.last().map(|p| p.value).unwrap_or(0.0)
}
