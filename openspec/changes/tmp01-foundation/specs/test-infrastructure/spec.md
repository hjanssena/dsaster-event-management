# Test infrastructure specification delta

## Purpose

Make the test suite run against a real PostgreSQL with the project's migrations, with each test isolated from the others, so the guarantees that live in SQL are tested as they run in production (ADR-011).

## ADDED Requirements

### Requirement: Tests run against migrated PostgreSQL
Tests that need a database SHALL run against a real PostgreSQL server named by `TEST_DATABASE_URL`, with every migration applied. A test run without `TEST_DATABASE_URL` SHALL fail with a message saying how to start the database, not skip silently.

#### Scenario: Migrated schema
- **WHEN** a test asks for a database
- **THEN** it gets a database where every migration has been applied

#### Scenario: Server not configured
- **WHEN** tests run without `TEST_DATABASE_URL`
- **THEN** the tests that need a database fail with a message naming `TEST_DATABASE_URL` and `docker compose up -d event-db`

### Requirement: Each test has its own database
Each test SHALL get a database of its own, so tests running at the same time cannot see each other's rows. The database SHALL be removed when the test ends, whether it passes or fails.

#### Scenario: Parallel tests
- **WHEN** two tests run at the same time and each writes a row
- **THEN** neither test sees the other's row

#### Scenario: Cleanup
- **WHEN** a test that asked for a database ends, passing or failing
- **THEN** its database no longer exists on the server
