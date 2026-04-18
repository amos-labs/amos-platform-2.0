//! Background task that refreshes OAuth2 access tokens approaching expiry.
//!
//! Polls `integration_credentials` every 5 minutes for active OAuth2 rows
//! whose `token_expires_at` is within 10 minutes of now (and which have a
//! `refresh_token`). For each, POSTs to the provider's `oauth_token_url`
//! with `grant_type=refresh_token` and writes the new `access_token` /
//! `token_expires_at` / optional new `refresh_token` back to the row.
//!
//! Without this, users hit expired tokens mid-call and have to reconnect.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

/// Spawn the refresh loop. Call once at startup.
pub fn start(db_pool: PgPool) {
    tokio::spawn(async move {
        let http_client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("oauth_refresh: failed to build HTTP client: {}", e);
                return;
            }
        };

        // 5-minute tick — tokens typically last 1h, so this gives us ~12
        // attempts before expiry.
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            process_due(&db_pool, &http_client).await;
        }
    });
    tracing::info!("OAuth token refresh worker started (5min tick)");
}

async fn process_due(db_pool: &PgPool, http_client: &reqwest::Client) {
    // Candidates: active oauth2 credentials whose access token will expire
    // within 10 minutes, and which have a refresh_token we can use.
    let rows = match sqlx::query_as::<_, DueRow>(
        r#"SELECT id, oauth_token_url, oauth_client_id, oauth_client_secret, refresh_token
             FROM integration_credentials
            WHERE auth_type = 'oauth2'
              AND status = 'active'
              AND refresh_token IS NOT NULL
              AND oauth_token_url IS NOT NULL
              AND token_expires_at IS NOT NULL
              AND token_expires_at < NOW() + INTERVAL '10 minutes'
            LIMIT 50"#,
    )
    .fetch_all(db_pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("oauth_refresh: query failed: {}", e);
            return;
        }
    };

    if rows.is_empty() {
        return;
    }

    tracing::debug!("oauth_refresh: refreshing {} token(s)", rows.len());

    for row in rows {
        if let Err(e) = refresh_one(db_pool, http_client, &row).await {
            tracing::warn!(
                credential_id = %row.id,
                error = %e,
                "oauth_refresh: refresh failed"
            );
        }
    }
}

async fn refresh_one(
    db_pool: &PgPool,
    http_client: &reqwest::Client,
    row: &DueRow,
) -> Result<(), String> {
    let token_url = row.oauth_token_url.as_deref().ok_or("no token_url")?;
    let client_id = row.oauth_client_id.as_deref().ok_or("no client_id")?;
    let refresh_token = row.refresh_token.as_deref().ok_or("no refresh_token")?;

    let mut form: Vec<(&str, String)> = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token.to_string()),
        ("client_id", client_id.to_string()),
    ];
    if let Some(secret) = row.oauth_client_secret.as_deref() {
        if !secret.is_empty() {
            form.push(("client_secret", secret.to_string()));
        }
    }

    let resp = http_client
        .post(token_url)
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("http: {}", e))?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("body: {}", e))?;

    if !status.is_success() {
        // Mark as expired so the user is forced to re-auth; don't keep retrying.
        let _ = sqlx::query(
            "UPDATE integration_credentials SET status = 'expired', updated_at = NOW() WHERE id = $1",
        )
        .bind(row.id)
        .execute(db_pool)
        .await;
        return Err(format!(
            "provider rejected refresh: HTTP {} — {}",
            status, body
        ));
    }

    let token_json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("invalid JSON: {} ({})", e, body))?;

    let access_token = token_json
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("missing access_token in response")?
        .to_string();
    // Some providers rotate the refresh_token each time; others keep it stable.
    let new_refresh = token_json
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let expires_in = token_json.get("expires_in").and_then(|v| v.as_i64());
    let expires_at: Option<DateTime<Utc>> =
        expires_in.map(|s| Utc::now() + chrono::Duration::seconds(s));

    sqlx::query(
        r#"UPDATE integration_credentials
              SET access_token = $1,
                  refresh_token = COALESCE($2, refresh_token),
                  token_expires_at = $3,
                  last_rotated_at = NOW(),
                  updated_at = NOW()
            WHERE id = $4"#,
    )
    .bind(&access_token)
    .bind(&new_refresh)
    .bind(expires_at)
    .bind(row.id)
    .execute(db_pool)
    .await
    .map_err(|e| format!("DB update: {}", e))?;

    tracing::info!(
        credential_id = %row.id,
        expires_at = ?expires_at,
        "oauth_refresh: token refreshed"
    );
    Ok(())
}

#[derive(sqlx::FromRow)]
struct DueRow {
    id: Uuid,
    oauth_token_url: Option<String>,
    oauth_client_id: Option<String>,
    oauth_client_secret: Option<String>,
    refresh_token: Option<String>,
}
