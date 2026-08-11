---
status: accepted
---

# Resolve Void Spirit-Shattering atomically

Void Spirit-Shattering Technique resolves every affected Spirit's two-point
power loss before applying the 20-point HP loss for each Spirit owner, and all
resulting deltas form one canonical resolution before the Game Outcome is
evaluated. Automatic Bloom eligibility observes the reduced Spirit Power, so a
six-power Wood Spirit reduced to four cannot answer HP loss caused by the same
Technique. Interleaved per-owner events were rejected because their serialized
order could change Spirit breaking, Bloom eligibility, or the winner.

Dark Glimmer extends the same atomic resolution. The Technique snapshots its
initial Spirit owners for the per-owner HP deductions, reduces every Spirit's
power, breaks zero-power Spirits, resolves 魔靈復甦, and resolves Shared Fate
from each surviving Death Spirit whose owner's Team HP was actually deducted by
this Formation effect before evaluating the Game Outcome. This is not an
Affected Player Set rule and does not include Attack damage. A Death Spirit
broken because the Technique reduced it to zero does not trigger Shared Fate,
and a replacement Spirit summoned by 魔靈復甦 neither adds another HP deduction
nor triggers retroactively.
