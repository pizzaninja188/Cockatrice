# Design Plan — Card-coverage primitive expansion

> **Status (verified 2026-08-05):** P1–P5 shipped. The calibration re-measure remains the next
> gate.

## Status

This plan is the prioritized roadmap for widening the data-tier vocabulary so more real cards
become implementable without new custom Rust. Its current state is:

- **P1–P2: shipped.** Static anthems/lords, mass pump, and the first target-filter widening are
  implemented.
- **P3: shipped.** `ReturnFromGraveyard` and `SearchLibrary` landed with private-choice routing;
  graveyard-to-battlefield reanimation followed as an extension.
- **P4: shipped.** Auras and Equipment share dynamic attachment modifiers for P/T, keywords, and
  combat restrictions; attachment SBAs enforce ongoing enchant/equip legality.
- **P5: shipped.** Regeneration, "attacks/blocks each combat if able," and parameterized basic
  landwalk are implemented. Further conditional-evasion forms can extend the same `Evasion`
  characteristic when two real cards justify a new reusable value.

Shipped in P1: `StaticAbilityDef::AnthemPt` + `AnthemFilter` (controller/subtype/color/
exclude_self), `AffectedScope::CreaturesMatching` (dynamic, registry-evaluated),
`SpellEffectKind::PumpAll`. Cards: Glorious Anthem, Crusade, Bad Moon, Glorious Charge,
Inspired Charge; Captain of the Watch's Soldier P/T and vigilance anthem.
Shipped in P2: `TargetFilter.not_color` + `.attacking_or_blocking`, and an optional
`spell_filter` on `CounterTargetSpell`/`CopyTargetSpell`. Cards: Doom Blade, Divine Verdict,
Essence Scatter, Negate; Twincast's instant/sorcery restriction now enforced. Next: re-run the
calibration triage (the re-measure gate below) before further batch generation or another
structural primitive investment.

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

- Derived characteristics flow through `GameEngine::characteristics` and explicit CR 613 layer
  slots in `engine/characteristics.rs`. Layer 6 keyword grants and layer 7c/7d modifiers/counters
  are active; the same pipeline evaluates filtered continuous-effect scopes.
- Runtime card characteristics are faces-only. `StaticAbilityDef`, `AffectedScope`, widened
  `TargetFilter`, and multi-effect activated/triggered abilities live under
  `tricerules-cards/src/primitives/`.
- `ReturnFromGraveyard` and `SearchLibrary` are resolved in `engine/resolution/zones.rs`; private
  library candidates reuse the generic resolution-choice protocol and relay redaction.
- `GameObject.attached_to`, Aura/Equipment primitives, attachment SBAs, regeneration shields,
  and attack/block requirement flags are all active engine state. Parameterized `Evasion` values
  remain separate from the parameterless `Keyword` enum; `combat::can_block` AND-composes both
  while dynamically checking the defending player's derived permanent characteristics.

---

## P1 — Static anthems & lords (filtered continuous P/T)  ⭐ highest leverage

> **Shipped.** The design below is retained as implementation history.

**Cards (the "name two" bar, easily cleared):** Glorious Anthem, Lord of Atlantis, Bad Moon,
Goblin Chieftain, Captain of the Watch, Crusade. Plus the one-shot
sibling below covers Glorious Charge, Overrun, Rally the Peasants.

**Historical gap.** The engine could not express static filtered P/T effects or mass one-shot
pumps; `PumpTarget` was single-target.

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

**Later extensions:** `AnthemKeyword` and the layer-6 keyword-grant effects subsequently shipped;
Captain of the Watch exercises the static form. Power/toughness *setting* (layer 7b) and
characteristic-defining P/T ("*/* equal to…") remain outside this phase.

---

## P2 — Target-filter widening

> **Shipped.** The initial color, combat-state, controller, permanent-type, and spell-type
> restrictions landed. Add further predicates only when two real cards justify them.

**Cards:** Divine Verdict & Hunt Down ("attacking or blocking"), Essence Scatter & Negate
("creature spell" / "noncreature spell"), Doom Blade ("nonblack"), Beast Within
(any-permanent). High frequency across removal, counters, and combat tricks.

**Historical gap.** `TargetFilter` originally carried only `{ kind, not_artifact, tapped }`, so
other restrictions forced a skip or an unfaithful broadening.

**Shipped design.** `TargetFilter` uses additive, AND-combined fields including
`attacking_or_blocking`, color inclusion/exclusion, controller restriction, type/subtype
constraints, and other reusable predicates. `CounterTargetSpell` and `CopyTargetSpell` carry a
shared optional `SpellTypeFilter`, covering Essence Scatter, Negate, and Twincast without
card-name branching.

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

> **Shipped.** `ReturnFromGraveyard` supports graveyard-to-hand and graveyard-to-battlefield
> destinations; `SearchLibrary` uses the private `LibrarySearch` resolution choice. Implemented
> cards include Disentomb, Raise Dead, Gravedigger, Demonic Tutor, Mystical Tutor, Reanimate, and
> Zombify.

**Cards:** Disentomb, Raise Dead, Gravedigger (graveyard→hand); Demonic Tutor, Diabolic Tutor
(library search→hand). Reanimation (graveyard→battlefield) is a natural extension but interacts
with ETB and is lower priority.

**Historical gap.** Battlefield bounce could not express graveyard-sourced return or library
search-to-hand.

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

**Still out of scope:** search with reveal-to-all and fetch lands. Reanimation was implemented
after this phase with controller-aware battlefield entry and ETB handling.

---

## P4 — Auras & Equipment (attachment)  — largest count, structural

> **Shipped.** `GameObject.attached_to`, Aura/Equipment targeting and resolution, attachment SBAs,
> relay/client attachment presentation, and the unified `AttachedModifier` cover P/T changes,
> keyword grants, and combat restrictions. Holy Strength, Unholy Strength, Oakenform, Pacifism,
> Flight, Guard Duty, Indestructibility, Bonesplitter, Short Sword, Vulshok Morningstar, and
> Swiftfoot Boots exercise the substrate.

**Cards:** Pacifism, Holy Strength, Oakenform (Auras); Bonesplitter, Short Sword, any Equipment.
By raw count this is likely the single biggest gap, but it is a **structural engine project**,
not a primitive widening — sequence it after P1–P3.

**Historical gap.** The engine had no attachment state or attachment SBAs. The shipped substrate
closed that structural gap; the remaining work is expanding what attached continuous effects can
express.

**Shipped design.**
- `GameObject.attached_to: Option<ObjectId>` and the inverse "attachments of X".
- Auras: cast targets a permanent (CR 303.4); on resolution the Aura enters the battlefield
  **attached** (not the normal ETB-to-open-battlefield path). SBA (CR 704.5n/m): an Aura
  attached illegally or to nothing is put into the graveyard — a new SBA in the `apply_sbas`
  fixpoint (the loop from `3120efd8` already re-checks to convergence).
- `StaticAbilityDef::AttachedModifier` emits `WhileSourceOnBattlefield` effects dynamically scoped
  through `AffectedScope::AttachedTo`: layer-6 keyword grants, layer-7c P/T modifiers, and
  non-layered combat restrictions. Re-equipping moves the whole modifier set together.
- Equipment: `{cost}: Attach` activated ability that moves the Equipment to a creature you
  control (CR 301.5); detaches when the creature leaves. Effects are the same attached
  continuous effects as Auras.
- Attachment SBAs use derived characteristics: an Aura whose host stops satisfying its printed
  enchant filter goes to the graveyard, while Equipment whose host stops being a creature merely
  detaches. Existing attachments ignore shroud/hexproof because those restrict targeting only.

**Tests.** Scenarios cover attachment/P/T modification, keyword grant/removal and reattachment,
Pacifism attack/block legality including must-attack interaction, Indestructibility, illegal Aura
hosts, Equipment detachment, illegal cast/equip targets, and registry validation. Conformance
covers every added card.

**Out of scope (initially):** fortifications, Auras with triggered abilities, "enchant
player/land", reconfigure, modular.

---

## P5 — Regenerate + minor drawback / evasion keywords

> **Shipped.** Regeneration shields and their destroy/SBA interactions are implemented with Cudgel
> Troll and Drudge Skeletons. Attack/block requirements are engine-authoritative; Juggernaut,
> Crazed Goblin, and Goblin Brigand exercise must-attack. River Boa and Shanodin Dryads exercise
> parameterized Islandwalk and Forestwalk through one generic basic-landwalk implementation.

**Cards:** Cudgel Troll, Drudge Skeletons (regenerate); Juggernaut, Berserkers of Blood Ridge
("attacks each combat if able"); landwalk creatures; "can't be blocked except by …".

**Gap.** A scattering of low-frequency keyword/ability mechanics that individually block one
card each but collectively tax older sets.

**Design status.** The items are independent and require two real cards minimum each:
- **Regenerate: shipped.** `SpellEffectKind::Regenerate`, regeneration-shield state, destroy
  replacement, tap/remove-from-combat/heal behavior, cleanup expiry, and "can't be regenerated"
  bypasses are covered by focused scenarios.
- **"Attacks/blocks each combat if able": shipped.** Face-level requirement flags feed combat
  declaration legality and the authoritative required attacker/blocker sets.
- **Basic landwalk: shipped.** `Evasion::Landwalk { land_subtype }` is a face characteristic,
  separate from parameterless `Keyword`. Block legality dynamically checks lands controlled by
  the defending player through derived characteristics, and the shared legality predicate keeps
  explicit rejection, must-block calculation, menace pairing, and auto-skip consistent. Island
  and Forest demonstrate subtype reuse without card-specific Rust.

**Coverage.** Scenarios prove matching and nonmatching subtypes, no-land behavior, controller vs.
owner, two landwalk values, composition with existing evasion, illegal block rejection, and
declare-blockers auto-skip. Registry conformance covers both cards.

**Out of scope:** protection (CR 702.16 — multi-axis: damage/enchant/block/target, its own
plan), banding, phasing.

---

## Sequencing & re-measure gate

1. **Re-run the calibration triage** (150–200 unimplemented modern-core cards), bucket skips by
   mechanic, and recompute the full/partial hit rate against the now-shipped P1–P5 substrate.
2. If the hit rate is worthwhile, generate or author the newly unblocked data-tier band and run
   the registry/conformance/checklist gates.
3. Extend the shipped `Evasion` representation only when the sample supports another conditional
   evasion form; keep each value player-set-generic and require two real cards.

## MTG applicability

CR governs every phase. P1: CR 604 (static abilities), 611/613.4 (continuous P/T, layers 7c/7d),
611.3/604.3 (static effects exist only while the source is on the battlefield). P2: CR 115
(targeting) + 601.2c (legal targets). P3: CR 700-zone changes + 701.16 (search) + hidden-zone
redaction (CR 400.2 player-private information). P4: CR 303/301 (Auras/Equipment), 704.5n/m
(attachment SBAs), 613 (the attached continuous effects). P5: CR 701.15 (regenerate), 508/509
(attack/block requirements), and 702.14 (landwalk). Names stay Oracle-sourced, mechanics
tricerules-owned, per the two-database rule. Each card cites its Oracle text + CR in-file and
carries happy + illegal scenario coverage, same standard as existing engine changes.

## Related plans

- [Copy effects](plan-copy-effects.md): permanent copy uses the same characteristics/layer seam.
- Tokens and counters are shipped engine substrates rather than remaining standalone plans;
  anthem/lord scopes already apply to token creatures, and counters occupy layer 7d beneath the
  same characteristics pipeline.
