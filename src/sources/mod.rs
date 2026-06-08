use async_trait::async_trait;
use chrono;
use serde::{Deserialize, Serialize};

/// A single price observation from a data source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub city_code: String,
    pub value: f64,       // Normalized to USD per sq ft
    pub source: String,
    pub confidence: f64,  // 0.0 to 1.0
    pub timestamp: i64,
}

/// Trait for a real estate data provider.
#[async_trait]
pub trait Source: Send + Sync {
    fn name(&self) -> &str;
    fn cities(&self) -> Vec<&str>;
    async fn fetch(&self, cities: &[&str]) -> anyhow::Result<Vec<DataPoint>>;
}

/// Zillow Home Value Index (ZHVI) — US cities.
pub struct ZillowSource {
    api_key: String,
}

impl ZillowSource {
    pub fn new(api_key: &str) -> Self {
        Self { api_key: api_key.to_string() }
    }
}

#[async_trait]
impl Source for ZillowSource {
    fn name(&self) -> &str { "zillow" }

    fn cities(&self) -> Vec<&str> { vec!["NYC", "SFO", "LAX", "CHI", "MIA"] }

    async fn fetch(&self, cities: &[&str]) -> anyhow::Result<Vec<DataPoint>> {
        // TODO: Replace with actual Zillow API integration
        // For now, return mock data for development
        tracing::warn!(source = "zillow", cities = ?cities, "using mock data - implement actual API");
        let mut points = Vec::new();
        for city in cities {
            points.push(DataPoint {
                city_code: city.to_string(),
                value: 450_000.0,
                source: "zillow_mock".to_string(),
                confidence: 0.50,
                timestamp: chrono::Utc::now().timestamp(),
            });
        }
        Ok(points)
    }
}

/// Numbeo Property Index — global coverage.
pub struct NumbeoSource {
    api_key: String,
}

impl NumbeoSource {
    pub fn new(api_key: &str) -> Self {
        Self { api_key: api_key.to_string() }
    }
}

#[async_trait]
impl Source for NumbeoSource {
    fn name(&self) -> &str { "numbeo" }

    fn cities(&self) -> Vec<&str> {
        vec!["LAG", "NAI", "MUM", "TOK", "DUB", "BER", "SAO", "SYD", "LON"]
    }

    async fn fetch(&self, cities: &[&str]) -> anyhow::Result<Vec<DataPoint>> {
        // TODO: Replace with actual Numbeo API integration
        // For now, return mock data for development
        tracing::warn!(source = "numbeo", cities = ?cities, "using mock data - implement actual API");
        let mut points = Vec::new();
        for city in cities {
            points.push(DataPoint {
                city_code: city.to_string(),
                value: 350_000.0,
                source: "numbeo_mock".to_string(),
                confidence: 0.50,
                timestamp: chrono::Utc::now().timestamp(),
            });
        }
        Ok(points)
    }
}

/// UK Land Registry — London.
pub struct UKLandRegistrySource;

#[async_trait]
impl Source for UKLandRegistrySource {
    fn name(&self) -> &str { "uk_land_registry" }
    fn cities(&self) -> Vec<&str> { vec!["LON"] }

    async fn fetch(&self, cities: &[&str]) -> anyhow::Result<Vec<DataPoint>> {
        // UK Land Registry API - fetches recent sales for a given area
        // This is a free, public API
        let client = reqwest::Client::new();
        let mut points = Vec::new();

        for city in cities {
            let url = format!(
                "https://landregistry.data.gov.uk/data/ppi/transaction-record.json?_limit=10&propertyAddress.postcode={}",
                city
            );

            let resp = client.get(&url)
                .timeout(std::time::Duration::from_secs(30))
                .send()
                .await?;

            if !resp.status().is_success() {
                tracing::warn!("UK Land Registry API returned {} for {}", resp.status(), city);
                continue;
            }

            let body: serde_json::Value = resp.json().await?;

            if let Some(results) = body["result"]["items"].as_array() {
                for item in results {
                    if let Some(price) = item["pricePaid"].as_f64() {
                        points.push(DataPoint {
                            city_code: city.to_string(),
                            value: price,
                            source: "uk_land_registry".to_string(),
                            confidence: 0.85, // High confidence for official data
                            timestamp: chrono::Utc::now().timestamp(),
                        });
                    }
                }
            }
        }

        Ok(points)
    }
}

/// Create all configured data sources.
pub fn all_sources(zillow_key: &str, numbeo_key: &str) -> Vec<Box<dyn Source + Send + Sync>> {
    vec![
        Box::new(ZillowSource::new(zillow_key)),
        Box::new(NumbeoSource::new(numbeo_key)),
        Box::new(UKLandRegistrySource),
    ]
}
