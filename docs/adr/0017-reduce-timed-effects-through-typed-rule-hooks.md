---
status: accepted
---

# Reduce timed effects through typed rule hooks

變徵‧淨火 asks each enabled Rule Module for typed duration-reduction changes
to that module's timed effects, then records and applies the combined changes as
one atomic resolution. Each module retains its specialized effect state and
returns declarative deltas; the Echo module neither converts every effect into a
generic Status Effect nor switches directly over other modules' internal types.
Within a hook, multiple state components produced by one logical effect are
grouped and reduced together: 光芒's Cannot Act and Cannot Draw statuses remain
synchronized, while separate 光芒 uses remain independent. The established
serialized Status Effect contract is unchanged.

Hooks opt effects in by their published source semantics rather than by scanning
every state value with a duration. Timed Formation effects, Covered Passives,
public Counter Effects, active Delayed Spells, and effects explicitly named by
the rulebook are eligible; unrelated Profession, Spirit Skill, and permanent
effects do not become eligible merely because their representation has expiry
data.
