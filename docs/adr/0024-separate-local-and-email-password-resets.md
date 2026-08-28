---
status: deprecated
---

# Deprecated: local password reset policy

This decision is superseded by
[ADR-0036](0036-use-explicit-social-authentication-methods.md). The local
password-reset entry, API, tests, and enablement flag were removed. The product
does not provide password recovery or email delivery; Email and password remains
an ordinary Authentication Method alongside explicit social methods.
