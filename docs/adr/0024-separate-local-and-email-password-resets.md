---
status: accepted
---

# Separate local and email password resets

Local password reset is a development-only recovery tool for malformed local
credential hashes. It requires an explicit enablement flag, reports diagnostic
outcomes, preserves existing sessions, and signs the reset user in. It is not
deployed outside local development and has no separate rate limit. The production
recovery flow will use Better Auth's one-time reset tokens delivered through
Cloudflare Email Sending, which verifies control of the account email before a
password can change and will be rate-limited.
