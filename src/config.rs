//! Service configuration (ADR-006, ADR-008).
//!
//! This is the only module that reads environment variables. Every duration,
//! size and limit has a documented default and must be a positive integer;
//! Event Service refuses to start otherwise, naming every invalid variable.

use std::env;
use std::fmt;
use std::time::Duration;

/// Complete service configuration, grouped by the component that uses it.
#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub server_addr: String,
    /// Mock tokens and dev partners. Removed with the HS256 verifier (ADR-007).
    pub mock_auth: bool,
    pub http: HttpConfig,
    pub db: DbConfig,
    pub pagination: PaginationConfig,
    pub venue: VenueConfig,
    pub retry: RetryConfig,
    pub breaker: BreakerConfig,
    pub publish_job: PublishJobConfig,
    pub outbox: OutboxConfig,
    pub kafka: KafkaConfig,
    pub idempotency: IdempotencyConfig,
}

/// Incoming HTTP requests (ADR-008).
#[derive(Clone, Debug)]
pub struct HttpConfig {
    /// `HTTP_REQUEST_TIMEOUT_MS`: total time for one incoming request.
    pub request_timeout: Duration,
    /// `HTTP_RETRY_AFTER_SECONDS`: `Retry-After` on a 503 when no better value is known.
    pub retry_after_seconds: u64,
}

/// PostgreSQL connections (ADR-008).
#[derive(Clone, Debug)]
pub struct DbConfig {
    /// `DB_POOL_ACQUIRE_TIMEOUT_MS`: wait for a pooled connection.
    pub pool_acquire_timeout: Duration,
    /// `DB_STATEMENT_TIMEOUT_MS`: longest single SQL statement.
    pub statement_timeout: Duration,
}

/// List endpoints (ADR-006).
#[derive(Clone, Copy, Debug)]
pub struct PaginationConfig {
    /// `EVENTS_PAGE_SIZE_DEFAULT`: `per_page` when the request has none.
    pub default_page_size: u64,
    /// `EVENTS_PAGE_SIZE_MAX`: largest `per_page` accepted.
    pub max_page_size: u64,
}

/// Venue Service client (C2, ADR-008).
#[derive(Clone, Debug)]
pub struct VenueConfig {
    /// `VENUE_SERVICE_URL`
    pub base_url: String,
    /// `VENUE_SERVICE_TOKEN`: bearer token sent to Venue Service, if set. Empty means none.
    pub token: Option<String>,
    /// `MOCK_VENUE_SERVICE`
    pub use_mock: bool,
    /// `VENUE_SERVICE_TIMEOUT_MS`: one attempt to Venue Service.
    pub timeout: Duration,
}

/// Retries of outgoing calls (ADR-008).
#[derive(Clone, Debug)]
pub struct RetryConfig {
    /// `RETRY_MAX_ATTEMPTS`: attempts per call, including the first.
    pub max_attempts: u32,
    /// `RETRY_BASE_DELAY_MS`
    pub base_delay: Duration,
    /// `RETRY_MAX_DELAY_MS`
    pub max_delay: Duration,
}

/// Circuit breaker per outgoing dependency (ADR-008).
#[derive(Clone, Debug)]
pub struct BreakerConfig {
    /// `CIRCUIT_BREAKER_FAILURE_THRESHOLD`
    pub failure_threshold: u32,
    /// `CIRCUIT_BREAKER_OPEN_SECONDS`
    pub open_duration: Duration,
}

/// Scheduled publish job (C6, ADR-003).
#[derive(Clone, Debug)]
pub struct PublishJobConfig {
    /// `EVENT_VISIBILITY_LEAD_DAYS`, in days of 24 hours.
    pub visibility_lead_days: u32,
    /// `EVENT_PUBLISH_JOB_INTERVAL_SECONDS`
    pub interval: Duration,
    /// `EVENT_PUBLISH_JOB_BATCH_SIZE`
    pub batch_size: u32,
}

/// Outbox relay (ADR-004, ADR-008, ADR-009).
#[derive(Clone, Debug)]
pub struct OutboxConfig {
    /// `OUTBOX_RELAY_INTERVAL_MS`
    pub relay_interval: Duration,
    /// `OUTBOX_RELAY_BATCH_SIZE`
    pub batch_size: u32,
    /// `OUTBOX_RELAY_MAX_BACKOFF_MS`
    pub max_backoff: Duration,
}

/// Kafka producer (C6, ADR-008).
#[derive(Clone, Debug)]
pub struct KafkaConfig {
    /// `KAFKA_BROKERS`
    pub brokers: String,
    /// `KAFKA_EVENTS_TOPIC`
    pub events_topic: String,
    /// `KAFKA_MESSAGE_TIMEOUT_MS`
    pub message_timeout: Duration,
}

/// `Idempotency-Key` records (ADR-008).
#[derive(Clone, Debug)]
pub struct IdempotencyConfig {
    /// `IDEMPOTENCY_KEY_TTL_HOURS`
    pub key_ttl: Duration,
}

/// Every problem found in the configuration, reported together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub problems: Vec<String>,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "invalid configuration:")?;
        for problem in &self.problems {
            writeln!(f, "  - {problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ConfigError {}

/// Reads variables through a lookup function and records every problem.
struct Reader<F> {
    lookup: F,
    problems: Vec<String>,
}

impl<F: Fn(&str) -> Option<String>> Reader<F> {
    fn required_string(&mut self, name: &str) -> String {
        match (self.lookup)(name) {
            Some(value) if !value.trim().is_empty() => value,
            _ => {
                self.problems.push(format!("{name} is required"));
                String::new()
            }
        }
    }

    fn string(&mut self, name: &str, default: &str) -> String {
        match (self.lookup)(name) {
            None => default.to_string(),
            Some(value) if value.trim().is_empty() => {
                self.problems.push(format!("{name} must not be empty"));
                default.to_string()
            }
            Some(value) => value,
        }
    }

    /// An optional string; unset or blank means `None`.
    fn optional_string(&self, name: &str) -> Option<String> {
        (self.lookup)(name)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn flag(&self, name: &str, default: bool) -> bool {
        (self.lookup)(name)
            .map(|value| value.to_lowercase() != "false" && value != "0")
            .unwrap_or(default)
    }

    /// A positive integer that fits in `u64`.
    fn positive(&mut self, name: &str, default: u64) -> u64 {
        let Some(raw) = (self.lookup)(name) else {
            return default;
        };
        match raw.trim().parse::<u64>() {
            Ok(value) if value > 0 => value,
            _ => {
                self.problems
                    .push(format!("{name} must be a positive integer, got {raw:?}"));
                default
            }
        }
    }

    /// A positive integer that fits in `u32`.
    fn positive_u32(&mut self, name: &str, default: u32) -> u32 {
        let value = self.positive(name, u64::from(default));
        u32::try_from(value).unwrap_or_else(|_| {
            self.problems
                .push(format!("{name} must be at most {}, got {value}", u32::MAX));
            default
        })
    }

    fn millis(&mut self, name: &str, default: u64) -> Duration {
        Duration::from_millis(self.positive(name, default))
    }

    fn seconds(&mut self, name: &str, default: u64) -> Duration {
        Duration::from_secs(self.positive(name, default))
    }

    fn hours(&mut self, name: &str, default: u64) -> Duration {
        let hours = self.positive(name, default);
        Duration::from_secs(hours.saturating_mul(3600))
    }
}

impl AppConfig {
    /// Loads `.env` if present, reads the process environment and exits the
    /// process with every problem listed when the configuration is invalid.
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        match Self::from_lookup(|name| env::var(name).ok()) {
            Ok(config) => config,
            Err(error) => {
                eprintln!("Event Service refuses to start: {error}");
                std::process::exit(1);
            }
        }
    }

    /// Builds the configuration from any source of variables. Tests pass a map.
    pub fn from_lookup<F>(lookup: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let mut r = Reader {
            lookup,
            problems: Vec::new(),
        };

        let config = AppConfig {
            database_url: r.required_string("DATABASE_URL"),
            server_addr: r.string("SERVER_ADDR", "0.0.0.0:3000"),
            mock_auth: r.flag("MOCK_AUTH", true),
            http: HttpConfig {
                request_timeout: r.millis("HTTP_REQUEST_TIMEOUT_MS", 10_000),
                retry_after_seconds: r.positive("HTTP_RETRY_AFTER_SECONDS", 2),
            },
            db: DbConfig {
                pool_acquire_timeout: r.millis("DB_POOL_ACQUIRE_TIMEOUT_MS", 3_000),
                statement_timeout: r.millis("DB_STATEMENT_TIMEOUT_MS", 5_000),
            },
            pagination: PaginationConfig {
                default_page_size: r.positive("EVENTS_PAGE_SIZE_DEFAULT", 10),
                max_page_size: r.positive("EVENTS_PAGE_SIZE_MAX", 100),
            },
            venue: VenueConfig {
                base_url: r.string("VENUE_SERVICE_URL", "http://localhost:3001"),
                token: r.optional_string("VENUE_SERVICE_TOKEN"),
                use_mock: r.flag("MOCK_VENUE_SERVICE", true),
                timeout: r.millis("VENUE_SERVICE_TIMEOUT_MS", 2_000),
            },
            retry: RetryConfig {
                max_attempts: r.positive_u32("RETRY_MAX_ATTEMPTS", 3),
                base_delay: r.millis("RETRY_BASE_DELAY_MS", 100),
                max_delay: r.millis("RETRY_MAX_DELAY_MS", 2_000),
            },
            breaker: BreakerConfig {
                failure_threshold: r.positive_u32("CIRCUIT_BREAKER_FAILURE_THRESHOLD", 5),
                open_duration: r.seconds("CIRCUIT_BREAKER_OPEN_SECONDS", 30),
            },
            publish_job: PublishJobConfig {
                visibility_lead_days: r.positive_u32("EVENT_VISIBILITY_LEAD_DAYS", 10),
                interval: r.seconds("EVENT_PUBLISH_JOB_INTERVAL_SECONDS", 60),
                batch_size: r.positive_u32("EVENT_PUBLISH_JOB_BATCH_SIZE", 100),
            },
            outbox: OutboxConfig {
                relay_interval: r.millis("OUTBOX_RELAY_INTERVAL_MS", 1_000),
                batch_size: r.positive_u32("OUTBOX_RELAY_BATCH_SIZE", 100),
                max_backoff: r.millis("OUTBOX_RELAY_MAX_BACKOFF_MS", 30_000),
            },
            kafka: KafkaConfig {
                brokers: r.string("KAFKA_BROKERS", "localhost:9092"),
                events_topic: r.string("KAFKA_EVENTS_TOPIC", "events.lifecycle"),
                message_timeout: r.millis("KAFKA_MESSAGE_TIMEOUT_MS", 30_000),
            },
            idempotency: IdempotencyConfig {
                key_ttl: r.hours("IDEMPOTENCY_KEY_TTL_HOURS", 24),
            },
        };

        let pagination = config.pagination;
        if pagination.default_page_size > pagination.max_page_size {
            r.problems.push(format!(
                "EVENTS_PAGE_SIZE_DEFAULT ({}) must not be greater than EVENTS_PAGE_SIZE_MAX ({})",
                pagination.default_page_size, pagination.max_page_size
            ));
        }

        if r.problems.is_empty() {
            Ok(config)
        } else {
            Err(ConfigError {
                problems: r.problems,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn load(vars: &[(&str, &str)]) -> Result<AppConfig, ConfigError> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        AppConfig::from_lookup(|name| map.get(name).cloned())
    }

    const DB: (&str, &str) = ("DATABASE_URL", "postgres://u:p@localhost/db");

    fn problems(vars: &[(&str, &str)]) -> Vec<String> {
        load(vars)
            .expect_err("configuration should be invalid")
            .problems
    }

    #[test]
    fn unset_variables_use_documented_defaults() {
        let config = load(&[DB]).unwrap();

        assert_eq!(config.http.request_timeout, Duration::from_millis(10_000));
        assert_eq!(config.http.retry_after_seconds, 2);
        assert_eq!(config.db.pool_acquire_timeout, Duration::from_millis(3_000));
        assert_eq!(config.db.statement_timeout, Duration::from_millis(5_000));
        assert_eq!(config.pagination.default_page_size, 10);
        assert_eq!(config.pagination.max_page_size, 100);
        assert_eq!(config.venue.timeout, Duration::from_millis(2_000));
        assert_eq!(config.venue.base_url, "http://localhost:3001");
        assert!(config.venue.use_mock);
        assert!(config.mock_auth);
        assert_eq!(config.retry.max_attempts, 3);
        assert_eq!(config.retry.base_delay, Duration::from_millis(100));
        assert_eq!(config.retry.max_delay, Duration::from_millis(2_000));
        assert_eq!(config.breaker.failure_threshold, 5);
        assert_eq!(config.breaker.open_duration, Duration::from_secs(30));
        assert_eq!(config.publish_job.visibility_lead_days, 10);
        assert_eq!(config.publish_job.interval, Duration::from_secs(60));
        assert_eq!(config.publish_job.batch_size, 100);
        assert_eq!(config.outbox.relay_interval, Duration::from_millis(1_000));
        assert_eq!(config.outbox.batch_size, 100);
        assert_eq!(config.outbox.max_backoff, Duration::from_millis(30_000));
        assert_eq!(config.kafka.brokers, "localhost:9092");
        assert_eq!(config.kafka.events_topic, "events.lifecycle");
        assert_eq!(config.kafka.message_timeout, Duration::from_millis(30_000));
        assert_eq!(config.idempotency.key_ttl, Duration::from_secs(24 * 3600));
        assert_eq!(config.server_addr, "0.0.0.0:3000");
    }

    #[test]
    fn set_variable_overrides_default() {
        let config = load(&[DB, ("EVENTS_PAGE_SIZE_MAX", "50")]).unwrap();
        assert_eq!(config.pagination.max_page_size, 50);
    }

    #[test]
    fn missing_database_url_is_named() {
        let problems = problems(&[]);
        assert_eq!(problems, vec!["DATABASE_URL is required".to_string()]);
    }

    #[test]
    fn zero_value_is_rejected() {
        let problems = problems(&[DB, ("RETRY_MAX_ATTEMPTS", "0")]);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("RETRY_MAX_ATTEMPTS"), "{problems:?}");
    }

    #[test]
    fn non_numeric_value_is_rejected() {
        let problems = problems(&[DB, ("DB_STATEMENT_TIMEOUT_MS", "5s")]);
        assert_eq!(problems.len(), 1);
        assert!(
            problems[0].contains("DB_STATEMENT_TIMEOUT_MS"),
            "{problems:?}"
        );
    }

    #[test]
    fn every_invalid_value_is_named() {
        let problems = problems(&[
            DB,
            ("OUTBOX_RELAY_BATCH_SIZE", "-1"),
            ("HTTP_RETRY_AFTER_SECONDS", "abc"),
        ]);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(
            problems
                .iter()
                .any(|p| p.contains("OUTBOX_RELAY_BATCH_SIZE"))
        );
        assert!(
            problems
                .iter()
                .any(|p| p.contains("HTTP_RETRY_AFTER_SECONDS"))
        );
    }

    #[test]
    fn value_too_large_for_its_type_is_rejected() {
        let problems = problems(&[DB, ("RETRY_MAX_ATTEMPTS", "4294967296")]);
        assert!(problems[0].contains("RETRY_MAX_ATTEMPTS"), "{problems:?}");
    }

    #[test]
    fn empty_string_variable_is_rejected() {
        let problems = problems(&[DB, ("KAFKA_BROKERS", "")]);
        assert!(problems[0].contains("KAFKA_BROKERS"), "{problems:?}");
    }

    #[test]
    fn default_page_size_above_maximum_is_rejected() {
        let problems = problems(&[
            DB,
            ("EVENTS_PAGE_SIZE_DEFAULT", "200"),
            ("EVENTS_PAGE_SIZE_MAX", "100"),
        ]);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("EVENTS_PAGE_SIZE_DEFAULT"));
        assert!(problems[0].contains("EVENTS_PAGE_SIZE_MAX"));
    }

    #[test]
    fn error_message_lists_every_problem() {
        let error = load(&[("RETRY_MAX_ATTEMPTS", "0")]).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("DATABASE_URL"));
        assert!(message.contains("RETRY_MAX_ATTEMPTS"));
    }

    #[test]
    fn venue_service_token_is_optional() {
        assert_eq!(load(&[DB]).unwrap().venue.token, None);
        assert_eq!(
            load(&[DB, ("VENUE_SERVICE_TOKEN", "  ")])
                .unwrap()
                .venue
                .token,
            None
        );
        assert_eq!(
            load(&[DB, ("VENUE_SERVICE_TOKEN", " abc ")])
                .unwrap()
                .venue
                .token,
            Some("abc".to_string())
        );
    }

    #[test]
    fn flags_keep_current_parsing() {
        let config = load(&[DB, ("MOCK_VENUE_SERVICE", "FALSE"), ("MOCK_AUTH", "0")]).unwrap();
        assert!(!config.venue.use_mock);
        assert!(!config.mock_auth);
    }
}
