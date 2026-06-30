---
status: accepted
---

# Separate card origin from current zone

Every Card Instance has an immutable Card Origin identifying either the shared
deck or its original Personal Deck Player. Decks and Discard Piles are owned by
that shared origin or by individual Players, while a card's current zone and
holder are tracked separately.

This allows Discard Retrieval to move another Player's card through the current
Player's Deck and hand without changing where that card must ultimately be
discarded. Changing ownership on each move was rejected because it loses the
provenance required for deterministic return, replay, and Public View.
