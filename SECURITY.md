# Security and privacy

## What questlog does with your data

- **Reads:** files tracked by git in the folder you point it at (up to 200 KB each, 64 MB total), plus commit timestamps and subject lines from `git log`. Commit author names and emails are never read.
- **Writes:** nothing, unless you pass `--svg <file>`, which writes one SVG card to the path you chose.
- **Network:** none. The tool makes no network requests, has no telemetry, and has no accounts, servers, or sharing features. Nothing you scan is sent to anyone, and there is no way for one user to see another user's data.
- **Output:** a score, category bars, and fix messages in your terminal. The SVG card contains only the folder name (control characters removed, trimmed to 24 characters) and the scores. When a secret pattern is found, the message names the file, never the secret.
- **Symlinks** are never followed, so a link cannot pull files from outside the scanned folder.

Check before you share a card: the folder name is public once the card is. Rename the folder or edit the SVG if that matters.

## Reporting a vulnerability

Use the repository's **Security** tab, then **Report a vulnerability** (private reporting). Please do not open a public issue for security problems.
