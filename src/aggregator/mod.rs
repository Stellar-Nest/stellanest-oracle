use serde::Serialize;
use tracing::debug;

use crate::sources::DataPoint;

/// Data older than this is considered stale (matches UPDATE_INTERVAL_HOURS default).
const STALENESS_THRESHOLD_SECS: u64 = 6 * 3600; // 6 hours

/// Aggregated price result for a city.
#[derive(Debug, Clone, Serialize)]
pub struct AggregatedResult {
    pub city_code: String,
    pub price: f64,
    pub confidence: f64,
    pub source_count: usize,
    pub is_stale: bool,
    pub timestamp: u64,
}

/// Aggregator removes outliers and computes weighted averages.
pub struct Aggregator {
    outlier_stddev: f64,
    min_sources: usize,
}

impl Aggregator {
    pub fn new(outlier_stddev: f64, min_sources: usize) -> Self {
        Self { outlier_stddev, min_sources }
    }

    /// Aggregate data points for a city into a single price estimate.
    pub fn aggregate(&self, city: &str, points: &[DataPoint]) -> AggregatedResult {
        let now = chrono::Utc::now().timestamp() as u64;

        if points.is_empty() {
            return AggregatedResult {
                city_code: city.to_string(),
                price: 0.0,
                confidence: 0.0,
                source_count: 0,
                is_stale: true,
                timestamp: now,
            };
        }

        // Filter out stale data points
        let fresh_points: Vec<&DataPoint> = points
            .iter()
            .filter(|p| {
                let age = now.saturating_sub(p.timestamp as u64);
                age < STALENESS_THRESHOLD_SECS
            })
            .collect();

        if fresh_points.is_empty() {
            debug!(
                city,
                total = points.len(),
                "all data points are stale (older than {}s)", STALENESS_THRESHOLD_SECS
            );
            return AggregatedResult {
                city_code: city.to_string(),
                price: 0.0,
                confidence: 0.0,
                source_count: 0,
                is_stale: true,
                timestamp: now,
            };
        }

        // Calculate freshness-adjusted confidence for each point.
        // Points right at the threshold get near-zero weight; fresh points keep full confidence.
        let adjusted: Vec<DataPoint> = fresh_points
            .iter()
            .map(|p| {
                let age = now.saturating_sub(p.timestamp as u64) as f64;
                let threshold = STALENESS_THRESHOLD_SECS as f64;
                // Linear decay: 1.0 when age=0, approaching 0.0 as age approaches threshold
                let freshness_factor = 1.0 - (age / threshold);
                DataPoint {
                    city_code: p.city_code.clone(),
                    value: p.value,
                    source: p.source.clone(),
                    confidence: p.confidence * freshness_factor,
                    timestamp: p.timestamp,
                }
            })
            .collect();

        // Remove outliers from the freshness-adjusted set
        let filtered = self.remove_outliers(&adjusted);
        let working = if filtered.is_empty() { &adjusted } else { &filtered };

        // Weighted average (weight = freshness-adjusted confidence)
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
            fresh_sources = fresh_points.len(),
            total_sources = points.len(),
            "aggregated"
        );

        AggregatedResult {
            city_code: city.to_string(),
            price,
            confidence: avg_confidence,
            source_count: working.len(),
            is_stale: false,
            timestamp: now,
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
    sorted.sort_by(|a, b| a.value.partial_cmp(&b.value).unwrap());

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
