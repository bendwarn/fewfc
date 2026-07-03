# Keep initial Pouch selection in the Rules Engine

Initial Pouch Selection is an engine-managed, pre-deal stage rather than room
or Web-adapter configuration. Players choose private Card Instances in Turn
Order from their unshuffled Personal Decks, and canonical events record each
face-down placement. After every Player has chosen, a trusted adapter shuffles
each remaining Personal Deck and submits the result as canonical randomness;
only then does the engine deal initial hands and begin the first Turn. This
preserves deterministic replay, validates that each Pouch came from the correct
Deck, and keeps its hidden information under the existing Public View boundary.
The lifecycle is modeled as Game Preparation - Initial Pouch Selection,
Pending Deck Shuffle, and Initial Deal - rather than adding pre-turn values to
the Turn Phase enum. The game becomes Ongoing only after preparation completes.
Starting the room locks membership, rules, and Deck Lists immediately and moves
Players to this reconnectable in-game preparation state; Initial Pouch Selection
is not a waiting-room workflow.
