# Card authoring guide

This is the canonical workflow for adding or changing cards in the ruled `tricerules` registry.
It covers source research, implementation-tier selection, RON and custom-Rust authoring,
presentation metadata, partial-card tracking, generation, and completion checks.

Repository-level authority, architecture, and verification rules still apply. Follow
[`tricerules/AGENTS.md`](../../AGENTS.md) and use
[`docs/AGENT-VERIFICATION.md`](../../../docs/AGENT-VERIFICATION.md) as the command source of truth.

## 1. Research before editing

Never implement a card from memory.

1. Fetch the exact Scryfall card with a descriptive `User-Agent`.
2. Fetch the card's `rulings_uri`, even when no ruling is expected to change the implementation.
3. Verify `name`, layout and faces, `mana_cost`, `type_line`, power/toughness or loyalty/defense,
   complete Oracle text, and relevant rulings.
4. Verify exact Comprehensive Rules numbers and quotations against the current official rules.
5. If the name is ambiguous, the lookup fails, or authoritative sources disagree, stop before
   writing RON or Rust.

PowerShell lookup:

```powershell
$headers = @{ 'User-Agent' = 'CockatriceFork/1.0'; 'Accept' = 'application/json' }
$card = Invoke-RestMethod `
  -Uri 'https://api.scryfall.com/cards/named?exact=Howling%20Mine' `
  -Headers $headers
$rulings = Invoke-RestMethod -Uri $card.rulings_uri -Headers $headers
```

Oracle governs card-specific behavior; the Comprehensive Rules govern the mechanics. Record the
governing concepts and any intentional simplification or deferral in the implementation or its
verification evidence. Do not substitute Oracle Tagger classifications for reading the card.

## 2. Authority boundary

There are two card databases, and they must not be mixed:

| Database | Owner | Purpose |
|---|---|---|
| `cards.xml` and external Oracle/Scryfall data | Cockatrice client and freeform | Display, images, names, type lines, and search |
| `tricerules-cards` data, primitives, and custom Rust | Rules engine | Costs, legality, targets, characteristics, abilities, and effects |

- RON and Rust are authoritative for every ruled mechanical decision.
- External Oracle data supplies presentation wording only. The engine, relay, and client must not
  infer legality or resolution from it.
- Decks cross IPC by Oracle name, but `CardRegistry` resolves them to engine-owned card identity.
  Keep the card definition ID, face ID, Oracle name, physical object ID, and client card ID distinct.
- Rules RON contains no copied Oracle display prose.
- An unimplemented mainboard card must continue to block ruled game start through `ValidateDeck`;
  never add a silent casual fallback.

### Format scope and rules correctness

Format legality controls campaign selection, not engine semantics. Implement reusable primitives
according to the rules behavior they represent, without format-specific assumptions. Verify
relevant interactions beyond the selected cohort when they could expose an incomplete
implementation. Passing the selected cards' tests does not establish correctness for every
consumer of a primitive. Use concrete non-Standard examples or regressions when they reveal a
relevant distinction; this does not require authoring unrelated cards or exhaustively searching
the corpus. Preserve format-specific rules where the rules themselves require them.

Campaign instructions set the coverage priority and permission to include additional cards.
Any admitted card still requires complete-card preflight, review, and applicable semantic evidence.
When a campaign admits cards outside its target format, report target-format and broader coverage
separately; broader additions do not satisfy missing target-format coverage.

## 3. Choose the lowest implementation tier

Use the lowest tier that completely expresses the behavior:

1. **Generated data:** supported vanilla and french-vanilla cards from the pinned Scryfall bulk
   input.
2. **Hand-authored data:** RON under `tricerules-cards/data/`, using existing typed primitives.
3. **Generic primitive:** widen or add a typed effect, trigger, cost, condition, filter, or keyword
   when static parameters can describe the behavior.
4. **Custom Rust:** only for a unique resumable resolution algorithm involving live
   mid-resolution or interdependent choices.

Before adding a primitive, name at least two real cards or two distinct mechanics it supports.
Generalize only for demonstrated uses. If only the motivating card fits, justify the necessary
specialized behavior rather than adding speculative parameters. Two cards sharing a custom
algorithm are evidence that the algorithm belongs in a generic primitive.

`SpellEffectKind` is shared by spells, activated abilities, and triggered abilities. Prefer a
reusable typed effect over a card-specific path. `TargetKind::Self_` binds the source without
targeting under CR 115 and is invalid in spell effects; do not treat every effect subject as a
chosen target.

## 4. Author the card definition

For a hand-authored card:

1. Copy `mana_cost` verbatim in Scryfall brace syntax.
2. Represent the exact faces, type line, supertypes, subtypes, colors, and printed numeric
   characteristics.
3. Set the whole-card `id` to `slugify(name)` and give every face stable identity.
4. Express mechanics with typed costs, filters, targeting, conditions, abilities, and ordered
   effects. Do not encode rules in comments, labels, or presentation fields.
5. Add the `.ron` anywhere under `tricerules-cards/data/`; `build.rs` discovers it automatically.
   Do not edit a registry list.
6. Add happy and illegal scenario coverage with explicit zone, step, priority, and state assertions.

Stable IDs use canonical snake_case, remain stable when definitions are reordered, and are not
renumbered after release. Use the specific ID field for each surface:

Condition lists are conjunctive. Use `AnyOf([branch_a, branch_b, ...])` when Oracle gives
alternative conditions for the same effect, as on Hidden Lair and Gathering Place. `AnyOf`
requires at least two distinct, independently valid branches; do not duplicate the surrounding
ability to represent each branch.

| Surface | Stable ID |
|---|---|
| Card face | `face_id` |
| Activated, triggered, static, or characteristic ability | `ability_id` |
| Modal option | `mode_id` |
| Cast-cost group | `group_id` |
| Cast-cost option | `option_id` |
| Resolution branch | `branch_id` |
| Heterogeneous search slot | `slot_id` |
| Restricted-mana rule | `restriction_id` |

For Rust engine behavior, reject illegal input with `EngineError::Illegal` rather than panicking.
Keep steps, priority, event-time facts, new-object identity, hidden information, and player roles
explicit. Use player-set-generic logic rather than two-player arithmetic.

### Printed-card filters and graveyard targets

Use `ZoneCardFilter` for printed characteristics outside the battlefield and stack: searches,
hand-reveal costs, graveyard choices, and graveyard counts share this predicate. Leaves combine
with AND; `any_of` contains at least two distinct branches and cannot share a node with leaf fields.
Required subtypes all match, excluded subtypes/types must not match, and mana-value bounds are
inclusive. `has_adventure` tests the presence of Adventure characteristics, not which face was
last cast. `printed_power` inspects the normal/front printed numeric power; it does not evaluate
battlefield modifiers.

`GraveyardFilter` adds target context around an optional `card` predicate. Its `owner` and
generation-aware `excluded_objects` apply to every branch. Omit `card` for any card in the allowed
graveyards; `card: Some(())` is invalid. Do not use battlefield `TargetFilter` for these predicates.

```ron
// A land card with the Cave subtype in your graveyard.
filter: (owner: Controller, card: Some((card_type: Some(Land), required_subtypes: ["Cave"])))

// A non-targeting creature-or-land choice/search.
filter: (any_of: Some([(card_type: Some(Creature)), (card_type: Some(Land))]))
```

Exact names are rules names, not joined registry/deck names. Split cards and Rooms match either
half's name; Adventure, Omen, flip, and double-faced cards use their normal/front name here.
The removed graveyard characteristic fields and singular `ZoneCardFilter.subtype` are rejected;
author `card: Some(...)` and `required_subtypes: [...]` directly.

### Stack spells and permanent ownership

Use `StackSpellFilter` for counter/copy spell targets. `card_type`, `is_color`, and inclusive
mana-value bounds combine with AND. `any_of` follows the pure-OR validation of `TargetFilter`:
at least two distinct terminal predicates and no leaf fields on an OR node. Annul and Get Out
use type alternatives; Flashfreeze uses red/green color alternatives. Stack colors use the
selected face and applicable current color effects, including for spell copies.

`TargetFilter.owner: You` means the effect controller owns the permanent, independently of
`controller`. The default `Any` preserves unrestricted ownership. Get Out combines `owner: You`
with `kind: AnyPermanent` and `permanent_types: [Creature, Enchantment]`. Owner restrictions
require permanent-only kinds and must appear within each leaf of an OR filter. Context-free
mass, combat, and entry-copy filters reject relative ownership rather than silently ignoring it.

### Saga definitions

Author a Saga face with both `"Enchantment"` and `"Saga"` types and one
`SagaChapter(chapters: [...])` trigger per printed chapter ability. Chapter numbers must be
positive and strictly increasing; a combined ability such as “III, IV” uses one trigger with
`chapters: [3, 4]`. Do not author the ordinary one-lore-counter entry ability: the registry
materializes it from the Saga type. For read ahead, add `ReadAhead` to `keywords`; the engine then
uses the existing replacement-order and resolution-branch choice contracts to choose the entry
lore count and trigger only the resulting chapter.

## 5. Author presentation metadata

Ruled card data contains stable presentation references but no copied Oracle prose. Cockatrice
resolves those references against external Oracle data for prompts, context menus, choice labels,
and synthetic ability cards. If external data is missing or incompatible, the engine's
deterministic fallback remains usable.

Physical spells use their engine-authored face name as prompt and stack identity. They do not carry
a face-level Oracle presentation mapping: the rendered physical card already supplies its printed
text, and repeating the complete rules text inside `Choose a target for ...` obscures the action.
Nested spell choices still use stable paths beginning with the spell node, but that path component
does not require its own presentation mapping.

### Token and state-marker display prerequisites

Ruled-created token and state-marker artwork/details use the client's separately imported
Magic-Token database, normally `tokens.xml`. Updating `cards.xml` (including its ruled face text) or
engine RON does not refresh that database.

For a blank or incorrect token display:

1. Check the exact engine-emitted token/state name and the physical client's mapped identity.
2. Find the configured token database path in Cockatrice settings; do not assume the default path.
3. Inspect that file for the exact entry, its image references, and its source/version metadata.
   Compare against the current Magic-Token source used by Oracle's Tokens import.
4. If missing or stale, run Oracle's **Tokens import**, save to the configured path, and reload
   the database or restart the client. Updating the normal Cards import alone is insufficient.
5. Recheck the actual spawned object. If the data is current and the entry exists, trace the
   client token-display mapping instead of assuming every blank display is stale data.

Some state entries intentionally have no rules text. Validate the expected name, type, and art
against that entry. Do not add hardcoded image URLs or copied display prose to engine/relay code
to compensate for a stale external database.

### Presentation-bearing surfaces

Every authored display node needs stable identity and an explicit `OracleLines` or `Fallback`
decision.

| Surface | Presentation field | Primary consumers |
|---|---|---|
| Identified ability | `presentation` | Context-menu labels, ability target prompts, trigger choices, and synthetic stack cards |
| Modal option | `presentation` | Mode picker, mode target prompt, and chosen-mode annotation |
| Cast-cost group | `presentation` | Cast-cost prompt |
| Cast-cost option | `presentation` | Cast-cost option label and chosen-cost annotation |
| Resolution branch | `presentation` | Resolution option label |
| Heterogeneous search slot | `presentation` | Search-choice label |
| Restricted-mana rule | `presentation` | Mana restriction explanation |

For a modal spell, map each mode separately. Map cast-cost groups and options independently as
well. A permanent's battlefield abilities are mapped on their identified ability entries, not by
treating all printed text as spell presentation.

### Map Oracle lines

`OracleLines([..])` contains one-based indices into the selected external face's normalized Oracle
text. Normalization converts line endings, trims each line, and removes blank lines. A bullet that
occupies one Scryfall Oracle line is one addressable line even if it contains several sentences.

Before recording a mapping:

1. Select the exact external face.
2. Split `oracle_text` on line breaks, remove blank lines, and number the remaining lines from one.
3. Select every line needed to present that node, in ascending order.
4. Check the mapping again after changing mechanics; a valid line number can still point at the
   wrong ability.

Example normalized text:

```text
1  Kicker {2} (...)
2  Search your library ...
3  You gain 2 life.
```

- The kicker group and option may both use `OracleLines([1])`.
- The physical spell itself needs no mapping; its face name remains the source identity.
- Reusing one Oracle line for multiple mechanical nodes is valid when one printed ability is
  implemented by multiple typed nodes.

`OracleLines` is all-or-fallback. If any selected line, face identity, fingerprint, or external
database face text is invalid, clients display the supplied deterministic fallback rather than a partial
selection.

### Use `Fallback` deliberately

Use `Fallback` only after checking current Oracle text and determining that the node has no exact
external line mapping. Typical cases are:

- a synthetic no-op branch used internally by `FirstApplicable`;
- a runtime-only choice with no separately printed wording; or
- a child node representing only a fragment of a larger printed instruction where the whole line
  would mislabel the child.

Do not use `Fallback` merely to avoid verifying line numbers. It is an explicit presentation
decision, not a TODO marker. Add a short RON comment when the reason is not obvious. A mechanical
implementation gap belongs in `partial-cards.tsv`; `Fallback` does not record partial support.

Record every intentional authored `Fallback` in [presentation-exceptions.tsv](presentation-exceptions.tsv)
with four tab-separated fields: card ID, face ID, node path, and a specific reason. Copy the node
path from the audit output; ability and choice IDs identify array entries. Token templates use
intentional fallback because they have no normal-card Oracle fingerprint. Their artwork matching
keeps stable identity markers separate from readable menu and stack labels.

### Inspect and check presentation

From the repository root, use the pinned, SHA-verified Oracle snapshot:

```powershell
./scripts/gen-cards.ps1 --audit-presentation --inspect-card Forest
./scripts/gen-cards.ps1 --audit-presentation --check
```

These commands are read-only. Inspection accepts a card name or ID and prints each face's
normalized numbered Oracle lines, fingerprint, and node mappings. The audit distinguishes mapped
nodes, intentional exceptions with reasons, and unresolved fallback nodes. For simple abilities,
an exact match between a complete generated description and an Oracle line produces an advisory
suggestion; the tool never applies mappings. Review the mechanics and current Oracle/rulings before
accepting a suggestion. Index validation cannot prove that a line describes the correct ability.

Ordinary `gen-cards --check` also runs the audit. It rejects unresolved fallback, missing source
faces, empty/out-of-range/duplicate/descending line indices, and malformed, duplicate, or stale
exceptions. After changing a node, update or remove its exception and rerun the check.

Simple fallback descriptions include costs, effects, and supported timing restrictions. If any
piece cannot be described completely, the whole ability keeps its stable generic label.

Client resolution remains all-or-fallback. The `cockatrice.ruled.presentation` logging category
explains missing cards/faces, absent or invalid fingerprints, expected/loaded hash mismatches, and
invalid line selections. Identical warnings are emitted once per resolver, with a bounded cache.
Intentional fallback with no Oracle line selection is silent. Use these diagnostics to repair
metadata or the external card database; display text never determines legality.

### Keep target prompts narrow

`TargetGroupDef.prompt` is the narrow exception to the no-prose rule. It provides short,
effect-specific click guidance such as `Choose target creature you control`. It is combined with
the spell's face name, the selected mode's presentation, or the ability presentation and does not
replace that source context.

- Physical-spell target prompts use the engine-authored face name; a selected mode can replace that
  source context with its own presentation.
- Ability presentation supplies context-menu and target-prompt wording for activated and triggered
  abilities. Clients must not reconstruct it from mechanics or card names.
- Mode, cast-cost, branch, search-slot, and restriction presentations label their own choices.
  Parent presentation does not make child mappings complete.
- Do not add freeform cast-cost prompts or option labels.

### Complete presentation shapes

Targeted spell:

```ron
(
  id: "example_spell",
  name: "Example Spell",
  face_id: "example_spell",
  mana_cost: "{1}{U}",
  types: ["Instant"],
  spell_effect: [Tap(subject: Chosen((kind: Creature)))],
  targeting: Some((groups: [(
    min: 1,
    max: 1,
    prompt: "Choose target creature",
    effect_indices: [0],
  )])),
)
```

This prompt starts with `Choose a target for “Example Spell”` and appends the authored click
guidance.

Identified ability:

```ron
activated_abilities: [(
  ability_id: "activated_01",
  presentation: OracleLines([2]),
  costs: [Mana("{1}"), Tap],
  effect: [Draw(count: 1)],
)],
```

Nested granted, delayed, or reflexive abilities also need stable `ability_id` and presentation
metadata. Their IDs are scoped within their owning path but must remain stable.

Modal spell with cast-cost choices:

```ron
cast_cost_groups: [(
  group_id: "spree",
  presentation: OracleLines([1]),
  min: 1,
  max: 2,
  options: [
    Mana(option_id: "first_cost", presentation: OracleLines([2]), kind: AdditionalPayment, cost: "{1}"),
    Mana(option_id: "second_cost", presentation: OracleLines([3]), kind: AdditionalPayment, cost: "{2}"),
  ],
)],
modal_spell: (
  min_modes: 1,
  max_modes: 2,
  modes: [
    (mode_id: "first", presentation: OracleLines([2]), effects: [Draw(count: 1)]),
    (mode_id: "second", presentation: OracleLines([3]), effects: [GainLife(amount: 3)]),
  ],
),
```

Object-paid cast costs use the same group/option identity. `TapPermanents` accepts a
generation-bound cohort and may impose an aggregate current-power minimum; `SacrificePermanent`
accepts exactly one matching permanent. The semantic `kind` is recorded on the committed cast-cost
receipt so triggers can distinguish Teamwork, Kicker, and ordinary additional payments without
matching labels. Copies retain the announced receipt but never pay the object cost or emit its tap
or sacrifice actions again.

```ron
options: [
  TapPermanents(
    option_id: "teamwork_4",
    presentation: OracleLines([1]),
    kind: Teamwork,
    constraint: AggregateMinimum(minimum: 4, contribution: CurrentPower),
    filter: (kind: Creature, controller: You),
  ),
  SacrificePermanent(
    option_id: "sacrifice_kicker",
    presentation: OracleLines([2]),
    kind: Kicker,
    filter: (kind: Creature, controller: You),
  ),
],
```

Use `ConditionalCastCost` for a resolution instruction that applies only when a named cast-cost
option was announced. A target whose legal set expands under that option also needs
`cast_cost_expansion`: the target group's normal filter remains on the effect, while
`without_cost` is the narrower filter legal without paying. For modal spells that allow every
mode only after paying one option, set `all_modes_cast_cost` on `modal_spell`. These links are
validated by stable authored IDs and published to the client as engine-authored legality.

```ron
ConditionalCastCost(
  condition: (group_id: "teamwork", option_id: "teamwork_2", expected_selected: true),
  effect: GainLife(amount: 3),
)

cast_cost_expansion: Some((
  condition: (group_id: "teamwork", option_id: "teamwork_2", expected_selected: true),
  without_cost: (kind: Creature, max_mana_value: Some(3)),
))

all_modes_cast_cost: Some((group_id: "teamwork", option_id: "teamwork_4")),
```

Resolution branch:

```ron
ChooseResolutionBranch(
  optional: true,
  branches: [(
    branch_id: "sacrifice_a_land",
    presentation: OracleLines([2]),
    cost: SacrificePermanent(filter: (kind: AnyPermanent, permanent_types: [Land])),
    effects: [Draw(count: 1)],
  )],
)
```

If line 2 describes only the parent ability and would be misleading as the branch label, use
`Fallback` with a comment explaining the deliberate split.

### Bind "when you do" to a successful counter placement

Use a receipt-gated reflexive trigger when the printed trigger depends on the immediately
preceding counter instruction actually placing a counter. The receipt is private engine state,
matches the exact object generation, and is unavailable after any intervening instruction.

```ron
effect: [
  PutCounters(counter: Quest, count: 1, subject: Source),
  CreateReflexiveTrigger(
    when: Some(CountersPlaced(counter: Quest, object: Source)),
    ability: (
      ability_id: "reflexive_01",
      presentation: OracleLines([2]),
      intervening_if: Some(SourceCounterCount(counter: Quest, min: Some(4))),
      effect: [GainLife(amount: 1)],
    ),
  ),
],
```

The registry requires the same counter kind on an immediately preceding `PutCounters`. Omit
`when` for a reflexive trigger created unconditionally by a successful paid branch. Put an
`intervening_if` condition on the nested ability only for an actual CR 603.4 clause; the engine
checks it both before staging the reflexive trigger and again when that trigger resolves.

## 6. Add a generic primitive or keyword

Only add vocabulary after confirming existing data cannot express the behavior.

Use `Discard(quantity: Exact(2))` for a fixed untargeted discard and
`Discard(quantity: All)` for an entire hand, as on Stoke Genius. Whole-hand discard selects
the complete hand automatically while preserving discard replacements and madness. Follow it
with `Draw(count: 2)` for a mandatory discard-then-draw sequence, including an empty hand.

Use `Discard(quantity: UnlessOne(count: 2, filter: (card_type: Some(Creature))))`
after `Draw(count: 3)` for Winternight Stories. The same shape with `Artifact` expresses
Thirst for Knowledge. The count must be at least two and the printed-card filter must be valid.
The player selects either one matching card or the ordinary count, clamped to the remaining
hand. This uses the private hand picker and discard continuation, not `ChooseResolutionBranch`.
The one-card alternative is a resolution-time cost (CR 118.12a); Library of Leng cannot replace
it, while madness still applies. The ordinary discard instruction retains effect replacements.

Use `LookChooseToHand(count: 2, min: 1, max: 1, reveal: false, bottom_order: Chosen)`
for Sleight of Hand's mandatory private selection. Omit `filter` to allow any card, or use
`filter: Some((...))` for a printed-card predicate. Bounds clamp to available matches; these
cards are put into hand, not drawn. Existing omitted bounds and reveal fields retain optional
selection of one revealed card. Flow State uses costless `FirstApplicable` branches with an
`AllOf` condition for the two graveyard types, evaluated once as the instruction resolves.

1. Name two real cards or two mechanics supported by the proposed shape.
2. Put the variant in the appropriate `tricerules-cards/src/primitives/` module.
3. Add registry validation for authoring constraints and reject ambiguous or invalid shapes.
4. Implement behavior in the matching engine domain rather than a card-specific dispatch path.
5. Add focused primitive/registry coverage plus happy and illegal scenarios for real card consumers.

Keywords carry their CR citation and behavior in the appropriate engine subsystem. Battlefield
keywords already cross the wire as strings; do not add protobuf solely to publish a new keyword.
If a primitive changes protocol, relay, or UI contracts for a separate reason, follow the full
cross-component workflow and rules interaction checklist.

Attachment-scoped combat rules belong in the typed `restriction` field of `AttachedModifier`, so
they share validation, legality, and public rules annotations with self- and creature-scope
restrictions. Pacifism and Meltstrider's Resolve are representative forms:

```ron
AttachedModifier(restriction: (cant_attack: true, cant_block: true))
AttachedModifier(delta_toughness: 2, restriction: (maximum_blockers: Some(1)))
```

Do not put combat restrictions on a conditioned `AttachedModifier`; conditions there are limited
to characteristic modifiers. Add a separate typed restriction primitive if a future mechanic
needs a conditional combat rule.

## 7. Add a custom Rust card

Use custom Rust only when the resolution algorithm itself is unique and cannot be described as
static `(effect_kind, parameters)` data.

1. Set `custom_effect: "<card_id>"` in RON; it is mutually exclusive with `spell_effect`.
2. Create `tricerules-core/src/custom/<card_id>.rs` outside `support/`.
3. Export `pub(crate) static EFFECT: &dyn CardEffect = &YourType;`.
4. Match the card definition ID, RON `custom_effect`, and file stem exactly. Registration is
   automatic.
5. Keep the implementation one-to-one with a card ID. Shared algorithms belong in a primitive.
6. Use the capability-narrowed `ResolutionCtx`; never give custom code `&mut GameState`.
7. Reuse `resolution_choice_required` and `SubmitResolutionChoice`; do not add per-card protobuf.
8. Cite the checked Oracle text and governing CR concepts in the implementation header.
9. Add happy and illegal scenario coverage for `begin`, every resumable choice, and completion.

## 8. Generate supported cards

Do not hand-author supported vanilla or french-vanilla creatures. From the repository root:

```powershell
./scripts/fetch-scryfall-bulk.ps1
./scripts/gen-cards.ps1 --dry-run
./scripts/gen-cards.ps1
```

Generated RON contains stable face/ability IDs and Oracle line references, never Oracle prose.
Refresh may replace only files carrying valid generator provenance. Oracle Tags are advisory and
cannot select mechanics, IDs, or presentation mappings. Review the dry run and generated diff;
never accept unrelated bulk churn.

### Extend the exact-recipe catalog

When a named repeated template has a demonstrated throughput benefit from generation and existing
typed primitives express the complete behavior, a justified recipe can be added to
[`recipes.rs`](../src/bin/gen_cards/recipes.rs). Keep recipes in typed Rust; do not add an external
or stringly rules DSL.

Each recipe requires:

- a stable `RecipeId` that names the semantic surface and operation;
- the narrowest applicable `RecipeSurface`;
- an exact matcher that emits typed `tricerules-cards` data;
- at least two distinct named positive calibration cards; and
- reviewed negative near-misses covering the closest optional, bounded, conditional, or
  additional-clause forms.

Every functional Oracle clause must match exactly one applicable recipe. Zero matches remain
unsupported, while multiple matches are an ambiguity error listing the matching recipe IDs. Do
not loosen normalization or accept a near-match merely to increase coverage.

After focused catalog tests pass, preview and deliberately include newly qualifying cards:

```powershell
./scripts/gen-cards.ps1 --dry-run --include-new
./scripts/gen-cards.ps1 --include-new
```

Review every new generated RON file and the presentation-fingerprint/checklist changes. An ordinary
generation run or `update-card-data.ps1 -Mode Refresh` updates provenance-owned files that are
already tracked; neither opts new candidates into the registry.

Use `gen-cards --check` against the pinned SHA-verified snapshot to detect drift without writing.
Both PowerShell generator wrappers preserve the child's exit code. For the combined read-only
generator and checklist check, use the workflow entry point from the repository root:

```powershell
./scripts/update-card-data.ps1 -Mode Check
```

To rank unsupported Oracle clauses without changing generated data, write an explicit candidate
report destination:

```powershell
./scripts/gen-cards.ps1 --candidate-report build/candidates.json
./scripts/gen-cards.ps1 --candidate-report build/cube-candidates.json --target-names cube-names.txt
```

The optional target file contains one exact whole-card or face name per nonblank line. Unknown or
ambiguous names fail the command without writing the report. The stable JSON clusters normalized
clauses by descending printing-independent card count, retains face and source-routing context,
and keeps any optional Oracle Tags summary advisory and separate from the clause signatures.

### Bounded typed recipe families

`FixedSourceCreatureDamage` in `recipes.rs` is the first private parameterized family. It owns
only the complete `<exact face rules name> deals N damage to target creature.` clause. It uses
the existing `DamageTarget`, creature filter and modal targeting builder; the resolving spell
remains the damage source. Exact formatted comparison rejects alternative numeric spellings,
overflow, variables, other source names, qualifiers, optionality and appended instructions.
Other damage grammars retain their own recipes.

The reviewed catalog instances are now **spell 3**, **spell 4**, **spell 5**, **modal 3**, and
**modal 4**. The #449 structural pilot admitted only spell 4, modal 3, and modal 4 and
deliberately left the other spell-surface amounts unsupported; #452 added the `Five` amount
variant plus the two reviewed spell instances (Three and Five) after complete-card preflight
of the pinned corpus. Each instance keeps its own recipe ID, report label and calibration
metadata. All instances participate in the ordinary exact-one matcher: identical emissions from
overlapping owners are still an ambiguity error, and one amount never implies another surface.
The previous three bespoke recognizers were removed together. Modal assembly headers, bounds,
ordered recipe sets, stable mode IDs and presentation mappings are unchanged.

To add a future family, implement a private typed parameter domain and recognizer/emitter, then
register explicitly reviewed instances through ordinary `Recipe` entries and their existing
function-pointer matcher. No branch in the top-level card parser is needed. Keep per-instance
source calibrations and stable metadata, but share the grammar, emitter and parameterized tests.
Review observed corpus values before extending a parameter enum or admitting another surface;
do not accept every integer just because the runtime primitive can store it. Run the complete
definition, semantic evidence and admission workflow separately from clause recognition. A
recognized modal bullet never authorizes a new aggregate or card combination.

The #449 pilot scanned the pinned SHA-verified corpus
(`9611b5d93b20478a0ee46bae8b20a9eb39ee980f0ef4f5f6f6aaa8f7ab010ab2`). Exact complete lines/bullets
observed amounts 1, 2, 3, 4, 5, 6, 7 and 13 on respectively 3, 6, 10, 21, 18, 4, 2 and 1
printing-independent identities. Those observations include unsupported cards and are not
admission evidence. This structural pilot deliberately retains existing supported combinations:
**zero newly recognized clauses, zero newly eligible complete cards, zero admitted identities**.
Full dependency inventories compared all 38,626 identities with no classification, observation,
recipe-label or eligibility changes. Canonical checks against the pre-extraction output proved
byte equality for all 2,425 generated definitions and the fingerprint catalog both after the
builder extraction and after enabling the family. No metadata migration is needed.

Issue #452 used the same extraction as a throughput pilot and added two reviewed spell instances:

- **spell 3** completes Ragefire and Repulsor Rays.
- **spell 5** completes Scorching Shot (pinned-Standard), Command the Storm, Concentrated Fire,
  Direct Hit, and Engulfing Eruption.

The complete-card preflight excluded the observed amount-2 (Breath of Fire) and amount-7 (Fiery
Finish) spell singletons as unreviewed, and left the four already-eligible spell-4 identities
(Bathe in Dragonfire, Electrify, Explosive Shot, Flame Slash) outside the retained cohort to keep
the pilot bounded. No modal damage instance was added: the modal identities that print a
`deals N damage to target creature` bullet with an unadmitted amount each also print at least one
unsupported bullet, so a new parameter would not have completed a card. Measured marginal work
for the two instances was one added enum variant in total plus one catalog entry each — two
positive calibrations and four reviewed negative near-misses apiece — plus the shared
characterization/negative matrix; the seven-card cohort then cost one pinned
`--dry-run --include-new` preview, review of seven generated definitions, one registry matrix,
one shared semantic scenario matrix, and seven conformance baseline rows (seven `cast`
`exercised`, no new abilities). The three-damage and five-damage instances remain exact-one
matched. This is a measured cohort of **seven complete Oracle identities**, not a projection.

Measured duplication: three authored grammar comparisons and three typed damage constructions
become one of each, with three small catalog adapters. The two repeated modal wrappers become
one. Existing calibration rows stay intact; one shared characterization matrix covers all three
instances, and one shared negative matrix covers both surfaces. Test code grows to cover the
failure boundary, full RON equality and Iroh's targeted mode; this is not a claim of fewer total
lines or new card coverage. Session inventories and comparison evidence stay under `build/`,
not in a persistent campaign tracker.

Source review on 2026-09-19 fetched exact-name Scryfall records and `rulings_uri` for Bombard,
Iroh's Demonstration, Abrade and Bathe in Dragonfire; all returned no rulings. #452 additionally
fetched the same data for Ragefire, Repulsor Rays, Scorching Shot, Command the Storm, Concentrated
Fire, Direct Hit and Engulfing Eruption; all returned no rulings. The
[official rules page](https://magic.wizards.com/en/rules) linked the 2026-09-25 rules text.
CR 120.2b (damage source), 115.1a (spell targets), 608.2b (target revalidation), and 700.2a/c
(modes and their targets) govern the preserved semantics.

Interaction audit: compile-time source/face matching emits existing typed definitions; runtime
state authority, physical source identity/generations, target legality, resolution order and
damage processing are unchanged. Existing engine scenarios remain, with explicit targeted-mode
acceptance/rejection and bounded completion added for Iroh. The #452 cohort adds exact-damage,
illegal-target-class and CR 608.2b fizzle scenarios using the same command boundary. No new
player assumptions, fields, visibility flows or public offers are introduced. Protobuf, relay,
Qt, freeform and manual GUI acceptance are N/A because their contracts and generated inputs are
unchanged. Generator tests, feature-enabled Clippy, conformance and the full Rust/card-data gate
remain required.

### Independent modal composition pilot

Issue #450 keeps `RecipeSurface`, `RecipeEmission`, and the spell/trigger wrappers as distinct
authoring contexts, but shares private typed construction for controller draw, fixed life gain,
and controller token creation. `IndependentModalComposition` validates the already-emitted
`ModalDef`/`ModeDef` values rather than introducing another runtime IR. Its contract requires:

- one contiguous half-open Oracle-line span per bullet, in printed order, with stable sequential
  mode IDs and an exact presentation line;
- valid selection bounds and at least two distinct exact mode recipes;
- a successfully compiled existing `TargetSchema`, with no target groups or linked cast cost for
  this pilot;
- only controller `Draw(Fixed(1..=4))`, `GainLife(Fixed(2..=6))`, or controller
  `CreateTokens(Fixed(n))` effects, in their emitted order; and
- every referenced token ID to exist in the embedded token registry.

The reviewed initial clause signatures are the existing one-card draw, Food/Human token modes and
exact registered-token mode recipes, plus standalone controller draws of two through four cards
and standalone controller life gains of two through six. This is not an amount/surface
cross-product: each clause still needs one exact `ModalMode` recipe, and complete-card admission
still validates the whole header, every bullet and every nested payload. Conditional or shared
targets, result references, variable/budgeted or repeatable selection, additional costs (including
Teamwork and Spree), targeted-player effects, unsupported riders, duplicate modes and unknown
tokens remain rejected.

Ordinary `Choose one`, two-bullet `Choose one or both`, and reviewed three/four-bullet `Choose
two` assemblies, plus the exact creature-ETB wrapper, may use the pilot. Teamwork and every other
wrapper stay on their specialized path. Pilot admission and `reviewed_modal_mode_pair` are an
exclusive-or: overlap fails closed. The complete pinned-corpus comparison retained identical
eligibility, failure and production recipe-label classifications for all **38,626** identities
and all **4,887** pinned-Standard identities. It added diagnostic observations to **43** identities,
including six pinned-Standard identities (Demonic Pact, Splatter Technique, Apothecary Stomper,
Charming Prince, Witherbloom Charm and Wardens of the Cycle), but admitted **zero** new identities.
Exhibition Magician and A-Exhibition Magician are explicit missing-token candidates: their exact
Citizen bullet is observed, but `citizen_gw_1_1` is not registered, so both remain ineligible.

No legacy mode-set allowance was replaced because no currently allowed aggregate consists wholly
of the pilot effect set. Every existing `reviewed_modal_mode_pair` entry remains the compatibility
boundary for its prior composition; the new validator only owns disjoint, independently supported
combinations. Synthetic spell and creature-ETB tests prove a new combination needs no bespoke
whole-card pair matcher, while registered Divination, Elvish Visionary and Pawpatch Formation
tests preserve their exact effects, recipients, trigger/modal wrapper, stable IDs and Oracle
presentation mappings.

Issue #452's bounded corpus preflight found **zero fully supported independent-modal candidates**
in the same pinned snapshot. Only four modal identities print bullets that all fall inside the
pilot's draw/life/token grammar, and all four are unsupported token-copy effects (Ember Island
Production, Mirage Mockery, One Dozen Eyes, Saheeli's Artistry). Every other near-miss identity
pairs a supported draw/life/token bullet with at least one unsupported bullet or wrapper (for
example Demonic Pact, Splatter Technique, Apothecary Stomper, Charming Prince, Witherbloom Charm,
and Wardens of the Cycle). The pilot therefore reported the route as corpus-exhausted and did not
widen the effect grammar, add a modal mode, or admit an identity. Future modal admissions need a
new reviewed mode recipe or new composition support first; the empty result says nothing about
the validator's correctness, only that the pinned corpus holds no complete card it can own.

Source review on 2026-09-19 fetched exact Scryfall records and each `rulings_uri` for Divination,
Elvish Visionary, Pawpatch Formation, Exhibition Magician and A-Exhibition Magician. Only Pawpatch
returned rulings, whose Food notes were reviewed. The official 2026-06-19 Comprehensive Rules
govern through CR 700.2a-b/d (modal selection and nonrepeatability), 608.2c (printed resolution
order), 121.1-2 (draws), 603.6a (entry triggers), and 111.10a-b (Treasure and Food definitions).

Interaction audit: tricerules remains the sole runtime authority; the generator only validates
and emits existing typed effects. Card/face/ability/mode identities and source/controller roles are
preserved. No zone-change identity, timing, target, replacement, visibility, protocol, relay, Qt,
or freeform contract changes. C++ and manual GUI acceptance are N/A. Generator tests,
feature-enabled Clippy, semantic fixture/registered-card regressions and the full Rust/card-data
gate remain required.

### Scaffold source-backed authoring

For complete-card dependencies and projected individual/pair unlocks, use the separate
[versioned dependency report](DEPENDENCY-REPORT.md). It inventories the full pinned corpus and
its Standard subset, distinguishes generator eligibility from registered support, and requires
reviewed evidence before counting a whole identity as unlocked. The clause report above remains
unchanged.

For an unsupported card chosen from a candidate report, scaffold the clerical source fields from
the same pinned, SHA-verified Oracle bulk input:

```powershell
./scripts/gen-cards.ps1 --scaffold-card "Black Lotus"
./scripts/gen-cards.ps1 --scaffold-batch selected-cards.txt --scaffold-out-dir build/scaffolds
```

Single-card mode writes the incomplete scaffold to stdout by default. Batch stdout is a stable JSON
bundle containing the manifest and scaffold contents. `--scaffold-out-dir` instead writes a
`manifest.json` plus deterministic `*.ron.scaffold` files. The command refuses output anywhere
under `tricerules-cards/data/` and refuses every overwrite; the extra `.scaffold` suffix and the
unresolved sentinel fields prevent accidental registry loading. Reprints collapse by Oracle ID.
Unknown, ambiguous, colliding, unsupported-layout, and already-implemented selections are recorded
separately in the batch manifest. A single refused selection fails the command. Use
`--inspect-existing` only to report an already-implemented match; it never emits replacement RON or
renumbers the authored stable IDs.

Scaffolds contain source identity, faces, printed characteristics, normalized numbered Oracle lines,
candidate ability/choice IDs, presentation review prompts, and the per-card research checklist. The
IDs are deliberately marked for author confirmation: Oracle line boundaries are not trusted
mechanical boundaries. The tool never selects effects, costs, targets, conditions, or legality.

To turn a reviewed scaffold into authored RON:

1. Fetch and review the exact card, its `rulings_uri`, and the relevant current CR sections.
2. Choose the lowest complete implementation tier and hand-author every mechanical field.
3. Confirm, merge, split, rename, or remove candidate IDs, then keep accepted IDs stable.
4. Resolve every presentation mapping and prerequisite or record intentional partial support.
5. Remove both unresolved sentinels, rename the file to `.ron` under `data/`, and complete all
   checks in sections 9 and 10. A scaffold is never evidence that the card is implemented.

### Reviewed direct-RON path

Choose the authoring route in this order:

1. Reuse a shipped exact recipe when it already covers the complete card.
2. Otherwise use reviewed handwritten RON for supported cards, including repeated templates.
   Two matching cards alone do not justify a new parser or recipe family.
3. Develop a new typed recipe only with a concrete expected throughput benefit for a named cohort.
   Actual generator changes retain positive calibrations and negative near-misses; handwritten
   additions do not need matcher tests. Preserve existing generated cards and their checks.
4. File a scoped runtime blocker when any required cost, timing, choice, target, effect,
   presentation, or composition semantics are unsupported. Do not partially admit the card.

For the direct-RON route, keep the author-supplied draft outside `data/` and create a version-1 JSON
review map. Select one pinned source identity with either `oracle_id` or `exact_name`. Cover every
normalized Oracle line exactly once with either `typed_paths` (JSON pointers into the validated
runtime-shaped definition) or a nonblank `unresolved_reason`. List explicit primitive references,
every referenced token ID, and planned semantic fixtures. The tool does not infer any of these.

```powershell
./scripts/gen-cards.ps1 `
  --review-draft build/direct-ron/my_card.ron `
  --review-map build/direct-ron/my_card.review.json `
  --review-out build/direct-ron/my_card.packet.json
```

The command is an offline, separate mode. It verifies the pinned bulk SHA, resolves names to one
Oracle ID, validates source layout and face identity, rejects scaffold sentinels, loads the draft
through the production registry validators and shipped token namespace, rejects registry ID/name
collisions, verifies complete nonoverlapping span coverage and typed paths, and refuses drafts or
outputs under embedded `data/` plus all output overwrites. It records source/rulings links, per-face
Oracle fingerprints, presentation values, tokens, primitive references, semantic-fixture plans,
and structurally ranked nearby registered definitions. Nearby definitions are inspect-only; names,
prose similarity, and rank never establish support.

Packets always set `mechanical_equivalence_proven` to `false`. A structurally valid, fully mapped
but semantically wrong draft can therefore produce review evidence but can never be described as
mechanically proven. `promotion_ready_for_human_review` only means there are no explicitly
unresolved spans and the map records complete-definition review confirmation. The author and an
independent reviewer still establish Oracle/rulings equivalence and execute the planned semantic
evidence.

The checked-in Divination and Elvish Visionary maps demonstrate inspect-only review of existing
definitions without permitting replacement:

```powershell
./scripts/gen-cards.ps1 --review-existing `
  --review-draft tricerules/tricerules-cards/data/divination.ron `
  --review-map tricerules/tricerules-cards/authoring/review-maps/divination.json `
  --review-out build/direct-ron/divination.packet.json

./scripts/gen-cards.ps1 --review-existing `
  --review-draft tricerules/tricerules-cards/data/elvish_visionary.ron `
  --review-map tricerules/tricerules-cards/authoring/review-maps/elvish_visionary.json `
  --review-out build/direct-ron/elvish_visionary.packet.json
```

Issue #452 promoted the first identities through this path: `stand_up_for_yourself` (Stand Up for
Yourself, destroy target creature with power 3 or greater) and `oracles_restoration` (Oracle's
Restoration, pump/draw/life in printed order). Their maps are checked in beside the inspect-only
examples and can be rerun with `--review-existing` after any change:

```powershell
./scripts/gen-cards.ps1 --review-existing `
  --review-draft tricerules/tricerules-cards/data/stand_up_for_yourself.ron `
  --review-map tricerules/tricerules-cards/authoring/review-maps/stand_up_for_yourself.json `
  --review-out build/direct-ron/stand_up_for_yourself.packet.json

./scripts/gen-cards.ps1 --review-existing `
  --review-draft tricerules/tricerules-cards/data/oracles_restoration.ron `
  --review-map tricerules/tricerules-cards/authoring/review-maps/oracles_restoration.json `
  --review-out build/direct-ron/oracles_restoration.packet.json
```

Use `oracle_id` in the map whenever a whole-card name is ambiguous across oracle IDs: the `sos`
Oracle's Restoration and its `asos` art-series namesake normalize to the same name and an
`exact_name` selection is refused. Promotion also added explicit narrow target prompts
(`Choose target creature with power 3 or greater`, `Choose target creature you control`), a
registry/presentation matrix, semantic scenarios for accepted and illegal paths plus CR 608.2b
fizzling, and conformance baseline rows (`oracles_restoration` exercised; `stand_up_for_yourself`
recorded as `unsupported: target group 0 needs a richer fixture` because the shared conformance
fixture has no power-three-or-greater creature, while the dedicated scenario exercises it).
Runner-up Nimble Thopterist was excluded because `thopter_c_1_1_flying` is not in the embedded
token registry.

After a draft and packet pass review, promotion remains an explicit authored-card change: copy the
reviewed draft to its canonical handwritten `.ron` path under `data/`; recheck stable card, face,
ability, mode, and choice IDs; refresh and review Oracle fingerprints and `CARDS.md`; add the
complete-definition registry assertions, applicable happy/illegal semantic scenarios, presentation
and conformance coverage, and checklist evidence; then run the full Rust/card-data gate. Never
promote an unresolved packet, overwrite an existing identity, add generator provenance to direct
RON, or let Refresh modify a handwritten card.

### Authoring throughput and the dependency-driven blocker lane

The #452 bounded pilot (corpus `9611b5d9...`, starting revision `82960669a`, one writer with an
independent read-only review) established the following as the default campaign workflow:

1. **Preflight the complete card before authoring.** Use the versioned dependency report,
   `--candidate-report`, and targeted pinned-corpus scans to enumerate every face, clause, cost,
   target, choice, token, and presentation prerequisite. Classify each remaining requirement
   against the actual emitter/validator/consumer/tests. A recognized clause, recipe name, or
   effect enum is never evidence of complete support.
2. **Prefer direct authoring for supported cards.** Reuse complete shipped recipes; otherwise use
   handwritten RON, even for shared templates. New recipes need a throughput justification. Any needed
   runtime, choice, target, or presentation contract becomes a scoped blocker. Do not partially
   admit a card to keep a batch moving, and do not widen a grammar because a route's candidate
   set came back empty.
3. **Reuse evidence; do not re-run it for measurement.** #447-#451 evidence (dependency
   inventory, semantic fixtures, family extraction, modal validator, review tool) was consumed
   as-is. Admission from a pinned cohort input uses `--dry-run --include-new` followed by
   `--include-new` and then canonical Refresh, which restores canonical provenance and the full
   fingerprint catalog. Measure selection/preflight, implementation, review, verification, and
   rework separately; never infer phase timing from commit spacing.
4. **Review a frozen patch while verification is serialized.** Snapshot the intended diff, hash
   it, and have an independent reviewer inspect that exact patch and its evidence read-only.
   Run the focused and full gates on the same frozen content, and return findings as rework
   before treating the evidence as final. Escalate review depth only for demonstrated semantic
   risk; keep engine/primitive changes on the deeper-review path.
5. **Select shared blockers by verified complete-card unlocks.** A blocker is an assignment
   candidate only when its deliverable is the last remaining requirement for specific named
   identities and its evidence is reviewed. Verify the runtime/protocol/UI impact of the
   deliverable before assigning it, give the blocking mechanism one owner, and return the
   unlocked identities to a single routine owner for complete-card preflight and admission.
   Do not implement unrelated blockers to grow a batch, and never count a blocker-only mapping
   as implemented coverage.

## 9. Track partial implementations

Record a genuine implementation gap as one `card_id<TAB>note` row in
[`partial-cards.tsv`](partial-cards.tsv). Do not put `partial`, checklist metadata, or other tracking
fields in rules RON; the runtime registry must not load project-management state.

Presentation `Fallback` is not automatically a partial implementation. Conversely, accurate
presentation metadata does not make missing mechanics complete.

## 10. Verify and review

Use red/green TDD for behavior changes: add the smallest focused regression, confirm its intended
failure, implement one coherent increment, and rerun it. Finish with the exact Rust and card-data
gates in [`docs/AGENT-VERIFICATION.md`](../../../docs/AGENT-VERIFICATION.md).

For card-data changes, refresh existing generated RON, presentation fingerprints, and the
validated checklist, review the resulting diff, then run final verification from the repository
root:

```powershell
./scripts/update-card-data.ps1 -Mode Refresh
./scripts/verify.ps1 -Side Rust -CardData
```

Use `-Side Both` when C++ contracts are affected. Refresh uses the existing pinned local input;
it does not fetch new data or enable `--include-new`. Check is non-mutating for tracked files.
The legacy checklist generator's `--check` validates names but still writes its output; do not
use it as a read-only drift check. Source overrides and retained failure evidence are described
in the verification guide.

Complete the ruled interaction checklist when adding or changing a substantive primitive,
protocol, relay, or client contract. Rust-only status is N/A for C++ testing only after confirming
that presentation transport, visibility, physical identity, and client behavior did not change.

### Reusable semantic evidence

Keep four contracts separate: primitive behavior, composition (for example, an ETB draw or a
selected modal effect), each card's reviewed semantic mapping, and unusual interaction regressions.
Registry shape checks alone do not execute effects; successful command admission alone does not
prove resolution. The conformance coverage classifications and reviewed baseline remain unchanged.

Use [`scenario/helpers/semantic.rs`](../../tricerules-core/tests/scenario/helpers/semantic.rs)
for deterministic main-phase setup, accepted commands, explicit object assertions, and bounded
completion. `complete` counts individual commands, including priority passes and supplied choice
answers. It consults the engine's blocking-choice state as well as the stack and pending cast.
Supply explicit, reviewed choice commands through its callback; it never guesses an answer.
Its result proves only completion. Check the expected state before reporting semantic evidence.
`exercise_draw` demonstrates that sequence for the three registered pilot consumers.

- **Exercised:** every listed card ran accepted commands, finished all relevant choices/effects,
  and passed independent state expectations. Require `require_exercised()` for each required case.
- **N/A:** name the inapplicable surface and reason (untargeted draw has no target-selection case).
  This does not count as exercised behavior. Cast timing/payment can still have illegal cases.
- **Fixture-blocked:** record the missing choice fixture or exhausted command bound. Do not skip
  the row, count it as support, or alter the conformance baseline to make the test pass.
  Rejected commands and incorrect assertions are failures, not fixture limitations.

A parameterized happy/illegal scenario satisfies a card's evidence only when that card actually
executes, all relevant clauses are asserted, and its inputs and expectations were independently
reviewed against its complete definition, Oracle, and rulings. Do not derive expected recipients,
counts, modes, or effects from the production recipe or registry. Capturing pre-command object IDs
and library order is fixture input, not deriving the expected mechanic. The registry
[`FaceExpectation`](../tests/common/mod.rs) checks reviewed face constants; effect mappings remain
explicit assertions beside it. Neither helper replaces complete-definition review.

Add a dedicated interaction scenario for new timing, replacement, trigger ordering, stale identity,
control/ownership, target revalidation, visibility, or resumable-choice behavior that the shared
fixture does not prove. Preserve existing unusual regressions. Internal state assertions do not
prove public offers or privacy; test those surfaces separately. Do not manufacture behavioral
tests for generated metadata, clerical edits, or illegal targeting of an untargeted effect.

The #448 pilot retains these behavioral contracts:

| Before | After | Retained or added evidence |
|---|---|---|
| `cast_divination_draws_two_cards` | Same test delegates to `exercise_draw(divination())` | Cast consumes one hand card and draws two; adds exact top-card identities, both players' hands/libraries, source graveyard/generation and completed resolution |
| `pawpatch_formation_draws_and_creates_a_food_token` | Same test delegates to `exercise_draw(pawpatch())` | One draw and one Food; adds stable selected mode, source identity, recipient and completion checks |
| Existing Visionary copy/populate/return interactions | Unchanged dedicated tests; shared pilot adds a normal cast | Real entry creates one trigger; no early draw; trigger resolution draws one and leaves the creature on the battlefield |
| #412 eight-card registry identity loop | Same reviewed rows through `FaceExpectation` | Name, face ID, cost, types, keywords and printed P/T; all existing mode/effect/target-schema assertions remain |

The two ported scenario bodies remove 75 lines of repeated setup/assertion code; their original
hand/draw and token assertions are subsumed by stronger exact-state checks. Shared helpers and
failure-detection tests add code overall. This is a reduction in repeated authoring, not evidence
of correctness by line count. Authors now (1) review the full source/definition, (2) supply typed
fixture inputs and independent expectations, (3) require each row to be exercised, and (4) add
applicable illegal/interaction coverage. They no longer duplicate deck/setup/cast/pass bookkeeping.

Pilot source review on 2026-09-19 fetched Scryfall's exact-name and `rulings_uri` endpoints for
Divination, Elvish Visionary and Pawpatch Formation. The first two had no rulings; Pawpatch's Food
rulings were reviewed. The [official rules page](https://magic.wizards.com/en/rules) then linked
the 2026-09-25 text: CR 121.1 (draw), 603.6a (entry triggers), 700.2a (mode announcement),
608.2c (instruction order), and 111.10b (Food) govern these unchanged expectations.

Interaction audit: authority remains real engine commands; source/face/object/generation and
owner/controller are asserted; spell, ETB and modal timing remain distinct; applicable rejected
casts, activations, targets, mode indices and unauthorized choices have explicit failure tests.
Visibility/public-offer proof is N/A for this internal fixture extraction, with existing tests
retained. Runtime, protobuf, relay, Qt and freeform contracts are unchanged, so C++/GUI gates are
N/A. Focused fixture/registry tests, existing conformance, generator tests and full Rust/card-data
gates remain required. No new rules capability or card identity is introduced.

### Batch evidence and review discipline

Missing generator recognition is not a runtime blocker. Classify ready cards, unassessed cards,
generator limitations and genuine runtime blockers separately. Routine selection may be tracked
by the single active batch and generated audit without an issue. Deferred blockers require explicit
issues with per-identity evidence; keep one primary owner and link capability dependencies.
Update partially delivered issue
bodies, not only comments; retain useful research while removing obsolete parser deliverables.
Do not build recipes that admit no complete cards merely because an old issue asks for them.
Campaign instructions set batch-size targets; reduce scope for semantic risk. Before editing a
routine batch, state a short semantic preflight in the task: selected Oracle identities and complete
readiness, source/rulings reviewed, reused primitives and important semantic distinctions, tests
that distinguish plausible wrong implementations, and exclusions. This replaces a mandatory routine
issue, not source research or complete-card review. No separate plan file or approval is required.
Use decision-complete issues when work needs design decisions (new primitives, complex/ambiguous
compositions or cross-component changes) and for deferred blockers. Reuse existing inventories;
do not create retrospective issues for completed routine batches. Capability links and historical
delivered-card mentions are not duplicate ownership. Lack of an issue is not proof of blockage.

`scripts/check-card-evidence.ps1` validates all checked-in review maps against canonical handwritten
definitions and the pinned source. Map filenames equal the canonical RON filename stem.
The checker uses directory-mode `--review-existing` to load and deduplicate the corpus once for
the entire batch, with packets written outside data. Checked-in maps must be fully resolved and
confirmed; review output remains evidence, never proof of mechanical equivalence.
Semantic references use `scenario module::test` for tricerules-core or `integration_target::test`
for tricerules-cards. The gate resolves exact Cargo identifiers and rejects missing or ignored
tests. Canonical CardData Check includes it. Listing is not proof of execution: the full Rust suite
must pass on the same content, and review must confirm the cases exercise the named cards and
clauses. Draft packets may still describe planned fixtures before tests exist.

Use default effort for routine implementation and independent read-only review; escalate for a
specific unresolved correctness risk. Run formatting and focused lint before freezing the patch.
Resolve blocking defects and missing required evidence before the final full gate. Optional polish
may be deferred without another review cycle. Changes after a passing gate require affected
reverification; avoid optional changes after that gate. Reuse unchanged passing evidence.
Use focused/package checks during iteration, then the full affected-side gate on stable content.
Do not routinely run the full suite before review and again inside the final gate. Broaden testing
early only when failures, changed contracts or concrete unresolved risks justify it. Keep all
required final checks; a later delivery request alone does not invalidate passing evidence.

The root owns all command execution that can write artifacts, including tests, builds, Cargo,
formatting, verification, generators, metadata checks and review packets. Independent reviewers
inspect the frozen patch and existing evidence using the workflow skill's
[reviewer template](../../../.agents/skills/cockatrice-workflow/reviewer-template.md).
They request missing commands from the root rather than rerunning gates themselves. Independence
means independent semantic judgment; it does not require duplicate test execution.

Request follow-up review when a fix changes mechanics, targets, costs, choices, identity, timing,
visibility, or semantic assertions needed to close a required evidence gap, or otherwise invalidates
the prior verdict. Supply the exact delta and preserve prior review artifacts. The follow-up reviews
that delta and affected interactions, not the entire batch by default.

Do not routinely request follow-up review for a successful planned final gate, formatting,
clerical corrections, or evidence wording narrowed to match already-reviewed tests. The root
checks those changes and runs any affected validation. A wording change that hides missing required
behavior is not clerical: fix the evidence gap and seek material-delta review. Defer optional
assertions or polish rather than creating another edit/review/gate cycle; if an optional change is
made, assess its actual semantic impact and reverify accordingly. These rules do not waive required
coverage or permit delivery with outstanding correctness defects.

Extend shared semantic fixtures incrementally for repeated setup and assertions demonstrated by
the selected batch. Preserve independent expectations, actual-card execution, applicable illegal
paths and dedicated interaction regressions. Do not introduce a generic test DSL or derive expected
behavior from production RON. Keep routine authoring separate from new engine primitives.

### Completion checklist

- [ ] Exact Oracle data and `rulings_uri` were fetched; relevant official CR text was verified.
- [ ] The lowest complete implementation tier was used.
- [ ] Whole-card, face, ability, mode, and choice identities are correct and stable.
- [ ] Mana cost, type line, faces, and printed numeric characteristics match the source.
- [ ] Mechanics are typed and engine-authoritative; no legality is encoded in presentation.
- [ ] Every ability and choice-bearing child has stable identity and presentation.
- [ ] Every `OracleLines` mapping matches the correct face's normalized current lines.
- [ ] Every non-obvious `Fallback` has a deliberate reason.
- [ ] New token/state-marker displays have the correct exact identity and external database entry; any hands-on acceptance is recorded separately.
- [ ] Target prompts contain only short, effect-specific click guidance.
- [ ] RON contains no copied Oracle display prose or freeform choice labels.
- [ ] Happy and illegal scenarios cover the implemented mechanics and relevant prompt/choice path.
- [ ] Genuine deferrals are recorded in `partial-cards.tsv`, not runtime RON.
- [ ] `CARDS.md`, generator checks, Rust gates, and `git diff --check` pass as applicable.
- [ ] The final report includes the governed MTG concepts and compliance or deferral note.

### Token copy sources

`CreateTokenCopies(count: 1, source: Chosen((kind: Creature, controller: You)))`
uses one chosen permanent target (Cackling Counterpart). `source: Source` is untargeted
and requires a battlefield ability source (Colorstorm Stallion). It uses the current
source generation or its last battlefield copy values after departure. Populate uses
the same owned snapshot and token-entry pipeline with a resolution-time choice.
Actual Transform and ModalDfc sources produce tokens owning both faces and retaining
the active face. Copy provenance cannot turn a single-faced Clone into a double-faced
object. New copy-with-modification effects still require explicit implementation.
