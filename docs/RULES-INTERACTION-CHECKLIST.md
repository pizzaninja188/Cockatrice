# Ruled change interaction checklist

Complete this checklist before finalizing the plan for a substantive engine, card primitive, protobuf, relay, or ruled-client change. Record N/A with a short reason when a section genuinely does not apply. The goal is to expose missing interactions before implementation, not to force every change across every component.

For the card-specific implementation workflow, use the canonical
[card authoring guide](../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md). This checklist
remains the separate interaction audit for substantive ruled changes.

## 1. Rules authority and intended behavior

- Record the current Oracle text and relevant rulings for card-specific behavior.
- Identify the governing CR concepts; verify exact rule numbers and quotations against the current official rules.
- State intentional simplifications or deferred mechanics explicitly.
- Name at least two cards or two distinct mechanics for every new reusable primitive or public vocabulary entry.

## 2. State ownership and identity

- Identify the authoritative state writer and the component that merely displays or relays it.
- Distinguish engine `ObjectId`, card definition ID, Oracle name, `Server_Card.id`, hand slot, zone position, and face index.
- Define what happens on every relevant zone change, including permission expiry, attachments, counters, face choice, and last-known information.
- Check replay determinism: every state-affecting choice must remain a logged command or deterministic engine decision.

## 3. Timing and interacting mechanics

- Identify the event boundary, active step/phase, priority holder, and whether actions are simultaneous.
- Check trigger collection/order, APNAP ordering, state-based actions, replacement/prevention ordering, and resolution interruption where applicable.
- Check continuous characteristics, layers, dependencies, copy effects, and last-known information where applicable.
- Confirm the behavior composes with existing targeting, combat restrictions, costs, permissions, and pending choices rather than bypassing shared legality.

## 4. Players, legality, and failure paths

- Use player-set-generic logic; do not assume two-player arithmetic.
- Distinguish owner, controller, affected player, choosing player, and defending player.
- Enumerate happy, illegal, stale-command, ambiguous, and no-longer-legal paths.
- Return `EngineError::Illegal` or the established protocol error path; do not panic on player input.

## 5. Visibility and trust boundaries

- Classify every new field/event as public, per-player, or server-only.
- Trace redaction for each recipient, including omitted-versus-empty retention semantics.
- Confirm logs, prompts, picker candidates, replays, and debug output do not leak hidden information.
- Keep rules decisions in tricerules; do not trust client-derived legality or Oracle display data.

## 6. End-to-end propagation

- List every producer and consumer affected in Rust, protobuf, Servatrice, and Qt.
- Update all protobuf constructors, generated consumers, relay bindings, redaction, client state, dispatcher methods, and UI actions that share the contract.
- Preserve ruled/freeform gating and the upstream-file extraction boundary.
- Verify physical zone order and stable identity when engine vectors and Cockatrice piles use different orderings.

## 7. Verification and delivery

- Add a focused regression that fails for the intended reason before implementation.
- Cover happy and illegal behavior plus the most likely interaction regression.
- Select focused iteration gates and full affected-side completion gates from `docs/AGENT-VERIFICATION.md`.
- Decide whether a real two-client manual check is required; specify exact steps and expected visible/physical state.
- Plan generated-data updates, tracker reconciliation, focused staging, and the final MTG-applicability summary.
