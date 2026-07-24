# Design Plan — Card-coverage primitive expansion

> **Status (2026-07-23):** P1–P2 shipped; P3–P5 remain TODO. Moved from repo root to `docs/`.

## Status

**P1 (static anthems/lords + one-shot mass pump) and P2 (target-filter widening) are
implemented.** P3–P5 remain **TODO** (P4 to be promoted to its own plan before any code).
This plan is the prioritized roadmap for widening the data-tier vocabulary so more real cards
become implementable without new custom Rust.

Shipped in P1: `StaticAbilityDef::AnthemPt` + `AnthemFilter` (controller/subtype/color/
exclude_self), `AffectedScope::CreaturesMatching` (dynamic, registry-evaluated),
`SpellEffectKind::PumpAll`. Cards: Glorious Anthem, Crusade, Bad Moon, Glorious Charge,
Inspired Charge; Captain of the Watch's +1/+1 Soldier anthem (vigilance grant still deferred).
Shipped in P2: `TargetFilter.not_color` + `.attacking_or_blocking`, and an optional
`spell_filter` on `CounterTargetSpell`/`CopyTargetSpell`. Cards: Doom Blade, Divine Verdict,
Essence Scatter, Negate; Twincast's instant/sorcery restriction now enforced. Next: re-run the
calibration triage (the re-measure gate below) before P3 / batch generation.

## Motivation

A calibration pass (sample triage of unimplemented modern-core commons) put the
"implementable with today's primitives" hit rate at roughly **14% full / ~29% incl.
partials** — too low to justify batch generation yet. The skips clustered into a small
number of recurring mechanic gaps. This plan fills them in **leverage order** (most cards
unblocked per unit of engineering first), so that re-measuring the hit rate after Phase 1–2
decides whether batch generation becomes worthwhile.

Each phase obeys the repo's "name at least two real cards" rule for every new primitive,
field, or variant.

Priority order: **P1 static anthems/lords → P2 target-filter widening → P3 zone-sourced
effects → P4 auras & equipment (attachment) → P5 regenerate + minor keywords.** P1 and P2
are modest, high-ROI data+engine changes; P4 is a structural project on its own.

---

## Current-state grounding

- **P/T computation** (`engine.rs` `effective_power`/`effective_toughness`, ~2060–2094):
  base (printed) → CR 613.4 layer 7c `ContinuousEffectKind::PtModify` continuous effects
  filtered by `ContinuousEffect::affects(oid)` → layer 7d counters. This is the only place
  P/T modification flows through, so any anthem/lord rides this exact seam.
- **`AffectedScope`** (`state.rs` ~246): currently `Single(ObjectId) | AllCreatures`, with a
  reserved `// Future: CreaturesControlledBy(PlayerId), CreaturesWithPower(u32), …`. The
  anthem/lord work is largely *filling in this enum* and teaching `affects` to evaluate it.
- **`ContinuousEffect::affects(&self, oid)`** (`state.rs` ~266) takes only an `ObjectId` — it
  has **no `&GameState`**, so it cannot currently test controller or card type. Filtered
  scopes require threading characteristics into the check (see P1 design).
- **`EffectDuration::WhileSourceOnBattlefield`** already exists and is drained at LTB
  (`engine.rs` ~5236, `move_object_to_zone`); `UntilEndOfTurn` is drained at cleanup
  (~2099). The duration model for static anthems is therefore already in place — only the
  *source* of such an effect (a static ability) is missing.
- **`ContinuousEffectKind`** (`primitives.rs` ~657) is `PtModify { delta_power,
  delta_toughness }` only, with reserved `// Future: Layer6AddKeyword(Keyword),
  Layer7bSetPt { power, toughness }`.
- **Card data model** (`card_def.rs`): a card has `keywords`, `activated_abilities`,
  `triggered_abilities`, `spell_effect`/`custom_effect` — **no `static_abilities`**. Static
  abilities (CR 604) are not modeled at all today.
- **`TargetFilter`** (`primitives.rs` ~231) is exactly `{ kind, not_artifact, tapped }`.
- **`Zone`** (`state.rs` ~30): `Hand, Battlefield, Graveyard, Exile, …`. The
  `ReturnTargetCreatureToHand` / `ReturnTargetPermanentToHand` effects are hard-wired
  battlefield→hand bounce with no source-zone parameter.
- **`AbilityCost`** (`primitives.rs` ~508): `Tap | Mana | TapAndMana | Sacrifice`.

---

## P1 — Static anthems & lords (filtered continuous P/T)  ⭐ highest leverage

**Cards (the "name two" bar, easily cleared):** Glorious Anthem, Lord of Atlantis, Bad Moon,
Goblin Chieftain, Captain of the Watch (currently a partial), Crusade. Plus the one-shot
sibling below covers Glorious Charge, Overrun, Rally the Peasants.

**Gap.** No way to express "creatures you control [of type X] get +N/+N" as a *static* effect,
nor "creatures you control get +N/+N until end of turn" as a *mass one-shot*. `PumpTarget` is
single-target.

**Design.**
1. **Data:** add `static_abilities: Vec<StaticAbilityDef>` to `CardDefinition`/`CardFace`
   (CR 604). First `StaticAbilityDef` variant: `AnthemPt { scope: AnthemScope, delta_power,
   delta_toughness }`, where `AnthemScope` mirrors the widened `AffectedScope` (all creatures /
   creatures you control / a type filter, with an `exclude_self` flag for "*other* creatures").
2. **Engine — widen `AffectedScope`** to fill the reserved variants:
   `CreaturesControlledBy(PlayerId)`, and a type/keyword-filtered
   `CreaturesMatching { controller: Option<PlayerId>, type_filter, exclude: Option<ObjectId> }`.
   Because `affects` needs characteristics, change the call sites to evaluate scope **with
   `&GameState`** (either `affects(&self, oid, &GameState)` or pre-resolve affected ids each
   query). The P/T seam at `effective_power/toughness` is the only consumer, so this is a
   contained signature change.
3. **Engine — static-ability lifecycle:** when a permanent with an `AnthemPt` static ability
   **enters the battlefield**, push a `WhileSourceOnBattlefield` `ContinuousEffect` with the
   resolved scope and `source_id = ` the permanent; LTB drain (already implemented) removes it.
   This reuses the existing duration plumbing exactly — the only new code is "emit the effect on
   ETB from a static ability" alongside the existing trigger-collection on ETB.
4. **Mass one-shot sibling (cheap rider):** once `AffectedScope` is widened, add
   `SpellEffectKind::PumpAll { scope, power, toughness }` resolving to an `UntilEndOfTurn`
   continuous effect with the filtered scope — no static-ability machinery needed. Covers
   Glorious Charge / Overrun (Overrun also grants trample → pairs with P5/Layer6AddKeyword).

**Why this is first.** Highest card count among the cheap fixes, and the engine already
reserves both the scope enum slots and the `WhileSourceOnBattlefield` duration — the seam was
designed for exactly this (see commit `3120efd8`). It also retires an existing partial.

**Tests.** `scenario.rs`: anthem buffs only matching creatures; a creature entering *after* the
anthem is buffed (scope is dynamic, not a snapshot); anthem source dying drains the buff and the
same SBA pass re-checks lethality (the cascade `3120efd8` built); `exclude_self` lord doesn't
buff itself; PumpAll expires at cleanup. `conformance.rs`: each new card resolves.

**Out of scope for P1:** keyword-granting anthems ("creatures you control have trample") — that
is the reserved `ContinuousEffectKind::Layer6AddKeyword`; add with the first card that needs it
(Goblin Chieftain's haple-grant, Overrun's trample). Power/toughness *setting* (layer 7b),
characteristic-defining P/T ("*/* equal to…").

---

## P2 — Target-filter widening

**Cards:** Divine Verdict & Hunt Down ("attacking or blocking"), Essence Scatter & Negate
("creature spell" / "noncreature spell"), Doom Blade ("nonblack"), Beast Within
(any-permanent). High frequency across removal, counters, and combat tricks.

**Gap.** `TargetFilter` is only `{ kind, not_artifact, tapped }`, so any restriction beyond
those forces a skip (or an unfaithful "loosening" partial that broadens the card).

**Design.** Extend `TargetFilter` with additive, AND-combined optional fields, each gated by
"name two cards":
- `controller: Option<ControllerConstraint>` — `Yours | Opponents` (Fog Bank tricks, "creature
  you control").
- `attacking_or_blocking: bool` (Divine Verdict, Falter-style).
- `color: Option<Color>` / `not_color: Option<Color>` (Doom Blade, Terror, Pacifism color
  restrictions) — reuses the existing `Color` enum and the mana-derived color query.
- `card_type` for permanent filters and a **spell-type filter for `CounterTargetSpell`**
  (currently unparameterized): `counters: Option<SpellTypeFilter>` reusing the existing
  `SpellTypeFilter` (Essence Scatter = Creature, Negate = Noncreature). This finally lets
  Twincast/Counterspell-class cards drop their "restriction unenforced" partial.

All new fields default to "no constraint", so the entire existing corpus is untouched. The
engine's generic legality/targeting paths already enumerate target-bearing effects via
`SpellEffectKind::target_filters()` — extend the predicate there in one place.

**Tests.** `scenario.rs`: a restricted-target spell is rejected against an illegal target and
accepted against a legal one (attacking-only, color, controller, creature-spell counter).
`conformance.rs`: new cards resolve. Update the Twincast/Essence-Scatter partials → full.

**Out of scope:** restrictions needing characteristics we don't track yet (mana value
thresholds, "with a +1/+1 counter", subtype like "Goblin") — add the specific predicate with
its first two cards.

---

## P3 — Zone-sourced return / search effects

**Cards:** Disentomb, Raise Dead, Gravedigger (graveyard→hand); Demonic Tutor, Diabolic Tutor
(library search→hand). Reanimation (graveyard→battlefield) is a natural extension but interacts
with ETB and is lower priority.

**Gap.** `ReturnTargetCreatureToHand` is hard-wired battlefield→hand bounce; there is no
graveyard-sourced return and no library search-to-hand.

**Design.**
- Add a `source_zone: Zone` (default `Battlefield`) to the return effects, OR a dedicated
  `ReturnFromGraveyard { filter }` — prefer the parameter so one effect serves bounce and
  regrowth. Targeting a graveyard card means the relay/UI must allow graveyard objects as
  legal targets (engine already tracks `Zone::Graveyard` objects).
- `SearchLibrary { filter, destination: Zone, reveal: bool, shuffle: bool }` (the
  primitive the card-model doc already cites for Demonic Tutor). Search is a hidden-zone
  choice → reuse the tier-3 `ResolutionChoiceRequired`/`SubmitResolutionChoice` machinery
  and the **private `LibrarySearch` choice kind** the relay already redacts (added in commit
  `e56548a6` for Gifts Ungiven) so the searcher's library isn't leaked.

**Tests.** `scenario.rs`: return a specific creature card from graveyard to hand (and reject a
noncreature for a creature-only filter); search resolves, moves the chosen card, shuffles, and
the candidate list is redacted from the opponent. `conformance.rs`: new cards resolve.

**Out of scope:** graveyard→battlefield reanimation (ETB interaction — its own mini-phase),
search with reveal-to-all, fetch-lands (need the land-drop/`ProduceMana` interplay).

---

## P4 — Auras & Equipment (attachment)  — largest count, structural

**Cards:** Pacifism, Holy Strength, Oakenform (Auras); Bonesplitter, Short Sword, any Equipment.
By raw count this is likely the single biggest gap, but it is a **structural engine project**,
not a primitive widening — sequence it after P1–P3.

**Gap.** No attachment state at all: an object cannot be "attached to" another, and there is no
SBA for an Aura falling off / an Equipment's controller.

**Design (sketch — promote to its own plan before building).**
- `GameObject.attached_to: Option<ObjectId>` and the inverse "attachments of X".
- Auras: cast targets a permanent (CR 303.4); on resolution the Aura enters the battlefield
  **attached** (not the normal ETB-to-open-battlefield path). SBA (CR 704.5n/m): an Aura
  attached illegally or to nothing is put into the graveyard — a new SBA in the `apply_sbas`
  fixpoint (the loop from `3120efd8` already re-checks to convergence).
- The Aura's effect is a `WhileSourceOnBattlefield` continuous effect **scoped to the attached
  object** — this reuses the P1 continuous-effect plumbing, which is a second reason P1 comes
  first. Pacifism-class "can't attack or block" needs a restriction layer (new
  `ContinuousEffectKind`), and keyword-granting auras need `Layer6AddKeyword` (the P1 reserved
  slot).
- Equipment: `{cost}: Attach` activated ability that moves the Equipment to a creature you
  control (CR 301.5); detaches when the creature leaves. Effects are the same attached
  continuous effects as Auras.

**Tests.** Aura attaches and buffs/restricts the host; host leaving sends the Aura to the
graveyard via SBA; Equipment re-attaches between creatures and survives the first creature
dying; illegal aura target rejected. Full scenario + conformance coverage (this is a structural
change → same bar as engine work).

**Out of scope (initially):** fortifications, Auras with triggered abilities, "enchant
player/land", reconfigure, modular.

---

## P5 — Regenerate + minor drawback / evasion keywords

**Cards:** Cudgel Troll, Drudge Skeletons (regenerate); Juggernaut, Berserkers of Blood Ridge
("attacks each combat if able"); landwalk creatures; "can't be blocked except by …".

**Gap.** A scattering of low-frequency keyword/ability mechanics that individually block one
card each but collectively tax older sets.

**Design.** Each is small and independent — implement opportunistically, two cards minimum each:
- **Regenerate** (CR 701.15): a regeneration *shield* state on a permanent set by an ability
  (`AbilityCost` + new `SpellEffectKind::RegenerateSelf`/`RegenerateTarget`); the destroy SBA
  consumes a shield instead of destroying (tap, remove from combat, heal). Touches the destroy
  path — scenario coverage required.
- **"Attacks/blocks each combat if able"** — a creature flag consulted in the
  declare-attackers/blockers legality (reject a combat that omits a must-attack creature able
  to attack). Drawback keyword → a new `Keyword` variant or a `combat_requirement` field.
- **Landwalk / conditional unblockable** — parameterized evasion (CR 702.x). These need
  characteristic matching against the defending player's permanents; model as a small
  `Evasion` enum on the creature, consulted in block legality. The card-model doc currently
  defers parameterized keywords to custom Rust — revisit: a bounded `Evasion` enum is data-tier.

**Tests.** Regenerate: a regenerated creature survives lethal damage once per shield and dies on
the second; must-attack creature forces a legal attack; landwalk creature is unblockable vs the
matching land type. `conformance.rs`: new cards resolve.

**Out of scope:** protection (CR 702.16 — multi-axis: damage/enchant/block/target, its own
plan), banding, phasing.

---

## Sequencing & re-measure gate

1. **P1 + P2** land first (modest data+engine changes, highest ROI).
2. **Re-run the calibration triage** (150–200 unimplemented modern-core cards, bucket skips by
   mechanic) and recompute the hit rate. If P1+P2 move it from ~14% into the worthwhile range,
   greenlight batch generation (per the existing low-risk batch agent prompt) for the
   P1/P2-unblocked band.
3. **P3** next if zone-sourced effects show up as a large remaining bucket.
4. **P4 (attachment)** is promoted to its own design plan before any code; it is the structural
   investment, and P1's continuous-effect plumbing is its prerequisite.
5. **P5** opportunistically, never as a batch.

## MTG applicability

CR governs every phase. P1: CR 604 (static abilities), 611/613.4 (continuous P/T, layers 7c/7d),
611.3/604.3 (static effects exist only while the source is on the battlefield). P2: CR 115
(targeting) + 601.2c (legal targets). P3: CR 700-zone changes + 701.16 (search) + hidden-zone
redaction (CR 400.2 player-private information). P4: CR 303/301 (Auras/Equipment), 704.5n/m
(attachment SBAs), 613 (the attached continuous effects). P5: CR 701.15 (regenerate), 508/509
(attack/block requirements), 702.x (landwalk/evasion). Names stay Oracle-sourced, mechanics
tricerules-owned, per the two-database rule. Each card cites its Oracle text + CR in-file and
carries happy + illegal scenario coverage, same standard as existing engine changes.

## Related plans

[[plan-copy-effects]] (permanent copy also rides the continuous-effect/layer seam P1 widens),
[[plan-tokens]] (anthem/lord scopes apply to tokens; Captain of the Watch makes Soldier tokens),
[[plan-counters]] (layer 7d, already the model for P1's layer 7c filtered scopes).
