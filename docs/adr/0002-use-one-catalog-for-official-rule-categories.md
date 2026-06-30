---
status: accepted
---

# Use one closed catalog across official rule categories

The Rules Engine models Advanced Rules, Optional Rules, and future official
categories as kinds of Rule Module in one closed catalog. Their official
category remains domain-visible for presets, documentation, and UI, but all
modules share the same Ruleset configuration, deterministic event model,
replay, Public View, and deep rule-execution interface.

Separate execution systems per official category were rejected because modules
can interact across categories, while flattening every module into “Advanced
Rule Module” was rejected because it contradicts the official rule language.
