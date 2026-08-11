# Keep initial Pouch selection in the Rules Engine

Initial Pouch Selection is an engine-managed, pre-deal stage rather than room
or Web-adapter configuration. Every Player may independently choose one private
Card Instance from their unshuffled Personal Deck while the shared stage is
open. Each accepted choice commits immediately in server arrival order and
records its face-down placement; it does not wait for, reserve, or block another
Player's choice. The choices commute because each Player changes only their own
Deck and Pouch, so their final projected game state is independent of canonical
arrival order even though the event log preserves that order.

After every Player has chosen, a distinct canonical completion event closes the
selection stage. A trusted adapter then shuffles each remaining Personal Deck
in Turn Order and submits the result as canonical randomness; only after those
shuffles does the engine deal initial hands and begin the first Turn. This
preserves deterministic replay, validates that each Pouch came from the correct
Deck, and keeps its hidden information under the existing Public View boundary.

The lifecycle is modeled as Game Preparation - Initial Pouch Selection,
Pending Deck Shuffle, and Initial Deal - rather than adding pre-turn values to
the Turn Phase enum or representing the Players' choices as Pending Choices.
The game becomes Ongoing only after preparation completes. Starting the room
locks membership, rules, and Deck Lists immediately and moves Players to this
public, reconnectable in-game preparation state; Initial Pouch Selection is not
a waiting-room workflow.
