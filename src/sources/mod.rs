use async_trait::async_trait;
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
        // TODO: Call Zillow ZHVI API
        // 1. GET https://www.zillowapi.com/GetZestimate.htm?zpid=...
        // 2. Normalize to USD per sq ft
        // 3. Return DataPoints
        Ok(vec![])
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
        // TODO: Call Numbeo API
        // GET https://www.numbeo.com/api/city_prices?api_key=...&city=...
        Ok(vec![])
    }
}

/// UK Land Registry — London.
pub struct UKLandRegistrySource;

#[async_trait]
impl Source for UKLandRegistrySource {
    fn name(&self) -> &str { "uk_land_registry" }
    fn cities(&self) -> Vec<&str> { vec!["LON"] }

    async fn fetch(&self, cities: &[&str]) -> anyhow::Result<Vec<DataPoint>> {
        // TODO: Fetch from UK Land Registry open data
        Ok(vec![])
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
