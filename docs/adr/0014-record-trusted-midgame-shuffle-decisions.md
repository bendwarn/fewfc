---
status: accepted
---

# Record trusted midgame shuffle decisions

When a rule such as 商調‧鳴金 shuffles a Deck during play, a trusted application
adapter supplies the shuffled remainder through a dedicated randomness decision
boundary, and the canonical event records the complete resulting order. Player
commands select only rule-authorized choices and never supply shuffled order;
replay consumes the recorded order instead of rerunning a PRNG, preserving a
pure deterministic rules core without making persisted games depend on one
random algorithm. After the Player selects the searched Card, the unresolved
shuffle is persisted as Pending Randomness and blocks further resolution until
the adapter submits a validated permutation, allowing safe retry across Worker
errors or restarts.
