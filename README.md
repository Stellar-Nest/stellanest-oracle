# Stellanest — Oracle Aggregator

Rust service that fetches real estate data from multiple sources, aggregates prices, and submits them to the on-chain Price Oracle contract.

## Pipeline

```
Zillow ─┐
Numbeo ─┼──> Fetch ──> Normalize ──> Outlier Removal ──> Weighted Avg ──> Submit On-Chain
UK LR  ─┘
```

## Setup

```bash
export DATABASE_URL="postgres://stellanest:stellanest@localhost:5432/stellanest"
export ORACLE_SECRET_KEY="<stellar-secret>"
export ZILLOW_API_KEY="<key>"
export NUMBEO_API_KEY="<key>"
export UPDATE_INTERVAL_HOURS=6

cargo run
```
