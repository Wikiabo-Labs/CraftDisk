# Contributing

## Branches and versions
- `main` always holds the latest released version. Nobody pushes to it directly: changes arrive
  through pull requests.
- One branch per change, named after its kind: `fix/...`, `feat/...`, `ci/...`, `docs/...`.
- Every pull request bumps the version in `Cargo.toml` (patch number for fixes and small features:
  0.1.1 → 0.1.2) and adds a section for it at the top of `CHANGELOG.md`. CI checks both.
- Run `cargo build` after changing the version so `Cargo.lock` follows.

## Releases
When a pull request is merged into `main`, the Release workflow builds `DiskCraft.exe` and
publishes a GitHub release `vX.Y.Z` with the patch notes of that version. Branches other than
`main` never produce a release. All past releases are kept.

## Language
Everything on GitHub (code comments, commits, pull requests, issues, changelog) is written in English.
