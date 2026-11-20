# Changelog

All notable changes to DiskCraft are listed here. Each version has its own release, with the
Windows program attached, on the [Releases page](https://github.com/Wikiabo-Labs/CraftDisk/releases).

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `CHANGELOG.md` and `CONTRIBUTING.md`.

### Changed
- Releases are published automatically from `main` only. Each release carries the patch notes of its version from this changelog, and every past version stays available on the Releases page.

## [0.1.0] - 2026-11-19

### Added
- First public version: partition manager (resize, move, merge, create, format, delete, wipe,
  check), disk and partition cloning, Windows migration, MBR/GPT conversion, NTFS to FAT32,
  dynamic to basic disk, WinPE boot media.
- English and French interface, with a Settings page.

### Security
- The elevated helper only runs a plan whose SHA-256 fingerprint matches the one approved at
  elevation, and checks the identity of every disk before writing.
