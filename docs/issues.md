# Issue Tracker

> **Status (2026-07-23):** active tracker, moved from repo root to `docs/`; `scripts/auto-fix-issues.sh` paths updated to match.

This file is **your input** to the automated fixer. You own it — edit it (ideally
on your Windows machine) and push. The automation reads it but never writes it;
it records progress in `AUTOMATION_STATUS.md` instead.

## How to use
- Add issues under **Open**, each with a unique short ID (`#1`, `#2`, …). Don't
  reuse IDs.
- Give each a `Priority:` (High / Medium / Low) — the automation works High first.
- Use labels in brackets: `[bug]`, `[feature]`, `[chore]`, `[docs]`.
- Delete completed issues from this file rather than marking them checked. For work
  committed directly to `master`, remove the issue in the implementation commit; for a
  `fix/issue-N` branch, remove it immediately after merge. Before reporting completion,
  reconcile dependency wording in the remaining issues. Historical status stays in
  `AUTOMATION_STATUS.md`.
- Workflow: you add issues here → the box (cron) fixes them on `fix/issue-N`
  branches and pushes them → you pull, UI-test, and merge to `master`. Status and
  per-branch manual UI test steps live in `AUTOMATION_STATUS.md` / the branch
  commit message.

---

## Open

- [ ] #57 [feature] Targeting cost increases
  - Details: Add a reusable cost modifier for spells or abilities that target a protected object or player, with the surcharge included while legal casts and payments are computed. Calibration evidence: Boreal Elemental. The implementation should also serve ward-style mechanics, handle multiple taxed targets deterministically, and reject a cast when the selected targets make its total cost unpayable.
  - Priority: Low

- [ ] #58 [feature] Non-targeted discard and loot choices
  - Details: Generalize discard effects so the affected player chooses cards from their own hand and optional discard-then-draw sequences can branch without turning discard into targeting. Calibration evidence: Mind Rot, Rousing Read, Teferi's Protege, and Keldon Raider. The shared composite activated-cost and sorcery-speed timing paths are shipped; Liliana's Steward remains blocked here only on its opponent-controlled resolution-time discard choice. Reuse resumable resolution choices, enforce exact cardinality, and preserve hidden information in multiplayer views.
  - Priority: Low

- [ ] #59 [feature] Resolution-time modes, optional payments, and costs
  - Details: Extend resumable resolution choices with reusable mode selection and optional nonmana payment/cost branches. Calibration evidence: Trufflesnout, Sparktongue Dragon, and Crypt Lurker. Issue #88 shipped the shared generic-mana pay/decline foundation, including mana abilities during resolution, normal mana-counter payment, automatic acceptance when fully paid, and rollback on decline. Legal choice payloads must describe the remaining branches and required objects, stale submissions must be rejected atomically, and declining an optional branch must remain legal.
  - Priority: Low

- [ ] #65 [feature] Granted activated abilities
  - Details: Allow continuous effects to grant a parameterized activated ability to an affected permanent and expose it through that permanent's legal actions. Calibration evidence: Gift of Paradise. Preserve the granted ability's source identity, controller, costs, and mana-ability timing, remove it when the effect ends, and satisfy the two-card-or-mechanic reuse gate before adding the primitive.
  - Priority: Low

- [ ] #72 [feature] Planeswalker and battle objects as rules targets
  - Details: Add the missing nonplayer object kinds and legal target vocabulary so damage and destroy effects can include planeswalkers and battles where Oracle permits. Calibration evidence: Chandra's Magmutt, Finishing Blow, Goblin Arsonist, Pitchburn Devils, Scorch Spitter, Shock, Sorcerer of the Fang, and Viashino Pyromancer. This requires engine-owned card types, battlefield identity, damage/defeat state-based actions, proto/relay/client representation, and end-to-end target selection; do not approximate these objects as players.
  - Priority: Low

- [ ] #80 [feature] Protection
  - Details: Implement protection as a parameterized keyword consumed by damage prevention, attachment legality, blocking legality, and targeting legality rather than four card-specific checks. Calibration evidence: Feat of Resistance. Cover an Aura or Equipment becoming illegal, already-declared blocks, damage sources with matching characteristics, and satisfy the two-card-or-mechanic reuse gate before adding the keyword.
  - Priority: Low

- [ ] #86 [feature] Related-player effect recipients
  - Details: Add recipient expressions for the controller of a target, the defending player or planeswalker associated with an attacker, and each opponent attacking an enchanted player. Calibration evidence: Chandra's Outrage, Scorch Spitter, Curse of Opulence, and Curse of Disturbance. Issue #62 shipped the controller of a blocking-creature recipient with generation-aware last known information; issue #94 shipped typed player attachment identity and the two Curses as partial cards; issue #63 shipped event-time attacking/defending-player context and the attacked-player attachment trigger. This issue retains the Curses' controller-created reward plus each-attacking-opponent reward. Derive recipients from engine identities at the event or resolution time required by the effect, with controller-aware multiplayer coverage.
  - Priority: Low

- [ ] #89 [feature] Battlefield-to-library zone moves
  - Details: Add a zone-move effect from battlefield to the top or bottom of a library with explicit owner destination and ordering. Calibration evidence: Totally Lost. Preserve object identity through the public-to-hidden transition, redact the resulting private library state, and satisfy the two-card-or-mechanic reuse gate before adding the primitive.
  - Priority: Low

- [ ] #90 [feature] Search for a land onto the battlefield tapped
  - Details: Extend library search with a land filter, battlefield destination routed through the shared replacement-aware entry funnel, and mandatory shuffle. Calibration evidence: Evolving Wilds. Issue #52 shipped its tap plus self-sacrifice activation cost; this issue remains the owner-scoped search, reveal, failure-to-find, replacement-aware battlefield entry, and deterministic mandatory shuffle behavior.
  - Priority: Low

- [ ] #92 [feature] Look, choose a matching card, and bottom the rest
  - Details: Add a resumable library-selection algorithm that looks at the top N cards, optionally chooses one matching a filter for hand, and puts the rest on the bottom in the rules-required order. Calibration evidence: Brightwood Tracker. Preserve hidden information, deterministic ordering, exact choice validation, and satisfy the custom-effect review plus two-card-or-mechanic reuse gates before choosing a primitive or tier-3 implementation.
  - Priority: Low

- [ ] #46 [feature] Token copy effects — Populate and create-a-copy tokens
  - Details: Build on the shipped permanent copy-layer snapshot, effective-face accessors, and battlefield display identity from #45. Bridge the token lifecycle and `CreateTokens` support to that snapshot. Support both targeted “create a token that's a copy of target permanent” effects and untargeted Populate-style choices without conflating the latter with CR 115 targeting; reuse one snapshot/minting helper and keep player sets generic. A copied token must receive the chosen permanent's copiable values, including existing copy effects, while excluding counters, damage, attachments, and non-copy continuous effects; copying an inline token must not require a registry `CardId`. Reuse the shipped effective battlefield display identity through proto, relay, and ruled client paths. Scenario coverage: copy a registry-backed permanent; copy an inline token; copy an already-copied permanent; prove counters and temporary pumps are excluded; reject illegal/stale targets or choices; verify token ownership, controller, zone changes, and cease-to-exist behavior. Add conformance and end-to-end display coverage. Double-faced tokens and copy-with-modifications remain deferred.
  - Priority: Low

---

## Backlog (not yet prioritized)

- Own version of MTGO's FACE card rendering
- Targeting visuals overhaul
