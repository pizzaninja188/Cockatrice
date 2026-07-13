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

- [ ] #19 [feature] Modal spell mode selection (Charm cycles, Cryptic Command)
  - Details: Modal spells (CR 700.2) require the casting player to choose one or more modes at cast time. Add a `modes: Vec<Vec<SpellEffectKind>>` field to `CardFace` as a tier-1 primitive; during spell casting, if a card has modes, emit a `resolution_choice_required` interrupt (reusing the existing proto pair) before targeting to collect the mode selection. The chosen mode's effects are then treated exactly like the card's `spell_effect` list for targeting and resolution. Implement Boros Charm (indestructible / double strike / 3 damage to player) as the reference card, and add scenario tests for each mode. This machinery also covers Entwine (pay extra to get all modes) and future Charm cycles with minimal additions.
  - Priority: Low

- [ ] #20 [feature] Reanimation spells (graveyard → battlefield)
  - Details: Add `ReturnFromGraveyardToBattlefield { filter, enters_tapped: bool }` as a tier-1 `SpellEffectKind` variant. This differs from graveyard-to-hand (#14) in that the card enters as a permanent and fires ETB effects. Implement Reanimate (return target creature from any graveyard to your battlefield, you lose life equal to its converted mana cost). For enchantment-based reanimation (Animate Dead, Necromancy), defer the aura-attachment part until auras (#10) are implemented, but stub the card as partial. Add scenario tests: reanimated creature has ETB triggers fired, original owner retains ownership, creature leaving battlefield via reanimation enchantment works correctly.
  - Priority: Low

- [ ] #21 [bug] Eyeblight's Ending non-Elf restriction not enforced
  - Details: Eyeblight's Ending currently targets and destroys any creature, missing the "non-Elf" restriction. Add `not_subtype: Option<String>` to `TargetFilter` (parallel to the existing `not_color: Option<Color>` field) and update the Eyeblight's Ending RON to use `not_subtype: Some("Elf")`. Update `legal_actions.rs` target filtering and the relay's target-legality pass. Add scenario tests verifying: Elf creature is not a valid target, non-Elf creature is a valid target, casting on an Elf returns `EngineError::Illegal`. This same field also fixes any future cards with "non-[Subtype]" targeting restrictions.
  - Priority: Low

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

- [ ] #33 [feature] Adventure cards — cast the creature half from exile (plan-multiface-cards.md §3)
  - Details: Adventure (CR 715) is the most stateful multi-face layout and builds on the shipped `faces` model + `face_index` plumbing (§1/§2 done via #18). Casting the adventure (spell) half resolves the card into **exile with permission** to later cast the creature half from exile. Add an "exiled with adventure" marker on the object (or an exile sub-zone) plus a cast-from-exile permission keyed to the creature face; `cast_spell` must accept casting a face from exile when that permission is present (today it only reads from hand). First card: Bonecrusher Giant // Stomp. Relay/client: surface the exiled adventure card as castable (the physical card sits in exile). Add scenario tests: adventure half resolves to exile, the creature is then castable from exile, casting the creature normally from hand still works, and the permission is one-shot.
  - Priority: Low

- [ ] #34 [feature] Transform / Flip permanents — TDFC + werewolves (plan-multiface-cards.md §4)
  - Details: With `GameObject.face_up_index` (#18) and the `FaceChanged` proto event already in place, implement **in-place** face changes. A `TransformPermanent` effect/keyword flips `face_up_index`; characteristic queries already read the active face. **CR 712.8: transforming does NOT trigger ETB.** Flip (CR 710) uses the same mechanism. Wire the engine's `transform_permanent()` (Transform/Flip layouts only — ModalDfc is rejected) to a trigger/effect, implement werewolf day/night transform triggers, and add the client display: the proto field `battlefield_face_up_index` is emitted but **not yet consumed by any C++** — consume it and add a card name-change path (there is no `AttrCardName`, so a rename-in-place mechanism or card re-send is needed; the play-land entry rename in `Server_Game` is the reference). Add scenario tests: transform swaps P/T/types/keywords in place without firing ETB; flip works; werewolf day/night triggers flip correctly. Lowest priority of the multiface phases.
  - Priority: Low

- [ ] #35 [chore] Multi-face card generator ingestion (plan-multiface-cards.md §5)
  - Details: The batch generator (`gen-cards`, `scripts/gen-cards.*`) currently filters `layout == "normal"` only, so no multi-face vanilla/keyword cards are ingested. Extend the filter to author a `faces` vec from Scryfall's `card_faces` array for split / MDFC / transform / adventure cards where **every** face is vanilla or uses only supported keywords, reusing the existing per-face supported-keyword and `ManaSymbol` checks; skip (and report) any card with unsupported text on any face. After generating, `cargo test` (registry load + `conformance` validate every generated face) then the checklist `--check`. Add a `--dry-run` count of qualifying multi-face cards and skip reasons.
  - Priority: Low

---

## Backlog (not yet prioritized)

- Own version of MTGO's FACE card rendering
- Targeting visuals overhaul
