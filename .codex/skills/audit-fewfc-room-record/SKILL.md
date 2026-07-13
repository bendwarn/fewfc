---
name: audit-fewfc-room-record
description: Audit FEWFC local online-room canonical records from a room UUID. Use when the user asks to inspect a room log, explain a recorded Formation, Status, Spirit Skill, HP, Shield, or event-order interaction, or verify replay behavior in the FEWFC repository.
---

# Audit FEWFC Room Record

Inspect the requested local Wrangler `GameRoom` record without modifying room storage. Build the verdict from canonical events, the recorded prerequisites, and the repository's current rule contract.

## Workflow

1. Confirm the repository contains `apps/web/worker/durable-objects/game-room.ts`. Read `CONTEXT.md`, the relevant section of `docs/rule.md`, `docs/rules-engine-decisions.md`, and any directly relevant ADR before interpreting domain behavior.

2. Locate the room and list its canonical commands:

   ```bash
   bun .codex/skills/audit-fewfc-room-record/scripts/inspect-room-record.ts ROOM_UUID --repo "$PWD"
   ```

   Completion: the output identifies exactly one Durable Object database and lists the record's command IDs. If no room is found, also search intentionally selected alternate Wrangler roots such as `apps/web/.wrangler/e2e`; do not assume the room is remote.

3. Resolve user-facing names to stable IDs before filtering. Search source and rule docs with `rg`, for example:

   ```bash
   rg -n "天雷劫火" src docs
   ```

4. Extract the smallest relevant record slice. Use a command ID for the resolved action and `--match` for prerequisites such as a Status grant, Spirit transformation, Shield creation, or Environment change:

   ```bash
   bun .codex/skills/audit-fewfc-room-record/scripts/inspect-room-record.ts ROOM_UUID \
     --repo "$PWD" --command-id 70

   bun .codex/skills/audit-fewfc-room-record/scripts/inspect-room-record.ts ROOM_UUID \
     --repo "$PWD" --match DivineCalculation --match SpiritTransformed --until-command 70
   ```

   Multiple `--match` values use OR semantics. Add `--setup` only when Card Instance IDs, Teams, Turn Order, or enabled Rule Modules must be decoded. Add `--metadata` only when room-level facts matter. Keep unrelated hidden Cards out of the response.

5. Audit the interaction as a state transition:

   - Establish the pre-action owner, Team, HP, Shield, Status, Spirit, Environment, and pending-choice facts that affect the rule.
   - Validate the submitted Cards against printed/effective elements and levels when Formation legality is in question.
   - Walk canonical events in order and calculate every `old_hp`, `delta`, `new_hp`, and `effective_delta` independently.
   - Distinguish the Formation's main effect, additional effect, attack resolution, consumed protection, and secondary consequences such as Shared Fate. Event order may serialize one atomic resolution; consult the rule contract before treating ordering as separate timing.
   - Derive the Affected Player Set rather than inferring it from a Team HP delta alone.

   Completion: every observed delta and semantic event is attributed to one rule source, and every relevant prerequisite is supported by an earlier canonical event or setup fact.

6. Verify the conclusion. Run the narrowest existing Rust test that covers the same interaction. If correctness is disputed or a bug is suspected, use `diagnosing-bugs` to create an agent-runnable assertion against this exact captured room entry before forming hypotheses. Report the assertion command and focused test result.

7. Lead with `correct`, `incorrect`, or `inconclusive`. Then give the compact event timeline, explain surprising secondary effects, cite local rule/code lines, state validation commands and results, and mention whether files or room data were changed.

## Script contract

`scripts/inspect-room-record.ts` is read-only and requires Bun, Node.js, and the `sqlite3` CLI. The Bun/TypeScript entry point uses a bundled Node/V8 bridge only to decode Miniflare's V8-serialized `metadata` and `snapshot` values; Bun's native serialization format is not V8-compatible.

- Default: list canonical commands.
- `--command-id N`: print the exact record entry for command N.
- `--match TEXT`: print entries whose serialized source or events contain TEXT; repeat for OR matching.
- `--until-command N`: exclude entries after command N.
- `--setup`: include canonical setup.
- `--metadata`: include room metadata.
- `--database PATH`: inspect a known Durable Object SQLite file directly.

Treat a decode failure as an unsupported storage format, not evidence about game correctness.
