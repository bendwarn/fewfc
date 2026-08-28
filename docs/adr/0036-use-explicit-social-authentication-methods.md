---
status: accepted
---

# Use explicit social Authentication Methods

Google and GitHub are optional Authentication Methods for a Player Identity.
They are enabled only when their complete client-id and client-secret pair is
configured. OAuth tokens are encrypted at rest. Email and password remains an
Authentication Method, while password recovery is deliberately out of scope.

The system never implicitly links identities merely because their email addresses
match. A signed-in Player may explicitly link a Google or GitHub identity, even
when its email differs. This intentionally accepts the risk of a stolen active
session gaining a durable credential; OAuth state and CSRF protections remain
mandatory. A provider identity already attached to another permanent Player
Identity is rejected, rather than merging two permanent identities. Removing the
last Authentication Method is also rejected.

An Anonymous Player Identity may upgrade through the same social flow only after
leaving every waiting or active online room. A successful upgrade transfers its
Player Profile, custom Deck List, and saved Replay references without rewriting
any Durable Object Player Seat. For a new social identity, the anonymous profile
and Deck List transfer. When the provider already belongs to a permanent Player
Identity, that existing profile and Deck List win; an absent Deck List is filled
from the anonymous one, and replay conflicts retain the existing identity's
version. An unset Player Profile avatar may be filled from a provider image, but
a configured game avatar and game display name are never overwritten.

## Considered options

- Implicit same-email linking was rejected because email equality is not a
  sufficient authorization signal for attaching a durable credential.
- Rewriting active Durable Object Player Seats was rejected because a seat is a
  stable online Game Room identity and changing it would split room authority
  across D1 and Durable Object state.
- Requiring fresh re-authentication for explicit linking was rejected for this
  product release; the accepted active-session risk is documented above.
- Merging two permanent Player Identities was rejected because neither one has a
  trustworthy universal precedence rule for profile, Deck List, and replay data.
