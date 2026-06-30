---
status: accepted
---

# Separate card origin from current zone

Every Card Instance has an immutable Card Origin identifying either the shared
deck or its original Personal Deck Player. Decks and Discard Piles separately
have a Pile Owner identifying the shared game or one Player, while a card's
current zone and holder are tracked independently.

This allows Discard Retrieval to move another Player's card through the current
Player's Deck and hand without changing where that card must ultimately be
discarded. Changing ownership on each move was rejected because it loses the
provenance required for deterministic return, replay, and Public View.
