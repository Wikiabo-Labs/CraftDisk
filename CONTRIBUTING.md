# Contributing

## Branches
- `main` holds the released versions only. It receives nothing but pull requests from `dev`,
  and every merge into `main` publishes a release.
- `dev` gathers the changes of the next version.
- One branch per change, created from `dev` and merged back into `dev` through a pull request,
  named after its kind: `fix/...`, `feat/...`, `ci/...`, `docs/...`.

## Changelog
- A change branch does **not** touch the version. It adds one or more lines describing the
  change under `## [Unreleased]` at the top of `CHANGELOG.md` (sections Added / Changed / Fixed /
  Security). CI checks it.

## Releases
1. On a `release/vX.Y.Z` branch from `dev`: set the version in `Cargo.toml`, run `cargo build`
   so `Cargo.lock` follows, and rename `## [Unreleased]` to `## [X.Y.Z] - YYYY-MM-DD`.
   Merge it into `dev`.
2. Open a pull request from `dev` to `main`. CI checks the version bump and the changelog.
3. When it is merged, the Release workflow builds `DiskCraft.exe` and publishes the GitHub
   release `vX.Y.Z` with the notes of that version. All past releases are kept.

## Language
Everything on GitHub (code comments, commits, pull requests, issues, changelog) is written in English.
