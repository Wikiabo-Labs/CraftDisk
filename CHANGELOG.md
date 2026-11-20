# Changelog

All notable changes to DiskCraft are listed here. Each version has its own release, with the
Windows program attached, on the [Releases page](https://github.com/Wikiabo-Labs/CraftDisk/releases).

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `CHANGELOG.md` and `CONTRIBUTING.md`.
- **Disk health** page, right under Home: an overall verdict per disk (Good / Needs attention / Failing), Windows' health status, temperature, SSD wear and life left, power-on hours, read and write errors, firmware and serial number. Values are read automatically when DiskCraft starts, together with the disk scan, and again on Refresh.

### Changed
- Releases are published automatically from `main` only. Each release carries the patch notes of its version from this changelog, and every past version stays available on the Releases page.
- Merging copies the files to the root of the first partition. A file whose name is already used there gets the date and time added to its name: `test.txt` becomes `test-11-23-25-20-11-2026.txt` (`HH-mm-ss-dd-MM-yyyy`).
- The second partition is deleted only after every one of its files is found on the first partition with the same size; otherwise the merge stops and nothing is deleted.
- DiskCraft asks for administrator rights once, when it opens, instead of before every operation. The helper that runs the queue inherits them.

### Fixed
- Merging two partitions no longer seems to lose the files of the second one. They used to be copied into a "Contents of X" folder that robocopy marked hidden and system, so File Explorer did not show it.

## [0.1.0] - 2026-11-19

### Added
- First public version: partition manager (resize, move, merge, create, format, delete, wipe,
  check), disk and partition cloning, Windows migration, MBR/GPT conversion, NTFS to FAT32,
  dynamic to basic disk, WinPE boot media.
- English and French interface, with a Settings page.

### Security
- The elevated helper only runs a plan whose SHA-256 fingerprint matches the one approved at
  elevation, and checks the identity of every disk before writing.
