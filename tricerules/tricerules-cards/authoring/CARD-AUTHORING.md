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
Widen the parameters if only the motivating card fits. Two cards sharing a custom algorithm are
evidence that the algorithm belongs in a generic primitive.

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
cache input is invalid, clients display the supplied deterministic fallback rather than a partial
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

## 6. Add a generic primitive or keyword

Only add vocabulary after confirming existing data cannot express the behavior.

1. Name two real cards or two mechanics supported by the proposed shape.
2. Put the variant in the appropriate `tricerules-cards/src/primitives/` module.
3. Add registry validation for authoring constraints and reject ambiguous or invalid shapes.
4. Implement behavior in the matching engine domain rather than a card-specific dispatch path.
5. Add focused primitive/registry coverage plus happy and illegal scenarios for real card consumers.

Keywords carry their CR citation and behavior in the appropriate engine subsystem. Battlefield
keywords already cross the wire as strings; do not add protobuf solely to publish a new keyword.
If a primitive changes protocol, relay, or UI contracts for a separate reason, follow the full
cross-component workflow and rules interaction checklist.

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

Use `gen-cards --check` against the pinned SHA-verified snapshot to detect drift without writing.
The wrapper can mask a failing child exit code in some PowerShell flows, so verify `$LASTEXITCODE`
or invoke the generator executable directly when collecting completion evidence.

Direct check from the repository root:

```powershell
cargo run --release --manifest-path tricerules\Cargo.toml `
  -p tricerules-cards --features gencards --bin gen-cards -- `
  --input oracle-cards.jsonl.gz --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
```

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

For card-data changes, regenerate `tricerules/CARDS.md` and run the name gate from the repository
root:

```powershell
./scripts/gen-card-checklist.ps1 --check
```

Complete the ruled interaction checklist when adding or changing a substantive primitive,
protocol, relay, or client contract. Rust-only status is N/A for C++ testing only after confirming
that presentation transport, visibility, physical identity, and client behavior did not change.

### Completion checklist

- [ ] Exact Oracle data and `rulings_uri` were fetched; relevant official CR text was verified.
- [ ] The lowest complete implementation tier was used.
- [ ] Whole-card, face, ability, mode, and choice identities are correct and stable.
- [ ] Mana cost, type line, faces, and printed numeric characteristics match the source.
- [ ] Mechanics are typed and engine-authoritative; no legality is encoded in presentation.
- [ ] Every ability and choice-bearing child has stable identity and presentation.
- [ ] Every `OracleLines` mapping matches the correct face's normalized current lines.
- [ ] Every non-obvious `Fallback` has a deliberate reason.
- [ ] Target prompts contain only short, effect-specific click guidance.
- [ ] RON contains no copied Oracle display prose or freeform choice labels.
- [ ] Happy and illegal scenarios cover the implemented mechanics and relevant prompt/choice path.
- [ ] Genuine deferrals are recorded in `partial-cards.tsv`, not runtime RON.
- [ ] `CARDS.md`, generator checks, Rust gates, and `git diff --check` pass as applicable.
- [ ] The final report includes the governed MTG concepts and compliance or deferral note.
