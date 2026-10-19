# Changelog

## 0.1.2
- Fix: Windows MSI asset paths now resolve correctly from the repository root.
- Packaging: release builds derive the MSI version from the tag, validate the MSI, publish SHA-256 checksums, and safely replace assets on retry.
- Metadata: repository URLs and package metadata now point to `pactender/questlog`.

## 0.1.1
- Fix: `--min-score abc` silently passed the gate; invalid and unknown options now exit with an error.
- Fix: files like `latest.md` or `inspect.rs` were counted as tests; detection now uses real test paths and markers.
- Fix: CI "runs tests" matched any workflow containing the word "test"; it now looks for real test commands.
- Fix: symlinks were followed (cycles could hang the walker, links could read outside the folder); they are now skipped.
- Fix: huge READMEs counted as missing; history now scoped to the audited folder; failed SVG writes no longer panic.
- Fix: `NO_COLOR`, `TERM=dumb`, and narrow terminals now fall back to plain output.
- Fix: git "dubious ownership" is reported instead of looking like a non-git folder.
- Security findings now name the file. Unit and integration tests expanded from 3 to 8.
- Privacy: documented in SECURITY.md; CI uses least-privilege permissions and no third-party release action.

## 0.1.0
- Initial release: 27 checks across 6 categories, terminal sheet, SVG card, CI gate.
