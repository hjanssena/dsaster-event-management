# Design

## Context

See `proposal.md` for motivation. The existing release workflow listens for `v*.*.*`, generates OpenAPI, and creates a GitHub Release. `bump.sh` already checks the main branch, clean worktree, remote synchronization, CI, and local tests before pushing a version tag. The registry guide supplies pinned Docker and Tailscale actions. Repository secrets are already configured.

## Goals / Non-Goals

**Goals:** Keep tagging compatible, publish the tagged source through the private registry, and delay the GitHub Release until image publication succeeds.

**Non-Goals:** Change application behavior, provision secrets or tailnet accounts, enforce use of `bump.sh`, publish a release during implementation, or support multiple container architectures.

## Decisions

- Retain `push.tags: ["v*.*.*"]`. Dispatch from `bump.sh` was considered and rejected by the user in favor of tag pushes. Update only the script's release descriptions.
- Add a separate `publish-image` job with `contents: read` and `id-token: write`; retain `contents: write` only on the dependent GitHub Release job. Use `needs: publish-image` rather than independent publishing so a failed image push cannot create a new GitHub Release.
- Use the registry guide's pinned action revisions. Configure the Docker daemon's insecure registry list by merging existing JSON, correcting the guide's broken empty-content shell check. Select the Buildx `docker` driver so daemon HTTP configuration applies. Build and load locally before joining Tailscale, then authenticate and push all generated tags.
- Derive semantic tags with Docker metadata: full version, minor, major except `v0.*`, and automatic stable-only `latest`. Its prerelease behavior must be verified against alpha/beta fixtures before relying on it. Use `aura-event-management`, based on the README's team name and the guide's team/service naming rule.
- Reuse the existing Dockerfile; add `.dockerignore` for `.git`, `target`, `node_modules`, and `.env` credential files while retaining required Cargo manifests and source. No new Rust dependency is needed.
- Mark GitHub Releases as prereleases when the version tag contains a prerelease suffix. Preserve generated OpenAPI attachment and current release permissions.

## Risks / Trade-offs

- Plain HTTP registry transport → Connection is restricted to the encrypted tailnet; authenticate only after the build.
- Missing tailnet grants or registry credentials → Tailscale connectivity and login steps fail before publication; existing secrets are reused without exposing values.
- Multiple pushes can partially complete or older releases can move mutable aliases → Docker registry pushes are not atomic; aliases follow the guide's publication policy, while full version tags identify releases.
- Tag trigger is a glob rather than strict semantic validation → Reject tag events that produce no valid semantic image tags before building or connecting.
- End-to-end publication requires a real version tag → Do not trigger a live release during implementation; record that registry integration remains to be verified on the next authorized release.

## Validation and Scenario Mapping

Each spec scenario maps to a targeted configuration or metadata test, rather than new Rust behavior tests:

| Scenario | Test / validation |
| --- | --- |
| Version tag pushed | Validate trigger and checkout use the triggering tag; build the existing Dockerfile locally if the runtime is available |
| Branch or pull request event | Validate release triggers contain only version-tag pushes |
| Stable version | Run metadata fixture `v1.2.3` and assert exactly four specified tags |
| Pre-1.0 stable version | Run metadata fixture `v0.2.3` and assert three specified tags without `0` |
| Prerelease version | Run alpha and beta metadata fixtures and assert only the full version tag |
| Build before connecting | Validate step ordering, OIDC permissions, secret references, Docker driver, and context exclusions |
| Successful publication | Validate job dependency, OpenAPI attachment, and prerelease expression using stable/alpha/beta fixtures |
| Publication failure | Validate the release job has `needs: publish-image` and no condition allowing it after failure |

Run an available workflow linter, `bash -n scripts/bump.sh`, and the required Rust format/lint/test checks. Tests must exercise actual metadata behavior or the workflow configuration, not a separate reimplementation. Live connectivity and successful registry pushes remain checks for the first authorized tag after merge.

## Migration Plan

Merge through `testing` into `main`; future version-tag pushes run the updated workflow. Roll back the workflow through a normal reviewed change if publishing must be disabled. No application or database migration is needed.
