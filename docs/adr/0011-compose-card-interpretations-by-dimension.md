---
status: accepted
---

# Compose physical Card Interpretations by dimension

Physical Card Interpretation Layers apply in effect order. A layer replaces
only the dimensions it declares, or adjusts the current value when its rule
explicitly says to add or subtract a bounded amount; unrelated dimensions
continue to compose. Formation matching, previews, point calculation, and
effect resolution all consume the same resolved element and level instead of
re-reading printed Card facts after a match has been accepted.

Sacred Art Multiplicity applies after element and level interpretation, gives
both match slots the same effective values, and is incompatible with Star
Element Substitution both before and after expansion. The previous
single-source model was rejected because Fire Spirit, Pouch, Blazing Yang Art,
Dark Spirit, and Star effects must compose by dimension without mutating Card
data. Virtual Formation Cards are Formation components rather than Card
Interpretation Layers and are governed by ADR 0025.
