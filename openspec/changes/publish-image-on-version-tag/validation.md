# Release publishing validation

Validated on 2026-10-11 on `feat_NA_publish_image_on_version_tag`.

## Required checks

- `cargo fmt --all -- --check`: passed.
- `cargo lint`: passed without warnings.
- `cargo test`: passed (17 API tests, 13 service tests).
- `bash -n scripts/bump.sh`: passed; the script was not executed.
- `actionlint` v1.7.7 against `.github/workflows/release.yml`: passed.
- `git diff --check`: passed.
- `openspec validate publish-image-on-version-tag --strict`: passed.
- `docker build -t localhost/dsaster-event-management-validation .`: passed using the local Podman-backed Docker command and cached build layers.

## Scenario checks

Ran the unchanged `dist/index.cjs` from the pinned Docker metadata action revision `dc802804100637a589fabce1cb79ff13a1411302` with the workflow's actual metadata tag configuration. A local HTTP server supplied fixture repository metadata for the action's GitHub API request; tag generation ran through the real action. Fixture output assertions passed:

| Git tag | Actual image tag suffixes |
| --- | --- |
| `v1.2.3` | `1.2.3`, `1.2`, `1`, `latest` |
| `v0.2.3` | `0.2.3`, `0.2`, `latest` |
| `v1.2.3-alpha` | `1.2.3-alpha` |
| `v1.2.3-beta` | `1.2.3-beta` |
| `vnot.a.version` | No tags; the workflow's nonempty tag check rejects publication |

Parsed the actual workflow and asserted tag-only triggers, default checkout of the triggering source, read-only contents and OIDC write permission for publishing, Docker daemon driver selection, local build without push, build-before-tailnet/login/push ordering, existing secret references, the dependent release job without a failure override, OpenAPI attachment, and prerelease detection. Checked `.dockerignore` excludes Git metadata, dependency/build output, and root/nested `.env` files.

## Live release limitation

The configured GitHub secrets, OIDC tailnet grants, registry connectivity, and actual image pushes cannot be verified by local checks. They remain to be verified by the next authorized version-tag release after merge. No release script, version-tag push, registry push, or remote release workflow was invoked during implementation.
