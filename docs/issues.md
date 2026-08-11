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

- [ ] #53 [feature] Additional spell costs
  - Details: Model discard and sacrifice as composable additional costs paid atomically with mana during casting, rather than as resolution effects. Calibration evidence: Bone Splinters, Thrill of Possibility, Tormenting Voice, and Village Rites. Legal actions must expose valid payable combinations, and an illegal or stale choice must leave mana, hand, and battlefield unchanged. This is the `PendingRuledSpellCast` extraction trigger recorded in `docs/REFACTOR-ROADMAP.md`: first land a characterized, behavior-preserving extraction of the remaining ruled cast/payment orchestration from upstream `player_actions.{h,cpp}` into `cockatrice/src/game/ruled/ruled_pending_cast.{h,cpp}`, leaving `PlayerActions` with a member pointer and thin ruled guards; then add the additional-cost behavior as a separate green increment. Fold the spell, activated-ability, and new additional-cost pending state into the existing exclusive pending-choice model instead of adding another parallel pending family. The engine remains authoritative over payable combinations and atomic command validation; the client only stages engine-published choices, and freeform behavior must remain unchanged.
  - Priority: Low

- [ ] #54 [feature] Activated-ability timing and board-state restrictions
  - Details: Add reusable activation predicates for sorcery-speed timing and conditions over controlled permanents or keywords. Calibration evidence: Celestial Enforcer, Goblin Bird-Grabber, and Liliana's Steward's secondary `activation_timing_and_discard_choice` gap. Issue #52 shipped Liliana's tap plus self-sacrifice cost shape; its sorcery-speed restriction remains here and its opponent discard resolution remains in Issue #58. Integrate the predicates into authoritative legal actions and command validation. Caged Zombie can reuse the shipped `GameCondition::CreatureDeathsThisTurn`; its tap-and-mana cost is already supported and its untargeted each-opponent life-loss effect shipped with Issue #87, so the activation predicate is its only remaining gap.
  - Priority: Low

- [ ] #55 [feature] Conditional mana output and restricted mana spending
  - Details: Represent mana abilities whose output depends on current battlefield state and mana whose spending is restricted by spell or ability characteristics. Calibration evidence: Leafkin Druid, Chandra's Embercat, and Vodalian Arcanist. Track restrictions on individual mana contributions through payment selection, reject illegal mixed payments, and keep unrestricted mana behavior unchanged.
  - Priority: Low

- [ ] #56 [feature] Conditional spell-cost reductions
  - Details: Add a reusable cost-modification path whose reduction is derived from current game state and applied before payment enumeration. Calibration evidence: Winged Words. Cover the condition becoming true or false before casting, generic-cost floors, and interactions with additional costs; satisfy the two-card-or-mechanic reuse gate before committing a new primitive.
  - Priority: Low

- [ ] #57 [feature] Targeting cost increases
  - Details: Add a reusable cost modifier for spells or abilities that target a protected object or player, with the surcharge included while legal casts and payments are computed. Calibration evidence: Boreal Elemental. The implementation should also serve ward-style mechanics, handle multiple taxed targets deterministically, and reject a cast when the selected targets make its total cost unpayable.
  - Priority: Low

- [ ] #58 [feature] Non-targeted discard and loot choices
  - Details: Generalize discard effects so the affected player chooses cards from their own hand and optional discard-then-draw sequences can branch without turning discard into targeting. Calibration evidence: Mind Rot, Rousing Read, Teferi's Protege, and Keldon Raider; Liliana's Steward still depends on this resolution-time discard choice after Issue #52 shipped its composite activation cost. Reuse resumable resolution choices, enforce exact cardinality, and preserve hidden information in multiplayer views.
  - Priority: Low

- [ ] #59 [feature] Resolution-time modes, optional payments, and costs
  - Details: Extend resumable resolution choices with reusable mode selection and optional payment/cost branches. Calibration evidence: Trufflesnout, Sparktongue Dragon, and Crypt Lurker. Legal choice payloads must describe the available branches and required objects or mana, stale submissions must be rejected atomically, and declining an optional branch must remain legal.
  - Priority: Low

- [ ] #62 [feature] Block-event triggers and related-player recipients
  - Details: Extend combat events and trigger matching for "becomes blocked" and "blocks a creature," including effects applied to the blocking creature or its controller without targeting. Calibration evidence: Gloom Sower and Snarespinner; Gloom Sower also supplies the secondary `blocking_creature_controller_recipient` gap. Cover multiple blockers, one blocker assigned to multiple attackers where legal, removal before resolution, and trigger-source LKI.
  - Priority: Low

- [ ] #63 [feature] Triggers from the object carrying an attachment
  - Details: Let Equipment and Auras define triggers keyed to events involving the object they are attached to, while snapshotting the attachment relation needed at trigger time. Calibration evidence: Heart-Piercer Bow and Unholy Indenture. Cover attack and death events, detachment or zone changes before resolution, and multiple attachments without hard-coding card names.
  - Priority: Low

- [ ] #64 [feature] Granted and delayed triggered abilities
  - Details: Represent continuous and turn-scoped effects that grant a triggered ability to another object, including a delayed expiry boundary. Calibration evidence: Infernal Scarring and Abnormal Endurance. The granted trigger must use the affected object's identity and LKI, survive its source leaving when appropriate, expire deterministically, and participate in simultaneous trigger/APNAP collection.
  - Priority: Low

- [ ] #65 [feature] Granted activated abilities
  - Details: Allow continuous effects to grant a parameterized activated ability to an affected permanent and expose it through that permanent's legal actions. Calibration evidence: Gift of Paradise. Preserve the granted ability's source identity, controller, costs, and mana-ability timing, remove it when the effect ends, and satisfy the two-card-or-mechanic reuse gate before adding the primitive.
  - Priority: Low

- [ ] #66 [feature] Triggered abilities on created tokens
  - Details: Let token definitions carry reusable triggered abilities that are registered immediately when the token enters. Calibration evidence: Goblin Wizardry. Cover simultaneous token creation, token LKI, zone changes and cease-to-exist behavior, plus registry/conformance validation; satisfy the two-card-or-mechanic reuse gate before widening the token schema.
  - Priority: Low

- [ ] #67 [feature] Board-state predicates for attack and ETB triggers
  - Details: Add composable trigger predicates over controlled permanent types, total or maximum power, and named permanents. Calibration evidence: Ornery Dilophosaur, Scholar of Stars, Turret Ogre, and Faerie Miscreant. Evaluate intervening conditions at the rules-required times, use controller-relative multiplayer semantics, and share one predicate vocabulary rather than introducing card-specific trigger variants.
  - Priority: Low

- [ ] #68 [feature] Multi-attacker trigger conditions
  - Details: Add a reusable attack trigger predicate based on the declared attacking group, not one event per creature. Calibration evidence: Makeshift Battalion. Capture the simultaneous declaration set, trigger only once when its threshold is met, and cover attacks split among multiple defending players or planeswalkers; satisfy the two-card-or-mechanic reuse gate before adding the condition.
  - Priority: Low

- [ ] #71 [feature] Power and keyword predicates in target and sacrifice filters
  - Details: Extend the shared filter vocabulary with power comparisons and keyword absence, usable for targets and selections. Issue #52 shipped derived keyword-presence matching in the shared target/mass/cost predicate for Portcullis Vine. Remaining calibration evidence: Legion's Judgment, Reckless Air Strike, Run Afoul, and Goblin Smuggler's secondary `power_target_filter` gap. Keep derived characteristics controller-aware and reusable.
  - Priority: Low

- [ ] #72 [feature] Planeswalker and battle objects as rules targets
  - Details: Add the missing nonplayer object kinds and legal target vocabulary so damage and destroy effects can include planeswalkers and battles where Oracle permits. Calibration evidence: Chandra's Magmutt, Finishing Blow, Goblin Arsonist, Pitchburn Devils, Scorch Spitter, Shock, Sorcerer of the Fang, and Viashino Pyromancer. This requires engine-owned card types, battlefield identity, damage/defeat state-based actions, proto/relay/client representation, and end-to-end target selection; do not approximate these objects as players.
  - Priority: Low

- [ ] #73 [feature] Multiple distinct and optional targets
  - Details: Generalize target schemas beyond one homogeneous required target to support independently filtered slots, distinctness constraints, and "up to N" cardinality. Calibration evidence: Soul Salvage, optional targets on Frost Breath and Ghostform, and Hunter's Edge's secondary `multiple_distinct_targets` gap. Legal actions must enumerate or validate complete assignments without Cartesian explosion, and stale, duplicate, missing, or overfilled submissions must be rejected before costs are paid.
  - Priority: Low

- [ ] #75 [feature] Opponent-only mass effect scopes
  - Details: Add a controller-relative scope for all creatures or permanents controlled by opponents, distinct from targeting. Calibration evidence: Uncomfortable Chill. Support multiplayer opponent sets, snapshot or recompute the affected set according to effect duration, and satisfy the two-card-or-mechanic reuse gate before adding the scope.
  - Priority: Low

- [ ] #76 [feature] Attacking-creature scopes for one-shot and static effects
  - Details: Add reusable scopes for "attacking creatures" both during one-shot resolution and as a continuously evaluated static restriction or modifier. Calibration evidence: Trumpet Blast and Warded Battlements. Use authoritative combat assignments, handle attacks against multiple defenders, and stop applying the scope when an object leaves combat.
  - Priority: Low

- [ ] #77 [feature] Unblockable effects and mass blocking restrictions
  - Details: Generalize combat legality modifiers that make selected creatures unable to block or make an affected creature unable to be blocked for a duration. Calibration evidence: Destructive Tampering, Frilled Sea Serpent, Ghostform, and Goblin Smuggler. Ensure explicit blocks, must-block effects, menace, and automatic combat progression all consume the same legality predicate.
  - Priority: Low

- [ ] #78 [feature] Conditional continuous characteristics
  - Details: Add reusable static conditions for changing power/toughness, keywords, or attack permissions from current game state or turn state. Calibration evidence: Daggersail Aeronaut, Drowsing Tyrannodon, Gearsmith Guardian, and Gearsmith Prodigy. Evaluate conditions in the correct characteristics layer without controller circularity, and share condition expressions with other continuous effects where possible.
  - Priority: Low

- [ ] #79 [feature] Name- and counter-filtered static scopes
  - Details: Extend continuous-effect scopes to select permanents by printed or copiable name and by counter presence. Calibration evidence: Pack Mastiff and Pridemalkin. Define which derived name and counter state are observed, preserve timestamp/dependency ordering, and make the predicates reusable outside these two cards.
  - Priority: Low

- [ ] #80 [feature] Protection
  - Details: Implement protection as a parameterized keyword consumed by damage prevention, attachment legality, blocking legality, and targeting legality rather than four card-specific checks. Calibration evidence: Feat of Resistance. Cover an Aura or Equipment becoming illegal, already-declared blocks, damage sources with matching characteristics, and satisfy the two-card-or-mechanic reuse gate before adding the keyword.
  - Priority: Low

- [ ] #81 [feature] Type-adding continuous effects
  - Details: Add layer-aware continuous effects that add card types or subtypes without overwriting existing ones. Calibration evidence: Dub. Preserve dependencies with other type-changing effects, update downstream creature/tribal filters, and satisfy the two-card-or-mechanic reuse gate before committing the primitive.
  - Priority: Low

- [ ] #82 [feature] Attached-subject effects and untap restrictions
  - Details: Let an Aura or Equipment apply a continuous effect to its attached object and express "doesn't untap during its controller's untap step." Calibration evidence: Capture Sphere, including its secondary `attached_subject_effect` gap. Reuse attachment-derived scopes, evaluate the affected object's current controller, stop the effect immediately on detachment, and satisfy the two-card-or-mechanic reuse gate.
  - Priority: Low

- [ ] #83 [feature] Effects over Equipment attached to a target
  - Details: Add a reusable way for resolution effects to enumerate and affect attachments of a specified kind on a chosen permanent. Calibration evidence: Turn to Slag. Revalidate the target and attachment set independently, preserve deterministic ordering for simultaneous zone moves, and satisfy the two-card-or-mechanic reuse gate before adding a new effect primitive.
  - Priority: Low

- [ ] #85 [feature] Creature-deals-damage-equal-to-power effects
  - Details: Add a reusable effect in which a selected creature deals damage equal to its current power to another selected creature, with independently filtered distinct targets from Issue #73. Calibration evidence: Hunter's Edge and Rabid Bite. Determine power and source characteristics at resolution, handle a missing source or target legally, and attribute damage to the creature for downstream triggers and prevention.
  - Priority: Low

- [ ] #86 [feature] Related-player effect recipients
  - Details: Add recipient expressions for the controller of a target or blocking creature and the defending player or planeswalker associated with an attacker. Calibration evidence: Chandra's Outrage, Gloom Sower's secondary controller-recipient gap, and Scorch Spitter. Derive recipients from engine identities at the event or resolution time required by the effect, with controller-aware multiplayer coverage.
  - Priority: Low

- [ ] #88 [feature] Soft counters with optional payment
  - Details: Add a reusable counter-unless-the-controller-pays effect that pauses resolution for the spell's controller to make and pay a legal mana choice. Calibration evidence: Convolute. Reuse the payment engine and resolution-choice path, revalidate available mana, keep the spell on the stack when payment succeeds, and satisfy the two-card-or-mechanic reuse gate.
  - Priority: Low

- [ ] #89 [feature] Battlefield-to-library zone moves
  - Details: Add a zone-move effect from battlefield to the top or bottom of a library with explicit owner destination and ordering. Calibration evidence: Totally Lost. Preserve object identity through the public-to-hidden transition, redact the resulting private library state, and satisfy the two-card-or-mechanic reuse gate before adding the primitive.
  - Priority: Low

- [ ] #90 [feature] Search for a land onto the battlefield tapped
  - Details: Extend library search with a land filter, battlefield destination routed through the shared replacement-aware entry funnel, and mandatory shuffle. Calibration evidence: Evolving Wilds. Issue #52 shipped its tap plus self-sacrifice activation cost; this issue remains the owner-scoped search, reveal, failure-to-find, replacement-aware battlefield entry, and deterministic mandatory shuffle behavior.
  - Priority: Low

- [ ] #91 [feature] Aggregate results of milling cards
  - Details: Let a compound effect inspect the cards moved by its immediately preceding mill operation and derive a count from their characteristics. Calibration evidence: Gorging Vulture. Keep the moved-object result local to one resolving effect chain, use graveyard-zone characteristics, and satisfy the two-card-or-mechanic reuse gate before adding cross-effect result plumbing.
  - Priority: Low

- [ ] #92 [feature] Look, choose a matching card, and bottom the rest
  - Details: Add a resumable library-selection algorithm that looks at the top N cards, optionally chooses one matching a filter for hand, and puts the rest on the bottom in the rules-required order. Calibration evidence: Brightwood Tracker. Preserve hidden information, deterministic ordering, exact choice validation, and satisfy the custom-effect review plus two-card-or-mechanic reuse gates before choosing a primitive or tier-3 implementation.
  - Priority: Low

- [ ] #93 [feature] Skip the next untap of an affected permanent
  - Details: Add a consumable marker or delayed replacement that causes a permanent not to untap during its controller's next untap step, then expires whether or not that step untaps other objects. Calibration evidence: Frost Breath. Cover controller changes, already-untapped permanents, multiple applications, and skipped untap steps; satisfy the two-card-or-mechanic reuse gate before committing the representation.
  - Priority: Low

- [ ] #94 [feature] Player-attached Auras and Curses
  - Details: Generalize Aura attachment identity so an Aura can enchant either a battlefield object or a player under CR 303.4, without representing players as fake `ObjectId`s. Use a typed engine-owned attachment recipient and extend authoritative target publication, cast validation, resolution revalidation, ongoing enchant legality, CR 704.5m state-based actions, and player-leaves-game handling; Equipment remains permanent-only. Propagate the public recipient identity through `ruled_v1.proto`, every Rust/C++ constructor and consumer, Servatrice physical binding, and a ruled-client presentation that does not require a parent `CardItem`; preserve freeform behavior. Applicability evidence: Curse of Opulence and Curse of Disturbance. Cover self and opponent recipients, multiplayer player sets, illegal/stale/eliminated recipients, zone changes, replay determinism, reflection visibility classification, relay/client regressions, ruled E2E synchronization, and a manual two-client check. Card-specific attack triggers and related-player effects may reuse or remain explicitly dependent on #62, #63, and #86 rather than being hard-coded into the attachment substrate.
  - Priority: Low

- [ ] #95 [bug] Quiet Windows runner reports successful stderr as failure
  - Details: `scripts/run-quiet-command.ps1` runs under Windows PowerShell with `$ErrorActionPreference = 'Stop'`; redirected output from a successful native command that writes progress to stderr becomes a terminating `NativeCommandError`, enters the catch block, and is reported as exit 1 even when the process exited 0. Reproduced with `cmd.exe /d /c "echo progress 1>&2 & exit /b 0"`; an extended-length `\\?\C:\...` working directory also makes `cmd.exe` emit a successful stderr warning and hits the same false-failure path. Capture complete stdout/stderr logs without treating native stderr as process failure, normalize or launch safely from extended-length drive paths, and preserve the wrapped process's exact exit code. Extend `tests/scripts/run_quiet_command_test.ps1` through the real `powershell.exe -File` entrypoint with stderr-plus-exit-0 and extended-path fixtures while retaining the existing exit-7/log regressions.
  - Priority: Medium

- [ ] #46 [feature] Token copy effects — Populate and create-a-copy tokens
  - Details: Build on the shipped permanent copy-layer snapshot, effective-face accessors, and battlefield display identity from #45. Bridge the token lifecycle and `CreateTokens` support to that snapshot. Support both targeted “create a token that's a copy of target permanent” effects and untargeted Populate-style choices without conflating the latter with CR 115 targeting; reuse one snapshot/minting helper and keep player sets generic. A copied token must receive the chosen permanent's copiable values, including existing copy effects, while excluding counters, damage, attachments, and non-copy continuous effects; copying an inline token must not require a registry `CardId`. Reuse the shipped effective battlefield display identity through proto, relay, and ruled client paths. Scenario coverage: copy a registry-backed permanent; copy an inline token; copy an already-copied permanent; prove counters and temporary pumps are excluded; reject illegal/stale targets or choices; verify token ownership, controller, zone changes, and cease-to-exist behavior. Add conformance and end-to-end display coverage. Double-faced tokens and copy-with-modifications remain deferred.
  - Priority: Low

- [ ] #37 [feature] Continuous control-change effects (Mind Control, Threaten)
  - Calibration: The Issue #44 `continuous_control_change` gap for Act of Treason maps to this existing issue.
  - Details: Issue #20 filled the CR 613 layer-2 slot for control decided **at battlefield entry** (`GameObject::controller`, read by `characteristics()`; the per-player `battlefield` list is the control index). Effects that change control of a permanent already on the battlefield are still unimplemented: `apply_layer_2_control` in `tricerules-core/src/engine/characteristics.rs` is an empty stub and its doc comment records the two traps. (1) It cannot use `ordered_effects` — that runs after layer 5 and its `effect_affects` reads `pre_layer_6.controller`, which is circular for a `CreaturesMatching { controller }` scope; this is exactly CR 613.8 dependency ordering. It needs its own earlier pass over `AffectedScope::Single` effects on the same `(timestamp, index)` key. (2) Once the derived controller can differ from the `controller` field, the battlefield lists stop being a valid control index and must be rebuilt whenever `continuous_effects` changes (add `reindex_battlefield_control()`, called from `apply_sbas`); the `debug_assert_battlefield_control_index` check in `state_based.rs` currently holds the two in sync and will start failing. Cards: Mind Control, Threaten / Act of Treason (until-EOT control + untap + haste), Confiscate, Ray of Command. The relay and client already handle a permanent sitting on a non-owner's table, so this is engine-side plus the existing `Owner:` annotation.
  - Priority: Low

---

## Backlog (not yet prioritized)

- Own version of MTGO's FACE card rendering
- Targeting visuals overhaul
