# Container release specification delta

## Purpose

Define version-tag publication of the service container and the GitHub Release that distributes its API contract.

## ADDED Requirements

### Requirement: Publish on version tag pushes
The release workflow SHALL publish the image built from the pushed version tag matching `v*.*.*`. Branch pushes and pull requests SHALL NOT initiate image publishing. Tags created manually and through `bump.sh` SHALL use the same publishing path.

#### Scenario: Version tag pushed
- **WHEN** `v1.2.3` is pushed
- **THEN** the workflow publishes the image built from that tag's commit

#### Scenario: Branch or pull request event
- **WHEN** a branch is pushed or a pull request is opened
- **THEN** the release workflow does not publish an image

### Requirement: Semantic image tags
Images SHALL use `gateway.tail9a6ddb.ts.net:5000/aura-event-management`. Stable versions SHALL publish full version, minor, major, and `latest` aliases, excluding the major-only `0` alias. Prerelease versions SHALL publish only their full version without the leading `v`.

#### Scenario: Stable version
- **WHEN** `v1.2.3` is released
- **THEN** the image tags are `1.2.3`, `1.2`, `1`, and `latest`

#### Scenario: Pre-1.0 stable version
- **WHEN** `v0.2.3` is released
- **THEN** the image tags are `0.2.3`, `0.2`, and `latest`, without `0`

#### Scenario: Prerelease version
- **WHEN** `v1.2.3-alpha` or `v1.2.3-beta` is released
- **THEN** only `1.2.3-alpha` or `1.2.3-beta`, respectively, is published
- **AND** stable aliases remain unchanged

### Requirement: Restrict registry access to publication
The image SHALL be built before the runner joins the tailnet. Publication SHALL authenticate through the existing OIDC and registry secrets. Docker build context SHALL exclude local credentials and build output.

#### Scenario: Build before connecting
- **WHEN** a version-tag release runs
- **THEN** the image build completes before tailnet connection and registry login
- **AND** `.env` files and local build output are excluded from the context

### Requirement: GitHub Release follows image publication
The GitHub Release SHALL be created only after successful image publication and SHALL retain the generated `openapi.yml` attachment. Prerelease tags SHALL create GitHub prereleases.

#### Scenario: Successful publication
- **WHEN** all image tags are pushed successfully
- **THEN** the dependent job generates and attaches `openapi.yml` to the GitHub Release
- **AND** alpha/beta versions are marked as prereleases

#### Scenario: Publication failure
- **WHEN** building, tailnet access, registry login, or pushing fails
- **THEN** the workflow fails and the GitHub Release job is skipped
