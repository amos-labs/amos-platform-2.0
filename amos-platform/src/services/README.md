# AMOS Platform Services

This module contains background services that power the AMOS token economy.

## Bounty Service

The `BountyService` is a nightly scheduled background task that handles daily token emission distribution to contributors.

### Features

1. **Nightly Emission Distribution**: Runs once per day at midnight UTC
2. **Point Aggregation**: Collects all contribution activities for the day
3. **Proportional Rewards**: Distributes AMOS tokens based on points earned
4. **Halving Schedule**: Respects the emission halving schedule (yearly)
5. **On-chain Proofs**: Optionally submits bounty proofs to Solana blockchain

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Nightly Scheduler                        │
│  (Runs at midnight UTC, calculates next midnight)          │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│              run_nightly_emission()                         │
│  1. Calculate day index from genesis                        │
│  2. Get daily emission for this day (with halving)          │
│  3. Query database for all contributions today              │
│  4. Build daily points snapshot                             │
│  5. Calculate proportional rewards                          │
│  6. Record rewards in database                              │
│  7. Submit bounty proofs to Solana (optional)               │
└─────────────────────────────────────────────────────────────┘
```

### Database Schema

The service relies on two main tables:

#### `contribution_activities`

Tracks all contribution activities that earn points.

```sql
CREATE TABLE contribution_activities (
    id BIGSERIAL PRIMARY KEY,
    contributor_id BIGINT NOT NULL,
    day_index BIGINT NOT NULL,
    activity_type VARCHAR(50) NOT NULL,
    points BIGINT NOT NULL,
    reference_id TEXT,
    created_at TIMESTAMPTZ NOT NULL
);
```

#### `emission_records`

Records daily emission rewards distributed to contributors.

```sql
CREATE TABLE emission_records (
    id BIGSERIAL PRIMARY KEY,
    contributor_id BIGINT NOT NULL,
    day_index BIGINT NOT NULL,
    tokens_awarded BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    UNIQUE (contributor_id, day_index)
);
```

### Usage

#### Starting the Nightly Scheduler

The bounty service is automatically started in `main.rs`:

```rust
use amos_platform::services::BountyService;

// Initialize platform state
let state = PlatformState::new(config).await?;

// Start the nightly scheduler
let bounty_service = BountyService::new(state.clone());
bounty_service.start_nightly_scheduler();
```

The scheduler runs indefinitely in the background, waking up at midnight UTC each day.

#### Recording Contributions

API routes can record contribution activities using the service:

```rust
use amos_platform::services::BountyService;

let bounty_service = BountyService::new(state);

// Record a bounty completion
bounty_service.record_contribution(
    contributor_id: 42,
    activity_type: "BountyCompletion",
    points: 500,
    reference_id: Some("bounty_123"),
).await?;
```

#### Querying Daily Summaries

Retrieve emission data for a specific day:

```rust
let summary = bounty_service.get_daily_summary(day_index).await?;

println!("Day {}: {} AMOS distributed to {} contributors",
    summary.day_index,
    summary.total_distributed,
    summary.contributor_count
);
```

### Token Economics

The bounty service implements the AMOS token emission schedule:

- **Year 0-1**: 16,000 AMOS/day
- **Year 1-2**: 8,000 AMOS/day (halved)
- **Year 2-3**: 4,000 AMOS/day (halved)
- **Floor**: 100 AMOS/day (minimum)

Points are distributed proportionally:

```
contributor_reward = (contributor_points / total_points) × daily_emission
```

### On-chain Integration

If a Solana client is configured, the service submits bounty proofs on-chain:

1. Computes a SHA-256 evidence hash from the emission record
2. Submits a transaction to the bounty program
3. Logs the transaction signature

The evidence hash is computed as:

```rust
hash = SHA256("AMOS_EMISSION_PROOF:" || day_index || contributor_id || tokens)
```

### Error Handling

The service is designed to be resilient:

- **Missing tables**: Gracefully degrades if database tables don't exist
- **Solana failures**: Logs warnings but continues processing other contributors
- **Scheduler failures**: Logs errors but continues to next scheduled run

### Testing

Run the bounty service tests:

```bash
cargo test -p amos-platform services::bounty_service
```

Test coverage includes:

- Day index calculation from genesis timestamp
- Genesis timestamp validation (Jan 1, 2025 UTC)
- Evidence hash determinism
- Evidence hash uniqueness for different inputs

### Configuration

The service uses these configuration values:

- **Genesis Timestamp**: `1735689600` (Jan 1, 2025 00:00:00 UTC)
- **Max Bounty Points**: `2000` (from `economics.rs`)
- **Emission Schedule**: Defined in `emission.rs`

### Monitoring

The service emits structured logs for observability:

- `INFO`: Normal operations (scheduler start, emission runs, rewards)
- `WARN`: Graceful degradation (missing tables, Solana failures)
- `ERROR`: Critical failures (database errors, calculation errors)

Example log output:

```json
{
  "timestamp": "2025-01-02T00:00:01Z",
  "level": "INFO",
  "message": "Running emission for day index 1",
  "target": "amos_platform::services::bounty_service"
}
```

### Future Enhancements

Potential improvements for the bounty service:

1. **Batch Solana Submissions**: Submit multiple proofs in a single transaction
2. **Redis Caching**: Cache daily summaries for faster API responses
3. **Retry Logic**: Implement exponential backoff for failed Solana submissions
4. **Metrics**: Export Prometheus metrics for emission distributions
5. **Webhooks**: Notify contributors when they receive emissions
6. **Historical Data**: Archive old emission records to separate table
