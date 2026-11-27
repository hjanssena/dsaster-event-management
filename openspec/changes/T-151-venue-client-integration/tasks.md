# Tasks

## 1. Specification & Implementation

- [x] 1.1 Document OpenSpec delta for `T-151` `HttpVenueClient` alignment.
- [x] 1.2 Add `VENUE_SERVICE_TOKEN` to `src/config.rs` and `.env.example`, wire it in `src/main.rs`, and update `HttpVenueClient` in `src/service/venue_client.rs` to query `/venues/{id}` with proper status differentiation.
- [x] 1.3 Add `tests/venue_client_test.rs` and run `cargo fmt --all -- --check`, `cargo lint`, and `cargo test`.
