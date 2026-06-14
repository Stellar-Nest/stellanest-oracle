use sqlx::PgPool;
use tracing::{info, error};

use crate::aggregator::AggregatedResult;

/// Submitter sends aggregated prices to the on-chain Price Oracle contract.
pub struct Submitter<'a> {
    db: PgPool,
    oracle_secret: &'a str,
}

impl<'a> Submitter<'a> {
    pub fn new(db: PgPool, oracle_secret: &'a str) -> Self {
        Self { db, oracle_secret }
    }

    /// Submit a single aggregated price to the on-chain oracle.
    pub async fn submit(&self, result: &AggregatedResult) -> anyhow::Result<()> {
        info!(
            city = %result.city_code,
            price = result.price,
            confidence = result.confidence,
            sources = result.source_count,
            "submitting price to chain"
        );

        // TODO:
        // 1. Build Soroban invoke transaction:
        //    - Contract: price-oracle
        //    - Function: submit_price(city, price, confidence, timestamp)
        // 2. Sign with oracle keypair (stellar-sdk)
        // 3. Submit to Stellar network
        // 4. Wait for confirmation

        // Store in DB for audit trail
        let _ = sqlx::query(
            "INSERT INTO oracle_submissions (oracle_wallet, city_code, price, confidence, source, accepted, timestamp)
             VALUES ('oracle', $1, $2, $3, 'aggregated', true, NOW())"
        )
        .bind(&result.city_code)
        .bind(result.price)
        .bind((result.confidence * 10000.0) as i32)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Submit multiple city prices.
    pub async fn submit_batch(&self, results: &[AggregatedResult]) -> anyhow::Result<()> {
        info!(count = results.len(), "batch submitting prices");
        let mut errors = Vec::new();

        for result in results {
            if let Err(e) = self.submit(result).await {
                error!(city = %result.city_code, error = %e, "submit failed");
                errors.push(format!("{}: {}", result.city_code, e));
            }
        }

        if !errors.is_empty() {
            anyhow::bail!("{} submissions failed: {}", errors.len(), errors.join(", "));
        }

        Ok(())
    }
}
