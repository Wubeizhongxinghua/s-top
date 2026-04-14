# Security Policy

## Supported versions

Security fixes, when available, are most likely to land on:

- the latest tagged release
- the current `main` branch

Older releases may not receive backports.

## Reporting a vulnerability

Please do **not** publish security-sensitive details in a public issue.

Preferred process:

1. Use GitHub private vulnerability reporting if it is enabled for the repository
2. If that is not available, contact the maintainer through GitHub first and share only minimal, non-public details
3. Include reproduction details, affected versions, and impact assessment when possible

## Scope

Relevant reports include:

- command execution safety
- unsafe handling of external command output
- packaging or release artifacts that introduce supply-chain risk
- vulnerabilities that can expose credentials, private data, or unintended command execution

General usage or compatibility issues should go through normal support channels.
