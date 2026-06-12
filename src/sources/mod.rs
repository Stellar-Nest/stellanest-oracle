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

// ---------------------------------------------------------------------------
// Zillow Home Value Index (ZHVI) -- US cities (mock for dev)
// ---------------------------------------------------------------------------

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
        tracing::warn!(
            source = "zillow",
            "using MOCK data generator -- set ZILLOW_API_KEY and implement real API for production"
        );

        let now = chrono::Utc::now().timestamp();
        let mut points = Vec::new();

        // Realistic USD/sqft baselines per city (approx mid-2026 market)
        let baselines: std::collections::HashMap<&str, (f64, f64)> = [
            ("NYC", (1_520.0, 0.72)),
            ("SFO", (1_180.0, 0.68)),
            ("LAX", (820.0, 0.70)),
            ("CHI", (390.0, 0.65)),
            ("MIA", (580.0, 0.67)),
        ]
        .iter()
        .cloned()
        .collect();

        for city in cities {
            let (base_price, base_confidence) = baselines
                .get(city)
                .copied()
                .unwrap_or((500.0, 0.50));

            // Add small deterministic jitter so repeated runs vary slightly
            let jitter = ((*city.as_bytes().first().unwrap_or(&0) % 10) as f64 - 5.0) * 8.0;

            points.push(DataPoint {
                city_code: city.to_string(),
                value: base_price + jitter,
                source: "zillow_mock".to_string(),
                confidence: base_confidence,
                timestamp: now,
            });
        }

        Ok(points)
    }
}

// ---------------------------------------------------------------------------
// Numbeo Property Index -- global coverage (mock for dev)
// ---------------------------------------------------------------------------

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
        tracing::warn!(
            source = "numbeo",
            "using MOCK data generator -- set NUMBEO_API_KEY and implement real API for production"
        );

        let now = chrono::Utc::now().timestamp();
        let mut points = Vec::new();

        // Approximate USD/sqft baselines derived from Numbeo cost-of-living indices
        let baselines: std::collections::HashMap<&str, (f64, f64)> = [
            ("LAG", (120.0, 0.45)),
            ("NAI", (135.0, 0.48)),
            ("MUM", (280.0, 0.52)),
            ("TOK", (1_050.0, 0.70)),
            ("DUB", (680.0, 0.65)),
            ("BER", (560.0, 0.68)),
            ("SAO", (210.0, 0.50)),
            ("SYD", (870.0, 0.72)),
            ("LON", (1_420.0, 0.75)),
        ]
        .iter()
        .cloned()
        .collect();

        for city in cities {
            let (base_price, base_confidence) = baselines
                .get(city)
                .copied()
                .unwrap_or((350.0, 0.40));

            // Deterministic jitter
            let jitter = ((*city.as_bytes().first().unwrap_or(&0) % 10) as f64 - 5.0) * 6.0;

            points.push(DataPoint {
                city_code: city.to_string(),
                value: base_price + jitter,
                source: "numbeo_mock".to_string(),
                confidence: base_confidence,
                timestamp: now,
            });
        }

        Ok(points)
    }
}

// ---------------------------------------------------------------------------
// UK Land Registry -- London (real SPARQL API)
// ---------------------------------------------------------------------------

pub struct UKLandRegistrySource;

#[async_trait]
impl Source for UKLandRegistrySource {
    fn name(&self) -> &str { "uk_land_registry" }
    fn cities(&self) -> Vec<&str> { vec!["LON"] }

    async fn fetch(&self, cities: &[&str]) -> anyhow::Result<Vec<DataPoint>> {
        if !cities.contains(&"LON") {
            return Ok(vec![]);
        }

        // SPARQL endpoint for UK Land Registry Price Paid Data
        let endpoint = "https://landregistry.data.gov.uk/landregistry/query";

        let sparql = r#"
            PREFIX lrppi: <http://landregistry.data.gov.uk/def/ppi/>
            PREFIX lrcommon: <http://landregistry.data.gov.uk/def/common/>

            SELECT ?price ?date WHERE {
              ?txn lrppi:pricePaid ?price .
              ?txn lrppi:propertyAddress ?addr .
              ?addr lrcommon:county "GREATER LONDON" .
              ?txn lrppi:transactionDate ?date .
            }
            ORDER BY DESC(?date)
            LIMIT 20
        "#;

        let client = reqwest::Client::new();
        let resp = client
            .post(endpoint)
            .header("Accept", "application/sparql-results+json")
            .form(&[("query", sparql)])
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("UK Land Registry SPARQL returned {}: {}", status, body);
        }

        let json: serde_json::Value = resp.json().await?;
        let now = chrono::Utc::now().timestamp();
        let mut points = Vec::new();

        if let Some(bindings) = json["results"]["bindings"].as_array() {
            for binding in bindings {
                // price is in GBP, whole pounds
                let price_gbp = binding["price"]["value"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);

                if price_gbp <= 0.0 {
                    continue;
                }

                // Convert GBP to USD (approximate) and normalise to per-sqft.
                // Average UK property ~900 sqft, GBP/USD ~1.27
                let gbp_to_usd = 1.27_f64;
                let avg_sqft = 900.0_f64;
                let usd_per_sqft = (price_gbp * gbp_to_usd) / avg_sqft;

                points.push(DataPoint {
                    city_code: "LON".to_string(),
                    value: usd_per_sqft,
                    source: "uk_land_registry".to_string(),
                    confidence: 0.85, // Official government data
                    timestamp: now,
                });
            }
        }

        tracing::info!(
            source = "uk_land_registry",
            points = points.len(),
            "fetched from UK Land Registry SPARQL endpoint"
        );

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
