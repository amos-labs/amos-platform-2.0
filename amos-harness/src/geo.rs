//! IP geolocation service
//!
//! Resolves IP addresses to approximate geographic locations using the ip-api.com
//! free API. Results are cached in-memory to avoid repeated lookups for the same IP.

use dashmap::DashMap;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Cached geolocation result
#[derive(Debug, Clone)]
pub struct GeoLocation {
    /// City name (e.g. "San Francisco")
    pub city: String,
    /// Region / state (e.g. "California")
    pub region: String,
    /// Country name (e.g. "United States")
    pub country: String,
    /// Timezone (e.g. "America/Los_Angeles")
    pub timezone: String,
    /// ISP / organization name (e.g. "Comcast Cable Communications")
    pub org: String,
}

impl GeoLocation {
    /// Format as a human-readable location string
    pub fn display_location(&self) -> String {
        if self.city.is_empty() && self.region.is_empty() {
            self.country.clone()
        } else if self.city.is_empty() {
            format!("{}, {}", self.region, self.country)
        } else {
            format!("{}, {}, {}", self.city, self.region, self.country)
        }
    }
}

/// Response from ip-api.com
#[derive(Debug, Deserialize)]
struct IpApiResponse {
    status: String,
    city: Option<String>,
    #[serde(rename = "regionName")]
    region_name: Option<String>,
    country: Option<String>,
    timezone: Option<String>,
    org: Option<String>,
}

/// Cache entry with TTL
struct CacheEntry {
    location: GeoLocation,
    expires_at: Instant,
}

/// IP geolocation service with in-memory caching
pub struct GeoLocator {
    cache: Arc<DashMap<String, CacheEntry>>,
    http_client: reqwest::Client,
    cache_ttl: Duration,
}

impl GeoLocator {
    /// Create a new GeoLocator with a 1-hour cache TTL
    pub fn new() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap_or_default(),
            cache_ttl: Duration::from_secs(3600), // 1 hour
        }
    }

    /// Look up geolocation for an IP address.
    ///
    /// Returns `None` if the lookup fails (network error, rate limit, private IP, etc.).
    /// Failed lookups are NOT cached so they can be retried.
    pub async fn lookup(&self, ip: &str) -> Option<GeoLocation> {
        // Skip private/loopback IPs
        if is_private_ip(ip) {
            return None;
        }

        // Check cache
        if let Some(entry) = self.cache.get(ip) {
            if entry.expires_at > Instant::now() {
                return Some(entry.location.clone());
            }
            // Expired — drop ref and re-fetch
            drop(entry);
            self.cache.remove(ip);
        }

        // Fetch from ip-api.com (free tier, 45 req/min, no key required)
        let url = format!(
            "http://ip-api.com/json/{}?fields=status,city,regionName,country,timezone,org",
            ip
        );
        let resp = match self.http_client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!("Geo lookup failed for {ip}: {e}");
                return None;
            }
        };

        let api_resp: IpApiResponse = match resp.json().await {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!("Geo lookup parse failed for {ip}: {e}");
                return None;
            }
        };

        if api_resp.status != "success" {
            tracing::debug!("Geo lookup returned non-success for {ip}: {}", api_resp.status);
            return None;
        }

        let location = GeoLocation {
            city: api_resp.city.unwrap_or_default(),
            region: api_resp.region_name.unwrap_or_default(),
            country: api_resp.country.unwrap_or_else(|| "Unknown".to_string()),
            timezone: api_resp.timezone.unwrap_or_default(),
            org: api_resp.org.unwrap_or_default(),
        };

        // Cache the result
        self.cache.insert(
            ip.to_string(),
            CacheEntry {
                location: location.clone(),
                expires_at: Instant::now() + self.cache_ttl,
            },
        );

        Some(location)
    }
}

/// Check if an IP address is private/loopback (not routable on the public internet)
fn is_private_ip(ip: &str) -> bool {
    ip == "127.0.0.1"
        || ip == "::1"
        || ip == "localhost"
        || ip.starts_with("10.")
        || ip.starts_with("172.16.")
        || ip.starts_with("172.17.")
        || ip.starts_with("172.18.")
        || ip.starts_with("172.19.")
        || ip.starts_with("172.2")
        || ip.starts_with("172.30.")
        || ip.starts_with("172.31.")
        || ip.starts_with("192.168.")
        || ip.starts_with("fd")
        || ip.starts_with("fe80:")
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_private_ip() {
        assert!(is_private_ip("127.0.0.1"));
        assert!(is_private_ip("::1"));
        assert!(is_private_ip("localhost"));
        assert!(is_private_ip("10.0.0.1"));
        assert!(is_private_ip("192.168.1.100"));
        assert!(is_private_ip("172.16.0.1"));
        assert!(!is_private_ip("8.8.8.8"));
        assert!(!is_private_ip("104.26.10.78"));
    }

    #[test]
    fn test_geo_location_display() {
        let loc = GeoLocation {
            city: "San Francisco".to_string(),
            region: "California".to_string(),
            country: "United States".to_string(),
            timezone: "America/Los_Angeles".to_string(),
            org: "Comcast".to_string(),
        };
        assert_eq!(loc.display_location(), "San Francisco, California, United States");

        let loc2 = GeoLocation {
            city: String::new(),
            region: "California".to_string(),
            country: "United States".to_string(),
            timezone: String::new(),
            org: String::new(),
        };
        assert_eq!(loc2.display_location(), "California, United States");

        let loc3 = GeoLocation {
            city: String::new(),
            region: String::new(),
            country: "United States".to_string(),
            timezone: String::new(),
            org: String::new(),
        };
        assert_eq!(loc3.display_location(), "United States");
    }

    #[test]
    fn test_geo_locator_new() {
        let locator = GeoLocator::new();
        assert!(locator.cache.is_empty());
    }

    #[tokio::test]
    async fn test_private_ip_returns_none() {
        let locator = GeoLocator::new();
        assert!(locator.lookup("127.0.0.1").await.is_none());
        assert!(locator.lookup("192.168.1.1").await.is_none());
        assert!(locator.lookup("10.0.0.1").await.is_none());
    }
}
