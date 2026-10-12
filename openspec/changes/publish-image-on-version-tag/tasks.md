# Tasks

## 1. Version-tag publication

- [x] 1.1 Implement the image publishing job and dependent GitHub Release in `.github/workflows/release.yml`, add `.dockerignore`, update `bump.sh` release descriptions and README documentation, and include scenario validation from `design.md`. Verify workflow syntax, actual metadata output for stable/pre-1.0/alpha/beta fixtures, publication dependency and step order, Docker context exclusions, and `bash -n scripts/bump.sh`; build the Dockerfile if a container runtime is available. Commit as `ci(NA): publish service image on version tags` with the reviewed OpenSpec change included.

## 2. Integration validation

- [x] 2.1 Run `cargo fmt --all -- --check`, `cargo lint`, and `cargo test`; validate the OpenSpec change and record outcomes and any environment limitations in this change folder, including the untested live registry connection. Confirm only intended files changed and mark completed tasks accurately. Commit as `chore(NA): record release publishing validation`. Do not run `bump.sh`, push tags, or invoke the release workflow.
