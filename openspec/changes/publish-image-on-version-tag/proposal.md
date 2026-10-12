# NA: Publish the service image on version tags

## Why

Version tags currently publish a GitHub Release with the OpenAPI contract but no deployable container. Publish the corresponding service image to the team's tailnet registry using the existing repository secrets.

## What Changes

- Extend the existing version-tag release workflow with a separate image publishing job using OIDC to join Tailscale after building the image.
- Publish `gateway.tail9a6ddb.ts.net:5000/aura-event-management` with semantic version aliases, stable-only `latest`, and no major-only `0` tag.
- Require successful image publishing before creating the GitHub Release; identify prerelease versions as GitHub prereleases.
- Preserve the tag push trigger and existing `bump.sh` checks and tagging behavior; update its release descriptions only.
- Add a Docker context ignore file and document publishing and the already configured secrets.

## Capabilities

### New Capabilities

- `container-release`: Version-tag container publication and its relationship to GitHub Releases.

### Modified Capabilities

None; the main specification directory currently contains no capability specs.

## Impact

Affected files: `.github/workflows/release.yml`, `.dockerignore`, `scripts/bump.sh`, and `README.md`. The existing Dockerfile is reused. No application APIs, Rust crates, migrations, or validation workflow changes are needed. GitHub Actions uses the existing `REGISTRY_USER`, `REGISTRY_PASS`, `TS_OAUTH_CLIENT_ID`, and `TS_OAUTH_AUDIENCE` secrets and previously granted tailnet access. Live registry verification requires a future authorized release tag; implementation must not publish one.
