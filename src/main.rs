use sqlx::postgres::PgPoolOptions;
use tokio::time::{sleep, Duration};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod aggregator;
mod sources;
mod submitter;

/// Launch cities tracked by Stellanest.
const LAUNCH_CITIES: &[&str] = &[
    "NYC", "LON", "LAG", "TOK", "DUB",
    "MUM", "NAI", "SAO", "BER", "SYD",
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "stellanest_oracle=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://stellanest:stellanest@localhost:5432/stellanest".into());
    let zillow_key = std::env::var("ZILLOW_API_KEY").unwrap_or_default();
    let numbeo_key = std::env::var("NUMBEO_API_KEY").unwrap_or_default();
    let oracle_secret = std::env::var("ORACLE_SECRET_KEY").unwrap_or_default();

    let update_hours: u64 = std::env::var("UPDATE_INTERVAL_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    tracing::info!("connected to PostgreSQL");

    let all_sources = sources::all_sources(&zillow_key, &numbeo_key);
    let agg = aggregator::Aggregator::new(3.0, 3);
    let sub = submitter::Submitter::new(db.clone(), &oracle_secret);

    // Run immediately, then on interval with graceful shutdown
    tracing::info!("starting initial oracle cycle");
    run_cycle(&all_sources, &agg, &sub, LAUNCH_CITIES).await;
    tracing::info!("initial oracle cycle complete");

    tokio::select! {
        _ = async {
            loop {
                sleep(Duration::from_secs(update_hours * 3600)).await;
                tracing::info!("starting scheduled oracle cycle");
                run_cycle(&all_sources, &agg, &sub, LAUNCH_CITIES).await;
                tracing::info!("scheduled oracle cycle complete");
            }
        } => {}
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("received shutdown signal, exiting gracefully");
        }
    }
}

/// One full fetch → aggregate → submit cycle.
async fn run_cycle(
    sources: &[Box<dyn sources::Source + Send + Sync>],
    agg: &aggregator::Aggregator,
    sub: &submitter::Submitter,
    cities: &[&str],
) {
    tracing::info!("starting oracle update cycle");

    // Collect data points per city
    let mut city_points: std::collections::HashMap<String, Vec<sources::DataPoint>> =
        std::collections::HashMap::new();

    for src in sources {
        match src.fetch(cities).await {
            Ok(points) => {
                for p in points {
                    city_points.entry(p.city_code.clone()).or_default().push(p);
                }
            }
            Err(e) => {
                tracing::error!(source = src.name(), error = %e, "fetch failed");
            }
        }
    }

    // Aggregate and submit
    let mut results = Vec::new();
    for &city in cities {
        let points = city_points.get(city).cloned().unwrap_or_default();
        let result = agg.aggregate(city, &points);

        if !result.is_stale && result.source_count >= 3 {
            results.push(result);
        } else {
            tracing::warn!(city, sources = result.source_count, stale = result.is_stale, "insufficient data");
        }
    }

    if !results.is_empty() {
        if let Err(e) = sub.submit_batch(&results).await {
            tracing::error!("batch submit failed: {}", e);
        }
    }

    tracing::info!(cities = results.len(), "oracle update cycle complete");
}
