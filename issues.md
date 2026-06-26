# Issue Tracker

This file is **your input** to the automated fixer. You own it — edit it (ideally
on your Windows machine) and push. The automation reads it but never writes it;
it records progress in `AUTOMATION_STATUS.md` instead.

## How to use
- Add issues under **Open**, each with a unique short ID (`#1`, `#2`, …). Don't
  reuse IDs.
- Give each a `Priority:` (High / Medium / Low) — the automation works High first.
- Use labels in brackets: `[bug]`, `[feature]`, `[chore]`, `[docs]`.
- When a `fix/issue-N` branch is merged, remove that issue from here (its status is
  tracked in `AUTOMATION_STATUS.md` until then).
- Workflow: you add issues here → the box (cron) fixes them on `fix/issue-N`
  branches and pushes them → you pull, UI-test, and merge to `master`. Status and
  per-branch manual UI test steps live in `AUTOMATION_STATUS.md` / the branch
  commit message.

---

## Open

- [ ] #5 [bug] Tap/Untap animations not working
  - Details: Lands used to have a quick animation when tapping, but this stopped working after the engine-owned mana update and lands tap instantly. Animations also never worked for untapping.
  - Priority: Low

- [ ] #13 [feature] Regenerate mechanic
  - Details: Regenerate (CR 701.15) places a shield on a creature: the next time it would be destroyed that turn, the shield is consumed instead — the creature is tapped, removed from combat, and has its damage cleared. Add `regeneration_shields: u32` to `GameObject`, modify the destroy SBA to consume a shield before moving the creature to the graveyard, and add `SpellEffectKind::Regenerate` and an activated-ability `AbilityCost` path. This also fixes the Wrath of God "can't be regenerated" partial (add a `prevent_regeneration` flag to `DestroyAll`). Implement Cudgel Troll and Drudge Skeletons with test coverage for: shield consumed on lethal damage, shield not present on second lethal, Wrath ignoring shields.
  - Priority: Medium

- [ ] #14 [feature] Graveyard-return spells (Phase 3 zone-sourced effects)
  - Details: Add a `ReturnFromGraveyard { filter, destination }` tier-1 `SpellEffectKind` primitive that moves a permanent card from a graveyard to hand (or battlefield). The graveyard is public so no hidden-zone redaction is needed. Implement Disentomb and Raise Dead (graveyard → hand). Also implement Gravedigger, which uses an ETB trigger wrapping the same primitive. This is the first Phase 3 zone-sourced effect and lays the structural groundwork for reanimation and tutors. Add scenario tests covering: return own card, return opponent card, empty graveyard fizzle.
  - Priority: Medium

- [ ] #15 [feature] Library search / tutor effects
  - Details: Implement `SearchLibrary { filter, destination, shuffle }` as a tier-1 `SpellEffectKind` primitive (CR 701.18). The relay already has a `LibrarySearch` choice-kind discriminant in ruled_v1.proto for hidden-zone redaction — the engine needs to pause resolution, emit a `resolution_choice_required` with matching library cards visible only to the searching player (opponents see a count or face-down list), resume on `SubmitResolutionChoice`, move the card to destination, then shuffle the library. Implement Demonic Tutor (any card → hand) and Mystical Tutor (instant or sorcery → top of library). Add scenario tests for: successful search, shuffle verification, empty library, relay redaction (opponent sees no card names).
  - Priority: Medium

- [ ] #16 [feature] Multi-target damage distribution for X spells (Fireball)
  - Details: Fireball's full Oracle text allows any number of targets with X damage divided among them (paying 1 extra generic mana per target beyond the first). The current Fireball RON is partial (single target only). Add a `DamageTargets { amount: Amount, extra_mana_per_target: u32 }` `SpellEffectKind` variant that accepts a variable-length target list at cast time and distributes the declared X value. The cast UI needs to collect (target, amount) pairs. Also fixes the Fire half of Fire // Ice (split 2 damage between any number of targets). Add scenario tests for: single target full damage, split between two, illegal over-allocation, fizzle when all targets gone.
  - Priority: Low

- [ ] #17 [feature] "Attacks/blocks each combat if able" combat requirement
  - Details: Some creatures are required to attack or block each combat if they legally can (CR 508.1d, 509.1c — "must attack/block" requirements). Add a `must_attack: bool` and `must_block: bool` field to `GameObject` (settable by continuous effects or card data), and enforce them during declare-attackers and declare-blockers legality: the engine should return `EngineError::Illegal` if the player tries to end declaration while a must-attack creature is idle and a legal attack exists, or a must-block creature is idle while a legal block exists. Add scenario tests covering: must-attack creature blocked from skipping, must-attack creature can still skip if no legal attack target exists.
  - Priority: Low

- [ ] #18 [feature] MDFC battlefield face state (plan-multiface-cards.md Phase 1)
  - Details: The multi-face card substrate is done (split layout, `face_index` on `StackItem`, `face_names` in `CardCatalog`) but MDFCs (modal double-faced cards), transforming DFCs, and Adventure cards need `GameObject.face_up_index: Option<usize>` to track which face is showing on the battlefield. Add this field, wire it into all permanent characteristic queries (card_id, P/T, keywords, types, mana cost), emit a `FaceChanged` event in the proto on transform, and expose a "Transform" right-click action gated to applicable layouts. On entry to the battlefield, set `face_up_index` per layout rules (MDFCs enter on front face; day/night DFCs start as specified). This completes Phase 1 of plan-multiface-cards.md and unblocks Adventure casting-from-exile and MDFC land backs.
  - Priority: Low

- [ ] #19 [feature] Modal spell mode selection (Charm cycles, Cryptic Command)
  - Details: Modal spells (CR 700.2) require the casting player to choose one or more modes at cast time. Add a `modes: Vec<Vec<SpellEffectKind>>` field to `CardFace` as a tier-1 primitive; during spell casting, if a card has modes, emit a `resolution_choice_required` interrupt (reusing the existing proto pair) before targeting to collect the mode selection. The chosen mode's effects are then treated exactly like the card's `spell_effect` list for targeting and resolution. Implement Boros Charm (indestructible / double strike / 3 damage to player) as the reference card, and add scenario tests for each mode. This machinery also covers Entwine (pay extra to get all modes) and future Charm cycles with minimal additions.
  - Priority: Low

- [ ] #20 [feature] Reanimation spells (graveyard → battlefield)
  - Details: Add `ReturnFromGraveyardToBattlefield { filter, enters_tapped: bool }` as a tier-1 `SpellEffectKind` variant. This differs from graveyard-to-hand (#14) in that the card enters as a permanent and fires ETB effects. Implement Reanimate (return target creature from any graveyard to your battlefield, you lose life equal to its converted mana cost). For enchantment-based reanimation (Animate Dead, Necromancy), defer the aura-attachment part until auras (#10) are implemented, but stub the card as partial. Add scenario tests: reanimated creature has ETB triggers fired, original owner retains ownership, creature leaving battlefield via reanimation enchantment works correctly.
  - Priority: Low

- [ ] #21 [bug] Eyeblight's Ending non-Elf restriction not enforced
  - Details: Eyeblight's Ending currently targets and destroys any creature, missing the "non-Elf" restriction. Add `not_subtype: Option<String>` to `TargetFilter` (parallel to the existing `not_color: Option<Color>` field) and update the Eyeblight's Ending RON to use `not_subtype: Some("Elf")`. Update `legal_actions.rs` target filtering and the relay's target-legality pass. Add scenario tests verifying: Elf creature is not a valid target, non-Elf creature is a valid target, casting on an Elf returns `EngineError::Illegal`. This same field also fixes any future cards with "non-[Subtype]" targeting restrictions.
  - Priority: Low

- [ ] #22 [feature] Discard effect primitive (hand disruption)
  - Details: No `DiscardCards` or forced-discard variant exists in `SpellEffectKind` (`tricerules-cards/src/primitives.rs`). Add `DiscardCards { count: u32, target: TargetFilter }` (target must be a player kind) to cover "target player discards N cards" (Coercion, Hymn to Tourach, Thoughtseize). When `count` exceeds the player's hand size the player discards all remaining cards (CR 701.7a). Add scenario tests for: normal discard, discard more than hand size, empty hand no-op.
  - Priority: Medium

- [ ] #23 [feature] Forced-sacrifice effect (Diabolic Edict / Plaguecrafter)
  - Details: `sacrifice_permanent` already exists in `tricerules-core/src/engine/resolution.rs:923` but there is no `SpellEffectKind` variant that forces a target player to sacrifice. Add `TargetPlayerSacrifices { filter: TargetFilter }` (filter restricts what may be sacrificed — default creature). The targeted player chooses which qualifying permanent they sacrifice; this requires a `ResolutionChoiceRequired` interrupt (reusing the existing proto pair) so the player picks from their matching permanents. Implement Diabolic Edict (opponent sacrifices a creature) and add scenario tests for: valid sacrifice, no valid permanent fizzle, player choice validation.
  - Priority: Medium

- [ ] #24 [feature] Damage prevention shields (Healing Salve mode 2 / Fog-family)
  - Details: Healing Salve is partial (`tricerules-cards/data/healing_salve.ron`) — the "prevent the next 3 damage" mode is missing. Add `PreventNextDamage { amount: u32, target: TargetFilter }` to `SpellEffectKind` and a `damage_prevention: HashMap<ObjectId, u32>` (player or creature id → remaining shield) to `GameState`. During damage application, consume the shield before recording damage on the object/player. Also add `PreventAllDamageThisTurn {}` variant for Fog-style effects (stores a flag per player). Add scenario tests for: shield partially consumed, shield fully consumed, Fog blocks all combat damage.
  - Priority: Medium

- [ ] #25 [feature] Scry effect primitive
  - Details: No `Scry` variant exists in `SpellEffectKind`. Add `Scry { count: u32 }` — the player looks at the top `count` cards of their library and puts any number on the bottom in any order, keeping the rest on top in any order (CR 701.18). Because the library is a hidden zone, the server relays only the player's own library slice; the engine emits a `ResolutionChoiceRequired` interrupt (reusing the existing proto pair with a new choice-kind discriminant) so the player submits their reordering. Implement Preordain (Scry 2 then Draw 1) and Opt (Scry 1 then Draw 1). Add scenario tests for: put all on bottom, put all on top, empty library no-op.
  - Priority: Low

- [ ] #26 [feature] Untap-target and untap-all effects
  - Details: No untap-effect variants exist in `SpellEffectKind`. Add `UntapTarget { target: TargetFilter }` (Twiddle, Aphetto Alchemist) and `UntapAll { filter: TargetFilter }` (Turnabout, Dramatic Reversal). `UntapTarget` reuses the existing targeting machinery; `UntapAll` is untargeted like `DestroyAll`. The engine's zone-move path already handles `tapped` state — untap just clears the flag and emits a `PermanentMoved`/`TapState` event. Add scenario tests for: untap tapped permanent, untap already-untapped permanent (no-op), untap-all with mixed tapped state.
  - Priority: Low

- [ ] #28 [feature] "Whenever a creature dies" observer trigger (Blood Artist, Grim Haruspex)
  - Details: `TriggerCondition::WhenSelfDies` fires only on the card with the ability itself. No observer variant exists for watching other creatures die, which blocks the entire "death matters" archetype (Blood Artist, Zulaport Cutthroat, Grim Haruspex, Butcher of the Horde). Add `WheneverCreatureDies { controller: CastTriggerPlayer, exclude_self: bool }` to `TriggerCondition` in `tricerules-cards/src/primitives.rs`, mirroring the existing `WheneverPermanentEntersBattlefield` structure. The engine's `fire_triggers(GameEvent::Dies …)` path in `resolution.rs` must be extended to check all battlefield permanents for this new condition. Implement Blood Artist (whenever a creature dies, target player loses 1 life and you gain 1 life) and add scenario tests for: own creature dies, opponent's creature dies, source permanent itself dying.
  - Priority: Medium

- [ ] #29 [feature] Extra land per turn continuous effect (Exploration / Oracle of Mul Daya)
  - Details: Exploration is partial (`tricerules-cards/data/exploration.ron`) because no "play an additional land" continuous effect exists. Add `ExtraLandPlays(u32)` to `ContinuousEffectKind` in `tricerules-cards/src/primitives.rs` (the `// Future:` comment at line 706 explicitly reserves this slot). The engine's land-play legality check in `legal_actions.rs` must sum all active `ExtraLandPlays` effects to compute `max_land_plays_this_turn`. Wire the `ExtraLandPlays(1)` effect into Exploration's `StaticAbilityDef` (ETB emit + LTB drain, same pattern as `AnthemPt`). Also fix the Exploration partial marker. Add scenario tests for: play two lands with Exploration in play, Exploration leaving resets to one, multiple Explorations stack.
  - Priority: Medium

- [ ] #30 [bug] Legend rule: no controller choice + LTB triggers not fired (CR 704.5j)
  - Details: `apply_legend_sbas` in `tricerules-core/src/engine/continuous.rs` always keeps the permanent with the lowest `ObjectId` rather than letting the controller choose which to keep (CR 704.5j). It also calls `destroy_permanent` directly without routing through the normal die/LTB trigger path, so dies-triggers and leaves-battlefield triggers are never fired for the legend that was removed. Fix: (1) emit a `ResolutionChoiceRequired` for the controller to pick which legend to keep (reuse existing proto pair, new choice-kind discriminant); (2) route removal through `sacrifice_permanent` (already exists in `resolution.rs:923`) so death triggers fire normally. Add scenario tests for: controller chooses which legend to keep, LTB trigger fires on removed legend.
  - Priority: Medium

- [ ] #31 [feature] "At beginning of each player's draw step" trigger (Howling Mine)
  - Details: `TriggerCondition` has `AtBeginningOfControllerUpkeep` but no draw-step analog. Add `AtBeginningOfEachDrawStep { controller: CastTriggerPlayer }` (where `AnyPlayer` fires for every player's draw step, `Controller` for only the enchantment controller's, `Opponent` for opponents'). Wire it in `priority.rs` where the engine advances to the draw step. Implement Howling Mine ("at the beginning of each player's draw step, if Howling Mine is untapped, that player draws a card") — this requires the additional `if source is untapped` guard, expressible as a `requires_source_untapped: bool` field on the trigger def. Add scenario tests for: both players draw on their respective draw steps, Howling Mine tapped skips draw.
  - Priority: Low

- [ ] #32 [feature] "Whenever you gain life" trigger (Ajani's Pridemate, Heliod, Sun-Crowned)
  - Details: No `WheneverControllerGainsLife` trigger condition exists. Add `WheneverControllerGainsLife` to `TriggerCondition` in `tricerules-cards/src/primitives.rs`. The engine's life-gain paths — `GainLife` effect resolution, `TargetPlayerGainsLife`, and lifelink combat gains — must each fire `GameEvent::LifeGained { player, amount }` and `fire_triggers` must check permanents for this condition. Implement Ajani's Pridemate (whenever you gain life, put a +1/+1 counter on Ajani's Pridemate) reusing the existing `PutCounters` effect. Add scenario tests for: Pridemate grows on spell life-gain, Pridemate grows on lifelink combat damage, multiple life-gain events in one turn each trigger separately.
  - Priority: Low

---

## Backlog (not yet prioritized)

- Own version of MTGO's FACE card rendering
- Targeting visuals overhaul
