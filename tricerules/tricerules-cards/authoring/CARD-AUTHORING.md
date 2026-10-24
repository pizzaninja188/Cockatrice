# Card authoring guide

This is the canonical workflow for adding or changing cards in the ruled `tricerules` registry.
It covers source research, implementation-tier selection, RON and typed engine authoring,
presentation metadata, partial-card tracking, generation, and completion checks.

Repository-level authority, architecture, and verification rules still apply. Follow
[`tricerules/AGENTS.md`](../../AGENTS.md) and use
[`docs/AGENT-VERIFICATION.md`](../../../docs/AGENT-VERIFICATION.md) as the command source of truth.

Execution order lives in [AUTHORING-WORKFLOW.md](../../../docs/AUTHORING-WORKFLOW.md).
This document is the card-specific reference. Routine authoring starts from a shipped analogue and
its assertions; consult only sections needed for unfamiliar rules or the new card's differences.
Do not reload opening sections, recipes or examples for every card. Current campaign policy owns
model settings; this reference does not start or resume a migration.

## 1. Research before editing

Never implement a card from memory. Reuse saved exact Oracle records, rulings (including an empty
list) and official CR evidence when their provenance, freshness and applicability are established
for the current task. Do not refetch or rebuild that research solely for a handoff or another card
in the same batch. Fetch missing/stale records and resolve changed rules or uncertain applicability
before relying on them. For source research not already covered:

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

### Find implemented analogues

Routine authoring defaults to copying and adapting the closest shipped card or ability, preferably
a complete matching composition, with its map and relevant test assertions. Keep its valid
representation; copying does not require converting primitive lists to Programs or the reverse.
Search local definitions, review maps and tests by card name, effect, cost or target shape. When
the analogue is not obvious, use [Scryfall](https://scryfall.com/) Oracle-text searches or
[Scryfall Tagger](https://tagger.scryfall.com/) functional categories to discover candidates, then
match those candidates against the implemented registry. Search across formats for examples;
this does not authorize admitting cards outside the campaign's scope. External discovery is useful
when it fills a gap, not a mandatory extra search after a suitable implemented example is found.

Inspect the analogue's exact typed definition and the assertions in its cited tests before copying
the relevant structure. Compare every face and clause, costs, targets and player scope, quantities,
timing, choices, identity, tokens and presentation requirements against the new card's Oracle and
rulings. Tag membership, similar wording, a registry entry or a nearby test name is a discovery
lead, not proof that these details match. The direct-RON review packet's ranked nearby definitions
are another navigation aid, not mechanical-equivalence evidence.

Keep routine preflight concise: **copy from; differences; reused evidence; distinguishing tests;
remaining gaps**, once in the existing task/batch record. Reuse supplied exact source, analogue,
test and command paths instead of repeating discovery or preparation. Inspect relevant assertions,
not entire test suites or API files. Reuse reviewed evidence for unchanged behavior; investigate
differences and material uncertainty. Check copied identity, mappings and source comments as well
as mechanics. A missing new-card definition or scenario is ordinary authoring work, not by itself
a runtime blocker; identify a concrete missing semantic contract. An unassessed composition still
needs assessment, actual-card execution evidence and independent review before admission.

## 2. Authority boundary

There are two card databases, and they must not be mixed:

| Database | Owner | Purpose |
|---|---|---|
| `cards.xml` and external Oracle/Scryfall data | Cockatrice client and freeform | Display, images, names, type lines, and search |
| `tricerules-cards` data and typed primitives | Rules engine | Costs, legality, targets, characteristics, abilities, and effects |

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
4. **Engine capability:** add typed rules semantics only when the existing data model cannot
   express the required behavior; do not add card-specific resolution handlers.

Before adding a primitive, name at least two real cards or two distinct mechanics it supports.
Generalize only for demonstrated uses. If only the motivating card fits, justify the necessary
specialized behavior rather than adding speculative parameters. A second real use is evidence that
the primitive should be reusable, while unique behavior needs a narrow, explicit contract.

`SpellEffectKind` is shared by spells, activated abilities, and triggered abilities. Prefer a
reusable typed effect over a card-specific path. `TargetKind::Self_` binds the source without
targeting under CR 115 and is invalid in spell effects; do not treat every effect subject as a
chosen target.

The [selected implementation examples](capabilities/README.md) are an optional navigation aid,
not a capability inventory or required research step. Use an entry when it helps find an analogue;
if it provides only a loose match, continue directly to definitions and tests. Ordinary authoring
requires neither index hit/miss reporting nor a pilot protocol. Absence from this narrow collection
does not imply lack of runtime support. When relying on an entry, check its boundaries and current
supporting evidence; unrelated freshness warnings do not create a catalogue-wide maintenance task.
Exact source checks and independent review of the new card remain required.

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
6. Add the card to applicable shared happy/illegal scenarios with independently reviewed inputs
   and expectations, or write focused scenarios where reuse does not cover its differences. Keep
   explicit zone, step, priority and state assertions; see [reusable semantic evidence](#reusable-semantic-evidence).

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
| Spell-program instruction | `id` (unique throughout the program tree) |

For Rust engine behavior, reject illegal input with `EngineError::Illegal` rather than panicking.
Keep steps, priority, event-time facts, new-object identity, hidden information, and player roles
explicit. Use player-set-generic logic rather than two-player arithmetic.

### Finite authored programs

Use a `CardProgram` at one exact resolution owner when its supported ordered actions need explicit
results, nested conditional suspension, or a target group bound to an instruction. The owners are
the card face's `spell_program`, one modal mode's `program`, or one activated/triggered ability's
`program`. A program replaces that owner's legacy effect list. Modal spell and triggered modal
owners are all-authored or all-legacy: when any sibling declares a program, every sibling must
declare one; otherwise their effects stay on ordinary effect-list dispatch. Each authored mode keeps
its own targeting groups and group indices. Do not add a face-level `spell_program` to a modal spell.

At each family checkpoint, inventory its real RON and generator callers before reusing a legacy
signature, then retire that exact path when its last caller moves to the typed contract. Keep shared
rule-transaction state that still has live callers, and use one shared lookup or control-flow walk
when the validated semantics are the same; add specialized runtime state only for a concrete rule
need. Whole-card AST and Oracle-source checks belong in conformance tests unless the runtime
operation itself consumes that metadata. Report production-code changes separately from tests,
documentation and card data so necessary typed-state growth and retired code remain visible.

The exact triggered Equipment ETB profile used by Conductive Machete, Cursed Windbreaker, and
Killer's Mask is `ManifestDread` bound as an optional `ManifestedPermanent` result with cardinality
`0..1`, followed immediately by `AttachEquipment(input: manifested, equipment: Source,
creature: PreviousEffectObject)`. This binding carries the exact permanent incarnation produced by
the entry, so a skipped or empty manifest yields no attach target and a stale Equipment source is a
no-op. This admission does not generalize to other trigger shapes, attach recipients, or the legacy
`Attach` effect. Preserve each owner's printed Equipment modifier and equip cost in its owner
assertions. The runtime has a direct skipped-entry disposition, but the current admitted cards do
not provide a scenario that produces that disposition; report that evidence as deferred rather
than treating ordinary successful entry coverage as proof of it.

Kyoshi Battle Fan's `triggered_01` has its own exact token-receipt profile: an ordinary untapped
`CreateTokens(token: ally_w_1_1, count: 1, who: Controller)` binds `CreatedTokenBatch(0..*)`,
immediately consumed by `AttachEquipment(input: created_tokens, equipment: Source,
creature: PreviousEffectObjects)`. The receipt is the complete replacement-expanded token cohort,
including a completed empty result, with each token's exact object and generation. Only this
producer, binding, input, owner and recipient shape is admitted; this specialized use records the
whole cohort because one created token auto-attaches, several require a choice, and an empty batch
does nothing. `AttachEquipment` also supports distinct mechanics with targeted subjects on
Swordsman and Sharp Scoundrel, and a selected Equipment on Vow to Erebor; those do not consume this
token receipt.

Vow to Erebor's spell Program is admitted only for its exact face and target shape. Bind the same
caster-controlled creature target to untap and +2/+2, test that target for the Dwarf subtype in a
resolution-time `GameCondition(ObjectMatches(...))` branch, then offer zero or one of the caster's
Equipment with `EquipmentAttachableTo(ChosenTarget(0, 0))`. Attach only the exact selected Equipment
to that already-targeted creature. The selection is optional and is not an additional target
(CR 115.10, 608.2d). This narrow owner profile permits the chosen-target reference needed by the
live Dwarf guard and attachment constraint; it does not admit widened target filters, constraints,
or conditions for other owners.

The finite tree has `Perform` leaves and nested `If`/`IfResult`/`Choose` blocks; no loops, jumps or
authored functions. `Choose` consumes no prior result and requires at least one branch; its chooser
is `Controller`, with `SourceController` also admitted for ability Programs and captured
`AffectedPlayer` for triggered ability Programs that supply the required input. Its branches must be
either all free or all paid; supported costs are `None`, fixed
nonempty `Mana` without X, fixed nonempty `Waterbend` without X, the fixed one-card
`PutHandCardOnLibraryBottom` cost on a `Controller`-chosen spell branch, the one-card
`ExileGraveyardCard` cost on a `Controller`-chosen optional spell branch, a fixed positive-count
`Blight` on a `Controller`-chosen ability branch, a filtered `DiscardCard` cost on an optional
`Controller`-chosen ability branch, and one-permanent `SacrificePermanent` or `TapPermanents` costs
on a `Controller`-chosen ability branch. Blight selects one creature that
player controls and that
can receive the counters; it is not admitted on spell programs or with other chooser shapes. The
hand-bottom cost is not admitted for ability programs. A Program
sacrifice filter requires every terminal leaf to select a `Creature` or `AnyPermanent` controlled
by `You`; `source_only: true` remains bound to the exact source and cannot use exclusions. With
`source_only: false`, flat filter-level `excluded_objects` remains supported, but exclusions on
`any_of` branches are rejected by typed Programs. The flat `ChooseResolutionBranch` syntax is
retired in full and rejected during deserialization; author resolution-time branches with typed
Program `Choose` instead. A Program tap-cost
filter must be flat, select a `Creature` or `AnyPermanent`
controlled by `You`, and cannot use `any_of` or filter-level `excluded_objects`. The cost-level
`exclude_source: true` instead omits the exact source incarnation (ObjectId plus zone-change
generation), as Gravelgill Scoundrel requires when it taps “another” creature. A candidate must be
untapped for the tap payment (CR 701.26a); this remains an untargeted resolution cost. `Always`, supported
controller-state `GameCondition`, and exact owning spell face/mode `CastCostReceipt` requirements
are admitted; flat-list applicability and card-result requirements are rejected. Branch and
Otherwise bodies use the same recursive finite-program subset, and a binding may join only
compatible explicit yields from every branch and Otherwise exit. Declining an optional choice or a
paid branch follows Otherwise. A cost-free branch whose sole instruction is a `Draw` is omitted
when the current draw limit prevents that draw in full; paid branches remain selectable and the
draw operation still applies normal draw limits when it resolves. Common targetless leaves include
`Draw`, `Discard`, `Blight`, `ChoosePermanents`,
`UntapChosenPermanents`, `SelectHandCards`, `PutSelectedHandCardsOnLibraryTop`, `Mill`, `ChooseCard`,
and `GainLife`,
plus ordinary
fixed-count controller `CreateTokens` without delayed sacrifice. Their bounds, along with other
accepted targetless forms, are described below.

One narrow ability `Choose` requirement is admitted for the Trade Route Envoy shape:
`BattlefieldCreatureCount` with `controllers: Controller`, `requires_any_counter: true`,
`exclude_source: false`, `min: Some(1)`, `max: None`, and no subtype, keyword, tapped, or
specific-counter constraints. This predicate is admitted only as an ability `Choose` requirement;
generic Program `If` remains unchanged. Trade Route Envoy is the reviewed use: its mandatory
controller `Choose` has one free `Draw(1)` branch and a Source +1/+1-counter `Otherwise`. The live
condition determines whether a countered controller creature exists; branch availability also
checks whether the free draw instruction can start under the current draw limit. With that creature
still present, Narset's exhausted opponent draw limit makes Envoy's draw branch unavailable, so
`Otherwise` places the counter (CR 101.2, 101.3, 118.3, 614.17b; this is a reasoned application,
not a card-specific ruling). This is not a general rule that every card that does not actually draw
selects `Otherwise`: an available draw with an empty library is still attempted and commits the
library-loss state, and an Abundance replacement suspends/resumes the started draw without
re-running `Choose` or counting as an actual card draw. See the [Trade Route Envoy review
map](review-maps/trade_route_envoy.json) for its line mapping and executed fixtures.

A spell Program also admits the resultless
`ExileTopWithPlayPermission` leaf only as `player: Controller`, `count: 1`, and
`count_by_cast_cost: None`. It uses the existing permission operation, keeps ordinary costs and
timing for playing the exiled card, and creates no permission when the library is empty. Impossible
Inferno uses this profile in its Delirium arm; its exact source mapping and executed fixtures are in
the [review map](review-maps/impossible_inferno.json). Burning Curiosity and Clockwork
Percussionist remain effect-list consumers of the same operation. Other Program recipients,
counts, cast-cost conditionals, and ability contexts remain unsupported. Spell-face and modal-spell
programs limit draw/discard recipients to `Controller`. Triggered ability programs may also use
`AffectedPlayer` for a draw/discard recipient or as a `Choose` chooser when their trigger supplies
that captured event-time player. For `Draw`, ability programs also admit
`EachOtherPlayerThanAffectedPlayer`, anchored to the same required input; this recipient is not
admitted for other effect kinds. Activated programs do not have that trigger input. Standstill uses
this Program draw form; Zenith Chronicler and Heartwood Storyteller are existing effect-list
analogues. Ability programs admit `Discard(EachOpponent, Exact(1))` as a combined
opponent cohort and the narrow `Discard(EachPlayer, Exact(1))` shape used by Fanatic of the
Harrowing; neither has a fixed aggregate maximum. For an `EachPlayer` result, only an unfiltered
controller `DiscardCount` with `min: Some(1)` and `max: None` is admitted, so the condition tests
whether that ability's controller discarded at least one card. Other cohorts, contexts, quantities
and predicates remain unsupported for this `EachPlayer` projection; controller-produced results
retain their existing predicate rules. This pairs Burglar Rat's player-set discard mechanic with
Gristle Glutton's typed controller discard-count mechanic. A begun ability-owned mandatory
player-set discard continues if its controller leaves; departed rows and their selections are
retired together while the live accepted prefix and its generations remain fixed (CR 101.4, 608.2m,
800.4a). Observer restoration republishes that same transaction with a fresh prompt ID. This runtime
hardening does not admit new syntax; spell, optional and alternative-cost departure behavior is
unchanged. Permanent choosers must be `Controller`; draw and hand-selection counts remain fixed
literals. `GainLife` normally uses a fixed literal, with one bounded result-count form: it may
consume the explicitly bound result of the immediately preceding unfiltered target `Discard`
(`CardsMatchingResult` with `players: All`) or `Mill(who: Controller)` (`players: Controller`).
The filter must use `source: PreviousEffect` and match that producer's action and player set. The
amount may be the direct count or exactly one positive term in `Affine(constant: 0, ...)`; its
producer must have a finite result maximum and the coefficient times that maximum must fit in
`u32`. This Program-only path uses the committed typed cohort, including an empty cohort as zero;
it does not reopen result leaves in general affine quantities. Gerrard's Verdict uses the target
Discard form, counting only Land cards in the committed discard cohort (CR 608.2c, 701.9a–c).

`GrantKeywordsAll` is also an accepted resultless `Perform` leaf when the ordinary effect
validator accepts its complete `CreatureScopeFilter`. Every untargeted validated scope can use it
with the operation's existing keyword values in a spell or ability Program; `TargetedPlayer` controller scopes remain
unsupported because this operation has no Program target-role binding. The existing one-shot resolver
snapshots the matching creatures at resolution, grants the keywords in layer 6 through end of turn,
and does not include later entrants (CR 611.2c, 613.1f, 702.111). Lord Skitter's Butcher uses the
ability Program form for its Menace mode; Overrun and Make a Stand are existing `GrantKeywordsAll`
effect-list analogues. This adds no card admission or runtime operation.

Modal spell modes also admit `GrantKeywords(subject: PreviousEffectObject, ...)` when it consumes an
explicit `ChoosePermanents` result typed as `PermanentSelection` with cardinality `0..1`. The
choice remains `min: 1, max: 1` when a candidate exists; an empty legal set produces no selected
object, so the grant is a no-op. This consumer is spell-only and does not generalize to other
contexts, result types, or cardinalities. Resultless `RemoveAbilitiesAll` and `DestroyAll` are
accepted `Perform` leaves through their ordinary effect validators. Final Showdown demonstrates
these forms with every modal sibling authored as a Program under the all-sibling rule above.

The filtered Program discard cost is narrowly admitted as `DiscardCard(filter: Some(Creature))`
on an optional `Controller`-chosen ability branch; it selects one creature card from that payer's
hand. Crypt Lurker uses it beside a creature-sacrifice branch, and either paid branch draws one
card. Its pinned Oracle line allows one of the two actions; the 2020-06-23 ruling confirms they
cannot both be performed to draw more than one card. This resolution-time payment follows CR
118.12, discard follows CR 701.9, sacrifice follows CR 701.21a, and a triggered ability on the
stack exists independently of its source (CR 113.7a). Other `ResolutionCost::DiscardCard` filters
and chooser contexts in Program `Choose` remain unsupported. The
`ResolutionCost::DiscardCard` type remains in use by Ward's separate
`CounterTriggeringStackObjectUnlessPays` Program leaf with `filter: None`; that does not restore
flat `ChooseResolutionBranch` discard support.

A Program `ExileGraveyardCard` cost is admitted only on an optional `Controller`-chosen spell
branch. It selects exactly one card from that spell controller's graveyard when paid and may carry
an ordinary validated `ZoneCardFilter`; Heated Argument uses the unfiltered form. The resolving
  spell controller remains the payer (CR 118.12). This specialized cost does not admit
ability-owned exile costs or multi-card payments.
The legacy flat `ChooseResolutionBranch` form of this cost is retired; author it only in the typed
Program `Choose` shape above.

`DamagePlayer(amount: Fixed(...), who: ControllerOfTargetGroup(g))` is a fixed-amount,
non-targeting spell-program input. It is admitted only when group `g` is a singleton bound to that
Program's `Creature` `DamageTarget` role. It adds no target role; while the exact target incarnation
remains on the battlefield at its expected generation, the recipient is its current controller;
after it leaves or changes generation, the recipient is its generation-specific last-known
controller (CR 608.2h). Heated Argument uses this rider. Chandra's Outrage is the exact
reference-only `DamagePlayer` leaf analogue (4 damage to a target creature, then 2 to that
creature's controller); it is not ported by this subset. The untargeted
`DamagePlayer(amount: Fixed(n), who: EachOpponent)` form is also admitted only on spell Programs,
for positive `n`. It creates no target role and applies ordinary noncombat damage to the spell
controller's current opponents with normal replacement, trigger, and resume behavior (CR 101.4,
120.3a). Grab the Prize uses 2; this reuses the existing `DamagePlayer` operation also used by
Boltwave, Boggart Cursecrafter, and Guttersnipe on their current effect-list surfaces. Zero, X,
ability-owned, and other recipient forms are not admitted by this shape.

`ImportCastPayment` is a no-input spell-Program instruction that binds one typed
`CastPaymentCohort(Discard, 1..1)`. Registry admission requires a nonmodal instant or sorcery face
whose `additional_costs` is exactly `[DiscardCard]`, with no cast-cost groups, prototype,
flashback, harmonize, warp, sneak, instant-speed cast cost, or cost modifier. It cannot be used by
ability or modal Programs or with another cost profile. At resolution start, input preflight checks
for exactly one canonical discard `ActionFacts` entry before instruction zero, even when the import
is nested. Its compiled slot retains the `CastPaymentCohort` descriptor while runtime stores the
cloned action-time receipt in the existing `ProgramValue::CardResultCohort`. It does not pay again,
re-read card characteristics, or require the original payer to remain live. A spell copy retains
the receipt, including when the original payer has left (CR 118.8, 601.2h, 707.10). Grab the Prize
uses this narrow import because its resolution condition reads the card paid for its mandatory
discard cost; other cast-payment profiles remain outside the admitted shape.
Ability-program `Choose` may also use `SourceController`. The chooser is captured from the exact
current source incarnation, or its last-known controller after it leaves, when the offer is made;
that captured player remains the original chooser throughout the occurrence. A paid branch keeps
that original chooser as its payer through payment and any counter-replacement prompts; payment
cannot be transferred to a delegate. If the payer leaves, `Otherwise` can run only when another
player's owning resolution survives. In the admitted `Controller`-chosen Blight shape, the payer
also controls the triggered stack object: departure removes it under CR 800.4a, without entering
either the paid body or `Otherwise`. A cost-payment choice is never delegated (CR 800.4f).

An ability-program `Choose` may use `AffectedPlayer` only when its triggering ability captures that
player. Registry validation requires `TriggerAffectedPlayer`; a missing value fails before
instruction zero. The captured actor remains the chooser unless that player has left before the
ability resolves or while its choice is parked. The engine then delegates only the pending decision
and publishes a fresh prompt for the selected live player (CR 800.4g). Resolution prompt IDs are
scoped to each deciding player, so stale-answer checks compare the submitting actor with that
actor's prompt ID. Delegation does not change the ability controller or any compiled cost payer.

For an ability `Choose` Blight payment,
counter replacements finish before the cost transaction commits. CR 614.16 governs when an
earlier replacement makes an effect-created-counter replacement apply; the affected permanent's
controller chooses applicable replacements one at a time, repeating the check after each (CR
616.1e-f). The selected body and the remainder of the resolving program finish before
state-based actions are checked (CR 704.4).
The ordinary `CreateTokens(..., who: SourceController)` leaf instead chooses its recipient when the
effect resolves, so a later source control change can make that recipient differ from the earlier
chooser. Ability Programs also admit fixed-positive `PutCounters` and `RemoveCounters` on `Source`
as ordinary effects, plus the existing resultless `RemoveAllCounters` shape, for validated counter
kinds. Replicating Ring and Lavabrink Floodgates use source-counter placement and threshold checks;
Replicating Ring and Lightning Coils use removal of all counters of one kind. `EffectsApplicable`
is still admitted only on an ability `Choose` branch whose complete body is one resultless,
fixed-positive `PutCounters(PlusOnePlusOne, Source)` operation. The engine
checks whether that exact source incarnation can receive the counters both when the menu is offered
and when a branch answer is validated. A mandatory choice with exactly one live cost-free branch
enters it automatically and logs the selection; optional choices and paid branches still receive
prompts.
For `PutHandCardOnLibraryBottom`, the chooser privately
selects one current hand card; if it is a declared Commander, its owner chooses Command zone or Library.
For `SacrificePermanent` and the one-permanent `TapPermanents` cost, the parked occurrence retains
its original `Controller` payer and candidate ObjectIds with their zone-change generations. The
engine revalidates the selected candidate's incarnation, filter, and control before committing the
shared cost transaction; a source-only sacrifice or `exclude_source` tap remains tied to that exact
source incarnation. `Choose.optional` controls both branch selection and permanent-cost payment. A
mandatory (`optional: false`) permanent-cost offer requires its one object and rejects Decline or
an empty selection; an optional cost may be declined and then follows `Otherwise` once. A cost with
no current candidate is omitted; if no branch remains selectable, `Otherwise` runs once. A departed candidate
owner loses their objects under CR 800.4a, after which the
surviving payer receives a refreshed candidate offer. If the ability controller/payer leaves while
their triggered ability is resolving, CR 800.4a removes that stack object; neither the cost nor its
branch body or `Otherwise` resolves. A Finality counter can replace the graveyard destination with
Exile (CR 122.1h) without undoing the selected sacrifice payment or preventing its paid body. If a
declared Commander is sacrificed into a graveyard, the search branch and parent tail finish before
the CR 903.9a state-based action is checked (CR 704.4). If a hand-bound library search finds a
Commander, its owner makes the CR 903.9b Command-zone-or-Hand choice before movement, then the same
search continuation completes.
`If` may also inspect the controller's current graveyard with `GraveyardAggregate(CardCount)`,
validated inclusive bounds, and an optional validated `ZoneCardFilter`. `AllOf` and `AnyOf` remain
recursive for admitted leaves. The only `DistinctCardTypes` exception is a direct `If` condition in
a spell Program with `owners: Controller`, `filter: None`, `min: Some(4)`, and `max: None`; this
preserves Impossible Inferno's exact Delirium profile. Other graveyard owners and aggregate
profiles remain unsupported, and the exception cannot be wrapped in `AllOf` or `AnyOf` or used by
an ability Program. The existing Controller `CardCount` condition remains valid. A direct
top-level Program `If` may also test `Void`, which is sampled when that instruction resolves before
the selected branch begins; a later draw-replacement suspension does not reevaluate the condition
(CR 608.2c, 608.2h). `Void` is not admitted inside `AllOf`/`AnyOf`, as a `Choose` requirement, or
as the predicate for a conditional top-of-library search. Ability Programs may use the closed
source-condition family in `If` at any nesting depth and in `Choose` requirements:
`SourceCounterCount` with ordinary bounds, and live `ObjectMatches(Source, AnyPermanent)`.
`AllOf` and `AnyOf` recursively combine these leaves using the ordinary nonempty and bounds
validation. Counter lookup keeps its existing source/LKI behavior; `ObjectMatches` separately
requires the exact source to be a live permanent. These source conditions remain unsupported in
spell Programs, conditional-search predicates, event conditions, and cross-object matching. Replicating
Ring and Lavabrink Floodgates use this family after preceding instructions; counter-replacement
choices finish and resume the same Program before a following threshold check (CR 608.2c, 608.2h,
616.1e-f, 122.2, 400.7). The graveyard test
also runs when its instruction resolves, after preceding steps, so Dawnhand Eulogist checks its
graveyard after milling and HYDRA Troopers checks before either its token creation or fallback mill
(CR 608.2c, 608.2h, 701.17a-b). `LoseLife` is resultless for a fixed amount applied to `EachOpponent`, or for a fixed
amount applied to `Controller` on a spell or ability program; neither form can bind or yield a
result. Dawnhand uses the EachOpponent form, which it shares with Acolyte of Aclazotz and Arbiter of
Woe. Decode Transmissions and Visions of Villainy use the fixed Controller form on spell programs.
An exact face-owned spell Program may use `If(condition: CastSnapshot(index: n))` only when `n`
references an existing entry in that face's `cast_conditions`. The cast origin is captured as the
spell is cast and the Program reads that snapshot when its instruction resolves (CR 601.2, 608.2c);
it does not reevaluate the cast condition. This remains unavailable to ability- or mode-owned
Programs and to `Choose` requirements. The Final Days uses the snapshot to select its graveyard
token count or its fixed fallback.
Spell Programs may create tokens with a fixed count, or with
`Count(GraveyardCards(owners: Controller, filter: Some(valid_filter)))`, only when the token
recipient is `Controller`. The explicit filter is validated by the normal graveyard-card filter
contract. This count is read when the `CreateTokens` instruction resolves (CR 608.2h); The Final
Days counts creature cards in its controller's graveyard after the cast snapshot selects the
graveyard branch.
`If` may also inspect the current battlefield with `BattlefieldAggregate(Count)`, inclusive count
bounds, and a validated `BattlefieldPermanentFilter`. This family requires
`exclude_observed_object: false` and `Controller` at the root filter and every recursively nested
`any_of` filter node; `AllOf` and `AnyOf` may compose only admitted condition leaves. The count is
evaluated when its instruction runs, after preceding steps. Hero in Training counts another
controlled permanent with the Hero subtype, including noncreature Kindred Heroes and a returned
new generation of the same physical card. Tend the Sprigs counts each controlled permanent once
when it matches Land or Treefolk after its Basic Land search. These shapes do not admit other
legacy creature counts, observed-object-dependent counts, or other relative player sets. Other
battlefield aggregate leaves remain unsupported except for the separate exact `MaximumPower` form
below.
`If` separately admits one `BattlefieldAggregate(MaximumPower)` form: exactly the controller's
creatures, `min: Some(positive)`, `max: None`, `exclude_observed_object: false`, and every other
`BattlefieldPermanentFilter` field at its default. It uses live power when that instruction runs;
it does not admit another cohort, bound, filter, or observed-object context. Scalestorm Summoner
uses this condition in its triggered ability. Fanatic of Rhonas's mana condition and Drowsing
Tyrannodon's attack restriction are existing `MaximumPower` analogues; this Program shape reuses
the current condition evaluator and adds no runtime operation.
`ReturnAllToOwnersHand` is also an accepted resultless leaf. Its validated permanent filter defines
the simultaneous cohort; Commander owner-hand replacement choices suspend and resume that same
operation, and the leaf cannot bind or yield a result.
`Destroy` has two distinct Program shapes. An ability Program may use only resultless `Destroy` of
a `Chosen` permanent with exactly `kind: AnyPermanent`, `permanent_types: [Artifact]`, and all
other `TargetFilter` fields at their defaults. Bind its `ProgramTargetRole::Subject` to the matching
target group owned by that ability or selected modal mode. Gearbane Orangutan uses this form for
its zero-or-one target artifact mode. It reuses the existing Destroy operation also used by Abrade
and Ancient Grudge; it does not admit another ability-side filter or subject, or
`DestroyPreventingRegeneration`.

A spell Program may bind the result of `Destroy(subject: Chosen(filter))` when the existing effect
and Program target-filter validators accept `filter`. Its producer must bind the result and have
exactly one `ProgramTargetRole::Subject` reference to that same instruction and target group, with
the group schema fixed to exactly `1..1`. The compiler represents the output as
`TargetCardResultCohort { action: Destroy, cardinality: 0..1, target_group_index }`; this is a
compiled slot type, not authored syntax, and `CardResultCount` is its only admitted consumer. The
existing runtime cohort keeps the successful `CardResultEntry`: target owner, ObjectId, post-move
generation, and `ActionFacts` provenance remain intact, while matched card types come from the
target's effective characteristics captured at the Destroy evaluation boundary before movement.
A successful Destroy still counts when a replacement changes its destination; indestructible and
regeneration produce no result. A stale or illegal target follows the normal target-legality gate,
so the spell does not resolve and produces neither result nor tail (CR 608.2b, 608.2h-i, 613.1,
613.1d, 701.8a-c, 701.19a-b, 702.12b, 614.1a, 616.1, 400.7). Shredder's Technique counts
Enchantment and makes its spell controller lose 2 life after a successful Destroy, even when
another player owns the target. Mortify has the same Creature-or-Enchantment chosen-Destroy
shape if migrated to this result form; its existing definition remains unchanged. Non-chosen
subjects, mass or attached Destroy, missing/repeated/mismatched Subject bindings, and target groups
other than exactly one remain unsupported. The target remains subject to the usual resolution-time
legality checks and destruction/replacement rules.

One separate spell-only profile binds a targeted hand discard. `ChooseHandCards` must use
`action: Discard`, a positive fixed count, `target: AnyPlayer`, `chooser: AffectedPlayer`, no card
filter, `optional: false`, and private-look visibility. Bind its result once with
`ProgramTargetRole::Target` to that instruction's target group, whose schema is exactly one
`AnyPlayer`. The compiler emits `TargetCardResultCohort { action: Discard, cardinality: 0..count,
target_group_index }`; the range reflects a target with fewer cards than requested. `CardResultCount`
is its only consumer and may filter the captured `matched_card_types` snapshot. Gerrard's Verdict
uses this form to gain 6 life for two visible discarded Lands, otherwise 3 for one; it shares the
same target-origin cohort and count operation as Shredder's Technique's successful chosen Destroy
result, while each retains its own action and exact target-group origin. A target that leaves after
the hand choice begins contributes an empty result without delegation; a begun spell continues
after its caster leaves, but a departed caster receives no life gain (CR 608.2m, 800.4a). An
unrevealed Library of Leng discard has undefined characteristics and contributes no card-type match
(CR 701.9c); never recover those types from the server's hidden card identity. Other hand actions,
choosers, visibility, filters, optional choices, target-group sizes, and ability-owned forms remain
unsupported.

An ability Program may bind `MoveGraveyardCards` only with destination `Exile`, no linked Exile,
no entry modifiers, and an ordinary validated graveyard filter that does not depend on a trigger
object. It requires one explicit `ProgramTargetRole::Target` reference to the instruction and one
controller-chosen graveyard-card group with bounds `0..2`, `same_graveyard: true`, and empty legacy
`effect_indices`. The compiler derives `TargetCardResultCohort { action: Exile, cardinality: 0..2,
target_group_index }` from that validated group. Named `CardResultCount` consumes only successful
Exile receipts, including their captured card types and exact owner/ObjectId/generation facts.
Soul-Shackled Zombie uses this profile when at least one creature was exiled: each opponent loses
2 life and its controller gains 2 life once. Arashin Sunshield uses the same underlying targeted Exile
operation but remains on its existing path. Zero targets and a surviving noncreature alone cause
no life change. Illegal or stale targets contribute no receipt; an already-triggered ability
survives source departure. Spell-owned forms, other destinations, multiple groups, other group
bounds, and event-dependent filters remain unsupported. This does not widen the separate Destroy
or targeted hand-discard profiles.

Two additional leaves are restricted to ability-owned Programs: `Sacrifice` may use only
`subject: Source`, and `ApplyCombatRestriction` may use only `scope: Source` with
`restriction: (cant_be_blocked: true)`; every other restriction field must remain at its default.
Source sacrifice remains resultless for existing uses such as Command Bridge. When bound, the
ability Program exposes the existing `CardResultCohort` type with action `Sacrifice`, recipient
`Controller`, and cardinality 0..1. Its only admitted consumer is an explicit input to the immediately
following `CreateReflexiveTrigger(when: Some(Sacrificed(Source)))`; the producer must be a direct
`Perform(Sacrifice(Source))` in the same block. Wrong-type, non-immediate, or branch-ambiguous inputs
are rejected. Runtime checks committed Battlefield movement for the exact source ObjectId and
generation and the instructed controller. An empty cohort creates no reflexive trigger; a successful
sacrifice remains successful if a replacement changes its destination (CR 701.21a, 400.7). The
existing `sacrificed_objects` receipt remains available to the normal resolver. Gravelgill Scoundrel
applies the source combat restriction only after its tap-payment branch is paid. These exact shapes
do not admit other sacrifice subjects, restriction scopes, or restriction fields.
`Scry` is a resultless Program leaf with a fixed literal count. A fixed count of zero is accepted in
Programs only; legacy effect lists retain their positive-count rule. The resolving card's controller
makes the normal library partition choice: zero does nothing (CR 701.22b), an empty library completes
without a choice, and multiple retained cards use the normal order choice (CR 701.22a). Scry accepts
no input result and binds or yields no output. The reviewed owners are Cavern Stomper's Scry 2 entry
trigger and Greedy Freebooter's Scry 1 followed by its Treasure creation. This bounded Program subset
does not add trigger-observer support for `Whenever you scry`, change who can see or answer the
existing private prompt, or admit additional card definitions.
`CounterTriggeringStackObjectUnlessPays` is a resultless `Perform` leaf only in a triggered ability
Program. Its cost must be either a fixed, nonempty `Mana` cost with no X or
`DiscardCard(filter: None)`. The exact trigger owner must provide both `TriggerAffectedPlayer` and
the retained `TriggerTargetingStackObject` inputs; registry admission checks those producers and the
runtime validates both before instruction zero. The captured `StackObjectRef` retains its optional
zone-change generation, preserving it whenever present; nonphysical abilities and copies may
legitimately have no generation. A missing outer reference is malformed and fails input validation;
it is distinct from a valid captured reference whose object is no longer live. The leaf cannot own a
target group or bind/yield a result. For example, a Ward `{2}` trigger has this direct leaf shape:

```ron
effect: [],
program: Some((body: (steps: [
  (id: "ward", kind: Perform(CounterTriggeringStackObjectUnlessPays(
    cost: Mana("{2}")
  ))),
], yields: []))),
```

Keep the operation on the exact triggered-ability owner whose Oracle text contains Ward, retaining
its ability ID when authoring or generating the definition. Other legacy Ward effect-list owners,
including nested or modal children and granted triggered abilities, are rejected so they cannot
bypass the captured-input contract.

Ward means “Whenever this permanent becomes the target of a spell or ability an opponent controls,
counter that spell or ability unless its controller pays [cost]” (CR 702.21a). The counter attempt
uses the captured stack-object reference's `ObjectId` and, when present, its generation, not Oracle
display text. If that exact spell or ability has already left the stack, the payer still receives
the payment option; the counter attempt then does nothing because the action is impossible. The
payment-before-counter order comes from CR 118.12a and applying CR 609.3 to the absent object is an
inference, not a direct ruling about this case. Once Ward begins resolving, its ability continues if its source or controller
leaves (CR 608.2m); a payer who leaves before answering has not paid (CR 800.4f). Saved mana/discard
choices retain the same Program cursor; observer restoration rebuilds current offers with a fresh
prompt ID and current card generations.
`LookChooseToHand` is a resultless leaf only for spell Programs with `filter: None`,
`reveal: false`, `bottom_order: Chosen`, and a fixed positive exact selection (`min == max`)
within the looked count. It offers the controller the existing private `LIBRARY_LOOK` choice,
then completes the chosen bottom-order prompt before resuming the same Program cursor. A zero- or
one-card remainder completes without the order prompt; an empty library continues without a choice
or draw. Moving a library card to its owner's hand without the word “draw” is not a draw (CR
121.5), and the owner may order cards placed at the same library position (CR 401.4). Flow State's
`ProgramIf` checks the controller's current graveyard for both card types when that instruction
resolves and keeps that branch through the private look (CR 608.2c, 608.2h). Filtered, revealed,
optional, random-bottom, and ability-owned `LookChooseToHand` forms remain available on the legacy
effect-list path only; this Program subset does not widen those forms or the existing private
candidate projection.
`Proliferate` is an accepted resultless leaf: Ezuri, Stalker of Spheres uses it twice in one
paid branch, with each instruction retaining its own recipient choice and trigger event.
`CreateReflexiveTrigger` is accepted as a resultless leaf in an ability-owned Program when its
reflexive child contains one fixed-amount `DamageTarget` with a valid Program target filter, or the
existing `Tap` of a chosen opposing creature (Icewrought Sentry). Sparktongue Dragon uses fixed 3
damage with `AnyTarget`; other fixed amounts and ordinary filters use the same targeting contract.
One additional exact child is admitted for `when: Some(Sacrificed(Source))`: its explicit input must
be the immediately preceding bound `Sacrifice(Source)` result, and its ordinary effect list contains
one fixed-positive `DamageAll` for all players with the exact default Creature filter. Lavabrink
Floodgates uses six damage. This reuses ordinary damage handling and does not admit a direct Program
`DamageAll` leaf. The inner ability cannot add a separate `when`, targeting definition, or
intervening condition, and a spell Program cannot create a reflexive child without a captured
permanent source (CR 603.12a, 120.4b, 616.1e-f).
Program `SearchLibrary` has bounded forms. The retained form uses fixed selection bounds and
`KeepInLibrary`, and produces a typed `FoundLibraryCohort`. Direct resultless `TopOfLibrary` and
`Hand` forms use the controller's fixed `Library`, count one, shuffle, no slots, selection
constraint, cast-cost sizing, conditional destination or result ID, and either a valid ordinary
`ZoneCardFilter` or no filter. Qualified searches use bounds 0..1 so they may fail to find; an
unrestricted search uses bounds 1..1 when candidates exist. Both preserve the authored reveal flag.
Fountainport Bell, Campus Guide and Spider-Bot use `TopOfLibrary`; Environmental Scientist,
Strixhaven Skycoach and Tempest Hawk use `Hand`. Demonic Tutor is an unrestricted private Hand
search, and Fierce Empath uses the Creature/minimum-mana-value filter. The `Battlefield` form remains
limited to `BasicLand`, tapped entry, and no reveal. It supports the existing bounds 0..1 used by
Lumbering Worldwagon's ETB and attack triggers and Solemn Simulacrum's ETB, and bounds 0..2 for
Larval Scoutlander's ETB search. For bounded searches, publish `min(bounds.max, count)` as the
maximum without shrinking it to the live eligible-candidate count; candidate generation and
submitted selections remain filter-validated. This prevents the eligible count from changing the
offer maximum, but does not establish full library-identity secrecy. These direct resultless forms
bind and yield no search result, so finding no card, including when the library is empty, still
shuffles and continues the program (CR 701.23b, 701.24e). A declared Commander selected for the
Hand form stays in the library while its owner chooses Command zone or Hand, then the exact
selected incarnation moves, the remaining library shuffles once, and the caller resumes at its next
instruction (CR 903.9b). A selected Battlefield card follows the normal entry-replacement and
entry-trigger pipeline before the search resumes its caller (CR 614.12, 616.1, 608.2m).
Embermouth Sentinel's `triggered_01` is the current conditional-destination use: its optional
controller `Choose` has one free `Always` branch, whose resultless search checks one
`BasicLand` with bounds 0..1, reveals the selection, and
shuffles; the base destination is the top of the library, changing to tapped Battlefield when the
controller's battlefield contains at least one Dragon of any permanent type. The filter has no
Creature restriction, so a noncreature Kindred Dragon qualifies (CR 109.2, 205.3m). This conditional
search keeps that destination shape and accepts a predicate from the same live condition domain as
Program `If`; source/cast snapshots and other condition contexts remain excluded.

Private, non-revealing Hand searches do not emit an explicit `CardsRevealed` event, and the relay
redacts library-search candidate details from non-deciders while keeping hand views private.
However, `PermanentMoved.object_id` is public: an observer who already knows an object's identity can
correlate that same object moving from Library to Hand. Do not promise that the current protocol
prevents every inference of a selected card's identity. This is a preexisting relay/protocol privacy
boundary shared by legacy and Program searches; a fix requires separate scope. See
[`PermanentMoved`](../../../libcockatrice_protocol/libcockatrice/protocol/pb/ruled_v1.proto#L2337)
and the [participant redaction path](../../../libcockatrice_network/libcockatrice/network/server/remote/game/ruled_broadcast_router.cpp#L585).
Discard retains the existing validated `DiscardQuantity` choices. Permanent filters cannot use
`DefendingPlayer` or `AttachedObject`, including inside `any_of`, and `ChoosePermanents` constraints
must be empty.
`If` accepts `ControllerLibraryEmpty`, controller `CardsInHand`, controller
`GraveyardAggregate(CardCount)`, controller-filtered `BattlefieldAggregate(Count)`, the exact
controller-Creature `BattlefieldAggregate(MaximumPower)` form with a positive `min`, no `max`,
default remaining filter fields, and no observed-object exclusion, plus the ability-only source
condition family described above. That source family also applies in `Choose` requirements; it does
not extend to spell Programs, conditional-search predicates, event conditions, or cross-object
matching. `AllOf` and `AnyOf` recursively combine only admitted leaves with ordinary nonempty and
bounds validation. The direct `Void` form remains a separate global turn-history predicate.
Other contexts and leaf operations fail registration.

Use `Choose` for a resolution-time branch whose cost and body belong together. Give the node a
program-unique `choice` ID, give each branch a stable ID local to that choice, and put the branch
instructions in its nested `body`. The `otherwise` block runs after an optional decline or a
declined paid branch. All sibling branches must use costs of the same free/paid class. This covers
several Oracle patterns: Waterbending Lesson offers Waterbend `{2}` before its discard consequence;
Chaos Spewer offers `{2}` before Blight 2; Ezuri pays once before two Proliferate instructions;
Icewrought Sentry pays before creating its reflexive tap trigger; and Sparktongue Dragon pays before
creating its reflexive damage trigger. Keep each real card's ordered actions and fallback in its
source-backed program; do not flatten the payment into a legacy branch effect.

One bounded outer-and-child case is admitted for Koya, Death from Above. Only the exact
`koya,_death_from_above` `triggered_01` owner may use the outer Program profile. Its triggered
ability keeps one optional target group (`min: 0`, `max: 1`, no legacy `effect_indices`); the group
binds as `Subject` to the immediately following `Exile(Chosen(Creature except Source))` instruction.
The compiler carries that operation's exact post-move object as a private zero-or-one
`ExiledObject` result, including its object ID, zone-change generation, and controller snapshot.
The immediately adjacent `CreateDelayedTrigger(PreviousEffectObject)` consumes that result for
`AtBeginningOfNextEndStep` and the exact `delayed_01` child. A completed optional-target Exile with
no target binds `None` and creates no observer; if the trigger fizzles before the Exile instruction,
there is no completed result to consume. Resolution never searches the Exile zone for a substitute.
These identity and source-departure rules follow CR 400.7j, 113.7a, and 608.2h. The observer is
created before a declared commander owner receives the state-based command-zone choice (CR 704.6d,
903.9a); once that choice moves the object to Command, the captured Exile incarnation cannot be
returned from there.

The child is compiled under the full parent-and-child ability identity. Its exact exiled object is
an inferred `CapturedTriggerObjectRef` input, distinct from the current/last-known copy snapshot
used by `CreateTokenCopies(TriggerObject)`; authors do not declare this input. The admitted child
leaf is exactly
`ReturnTriggeredCard(ExactTriggerObject, from: [Exile], destination: Battlefield, controller: Owner)`
with the default untapped entry behavior and no entry counters or type-setting replacement. Ordinary
supported program instructions may compose before, inside, or after that return; a registry-backed
`Return`→`GainLife` fixture verifies that a following instruction resumes. The child program retains
its cursor across Clone's entry-copy choice. `ReturnTriggeredCard`
also serves Abnormal Endurance's legacy graveyard return, but that different operation shape remains
on its existing legacy path. Other effect-created abilities remain unsupported as program owners;
Arcane Denial's distinct profile below is the only other bounded child case.
This exception fails closed for a non-adjacent producer, other delayed timings, plural or other
previous-effect subjects, modal or targetful child bodies, `may`, `once`, and maximum-use metadata;
do not generalize it to arbitrary effect-created or activated abilities. Koya's exact trigger,
target filters, and target bounds remain in its card definition and ordinary targeting validation.
The actual definition, closed owner/consumer profile, completed-empty versus fizzled execution, and
commander choice are indexed by the [Koya review map](review-maps/koya,_death_from_above.json).

Arcane Denial is a separate, closed face-spell child profile. On a nonmodal face, a direct top-level
`CreateDelayedTrigger` may own a Program only immediately after `CounterTargetSpell` with both
`unless_controller_pays` and `unless_controller_pays_by_cast_cost` set to `None`. The child has
`subject: None`, captures `affected_player: Some(PreviousTargetedSpellController)`, and uses
`AtBeginningOfNextTurnUpkeep`. It keeps the default nonmodal, untargeted, mandatory, no-intervening-if,
not-once, and uncapped metadata, with an empty legacy `effect` beside its Program. The face must have
an explicit `TargetingDef` whose group for that immediately preceding counter has exactly one
`StackSpell` binding and `min: 1, max: 1`; identify that group by its counter association, so unrelated
face target groups do not have to be absent. An inferred or implicit group, a different role, or a
different bound fails closed.

This scoped child may require `TriggerAffectedPlayer` through the private
`captured_delayed_affected_player` owner-input descriptor, derived from the delayed trigger's captured
`PreviousTargetedSpellController`; it does not widen which ordinary triggers supply that input. The
child's stable ID is compiled as a one-segment ability path below the exact card face, and is reserved
against direct delayed siblings and same-face printed or class-bar activated, triggered, and level-up
owners. Another face may use the same local ID. Compile the child's body through the normal finite
Program compiler: do not whitelist Arcane Denial's choice, branch IDs, draw counts, or body shape.
The actual delayed_01 definition and branches all use `OracleLines([1])`, so each offered branch keeps
the printed choice-line presentation. Twincast copies use the same face/child owner binding and retain
the copied spell's own captured target controller; no physical source-object lookup or card-named
runtime route is involved. Arcane Denial's separate `delayed_02` remains its existing legacy
`CreateDelayedTrigger` with `OracleLines([2])`. The reviewed behavior follows CR 503.1, 603.3b,
603.7a-b/d, 121.2 and copy rule 707.10; it adds no wire, relay, or client operation.

`Blight` as a `Perform` leaf is resultless in a program. It may suspend for its normal controller creature choice and
for applicable counter-replacement choices, but it cannot bind or yield an output. Once those
choices finish, execution continues at the next instruction in the same program block. If the
controller has no creature, it records Blight without a creature prompt and continues immediately.

Targetful leaves currently accepted are `DamageTarget` with role `Target`, `CounterTargetSpell` with
role `Spell`, `CounterTargetAbility` with role `StackAbility`,
`CreateTokenCopies(Chosen(...))` with role `Source`, and
`ReturnToOwnersHand(Chosen(...))` with role `Subject`. These are the only target-role signatures
accepted in authored programs. `CounterTargetAbility` has one explicit group with bounds `0..1` or
`1..1`; its optional bound result may be discarded. A copied permanent source requires exactly one
target (`min: 1`, `max: 1`).

Ability programs use the ability context. A triggered program may use `AffectedPlayer` for a
draw/discard recipient or as a `Choose` chooser only when its trigger supplies that event-time
player; registry validation requires the captured input before instruction zero. For `Draw`, a
triggered program may also use `EachOtherPlayerThanAffectedPlayer` with that required captured
player. Activated programs have no trigger input and cannot use these event-player forms. If the
captured chooser has left, the pending decision delegates with a fresh per-deciding-player prompt
ID; this does not change the ability controller or a compiled payer. Effect-created activated or triggered
abilities cannot own authored programs (including modal program bodies) except for the Koya and
Arcane Denial child shapes specified above. Do not author targetful activated abilities with `ChosenOpponent` target
groups from Hand or Graveyard: the opponent-choice activation path is
available only for battlefield sources, and model/registry validation rejects this authored shape.
This boundary is specific to opponent-selected groups; direct controller-chosen target groups are
not rejected solely because an activated ability's source zone is Hand or Graveyard. Legacy
abilities retain their existing validation path.

`CreateTokenCopies(source: TriggerObject)` consumes the captured `TriggerObservedObject` input.
Flameshadow Conjuring uses it as the typed token-batch producer for its shared Haste and
delayed-exile cohort; Molten Echoes uses the same mandatory three-instruction body behind its
linked creature-type trigger filter. Mirrorworks uses the observed entry object but keeps its copy
result resultless. Registry admission requires the exact triggered ability owner to observe a
permanent entry; runtime checks the captured reference and its matching current or last-known copy
snapshot before instruction zero and each program-unit resume. Non-entry triggers and activated
ability owners do not supply this input.

Mimic Vat's `create_copy` owner is the only admitted Program use of
`CreateTokenCopies(count: Fixed(1), source: LinkedExile(producer_ability_id: "imprint",
linked_exile_id: "imprinted_card"))`. It binds its `TokenCopyBatch` result to both `Haste` and
the next-end-step delayed `Exile` consumer; either consumer must name that same result. The producer
result may instead be discarded or bound under another local name, but consumers still require a
typed input. This owner-specific compiler admission does not generalize to other linked-copy cards.
Empty linked exile yields no copy and registers no delayed observer. The matching flat-list
Haste and delayed-Exile consumers of `PreviousEffectObjects` are retired; ordinary flat token-copy
lists remain supported for noncohort consumers.

Declare each target role in `CardProgram.target_bindings` by its stable instruction `id`, role and
zero-based `target_group_index`. Target groups remain in the owner's `TargetingDef`; use an empty
`effect_indices` list for program groups. Every targeted instruction role must bind exactly once,
and every group must reference at least one role. A target group intersects its roles' filters just
as a legacy group intersects its effects; use separate groups when distinct instructions have
different requirements, including targetful leaves in separate conditional branches. All groups
for the owner are announced and revalidated even when a branch is not selected. Mode groups are
local to their mode, not flattened across sibling modes.

For example, Calamitous Tide's return instruction uses its optional group 0; the following draw
and discard steps have no target binding:

```ron
spell_program: Some((body: (steps: [
  (id: "return_creature", kind: Perform(ReturnToOwnersHand(
    subject: Chosen((kind: Creature))))),
  (id: "draw_two", kind: Perform(Draw(who: Controller, count: 2))),
  (id: "discard_one", kind: Perform(Discard(who: Controller, quantity: Exact(1)))),
], yields: []), target_bindings: [
  (instruction_id: "return_creature", role: Subject, target_group_index: 0),
])),
targeting: Some((groups: [(min: 0, max: 2, prompt: "Choose up to two target creatures", effect_indices: [])])),
```

The current target-history profile is Face-only: only a nonmodal spell-face `ProgramOwnerId::Face`
may use it. Modal spell `FaceMode` Programs are unsupported because `ChosenTarget` history reads the
flattened stack `targets`, while mode-local targets live in `chosen_modes`; they fail the Program
condition allowlist with diagnostic prefix `If condition uses context outside`. For a face-owned
spell, the profile is intentionally specialized to a condition that reads the cast-time target after
a chosen-Creature Exile:
`If(ObjectWasDealtDamageThisTurn(ChosenTarget(0, 0)))` is admitted only with one required singleton
Creature group 0, exactly one unlinked `Exile(Chosen(Creature))` bound as its `Subject`, and the
condition after that Exile. The existing stack target record supplies the original ObjectId and
zone-change generation, so this profile adds no mutable capture state and does not re-resolve by
ObjectId after the zone change. Other condition shapes, target indices/groups, optional or
multi-target schemas, linked Exile, and mismatched or missing bindings remain unsupported.
`Shredder's Technique` and `Calamitous Tide` are real
spell-Program analogues for binding a chosen Subject to a target group; neither broadens this
history-condition profile. Sold Out is the current use of the history query, so keep this admission
narrow until another card has a reviewed mechanics case. Its direct `CreateTokens` rider retains the
ordinary token-operation admission. See the [Sold Out review map](review-maps/sold_out.json) for the
source-line mapping and semantic evidence references.

`bind` names an operation's typed result. `input: Some("name")` consumes an earlier in-scope
result: untap requires a permanent selection; library-top placement requires an ordered hand
selection. There is no implicit "previous instruction" input. Bindings cannot be shadowed or
read forward/across sibling scopes. Each branch explicitly lists its `yields`; both arms must
yield compatible types, and an `If` binding exports their joined result to its parent.
Conditions are evaluated on entry and are not reevaluated after a private choice resumes.

`IfResult(CardResultCount(result: "name", min: ..., max: ..., card_type: ...))` checks a named,
earlier in-scope `CardResultCohort` or imported `CastPaymentCohort`. It has ordinary optional
minimum/maximum bounds: at least one bound is required, and a bound cannot exceed the producer's
declared cardinality. Optional `card_type` matches the result's recorded `matched_card_types`;
runtime does not classify the current card after it moves. Discard-payment receipts retain their
action-time type knowledge. A bound `Sacrifice(Source)` receipt captures the source's effective
characteristics, including supertypes needed by `BasicLand`, before any selected permanent moves,
then keeps its committed Battlefield movement provenance, action, ObjectId, generation and
instructed player. The predicate has no ambient `PreviousEffect` or `Payment` source or
action/player selector. `Sacrifice(Source)` in an ability Program, `Mill`, and Grab the Prize's
imported discard are typed producer examples; this compiler surface does not establish complete-card
readiness for other owners. The flat `ChooseResolutionBranch` `CardResultCount` requirement was
retired on 2026-10-20 after its card owners moved to typed Programs. Named Program
`ProgramResultCondition::CardResultCount` and `CountExpression` result-count consumers remain live;
this retirement covers that branch requirement only.

`ChooseLinkedExiledCard` has one authored Program shape: a mandatory (`optional: false`)
ability-owned `Perform` leaf with destination `OwnerGraveyard`. The result slot is a typed
`LinkedExileCardResultCohort` keyed to the exact `producer_ability_id` and `linked_exile_id`, with
cardinality 0..1. When a player accepts an offered card, the engine records that exact selected
object's pre-move `ActionFacts` and effective type snapshot, then attempts the move to its owner's
graveyard. A following `CardResultCount` reads the selection facts, not a guessed result from a
successful destination move; if a replacement changes the attempted destination, the selected
card's recorded type still drives the printed type condition. An empty eligible cohort completes with
an empty result. The linked key retains the physical source ObjectId and zone-change generation,
producer identity, link ID, and occurrence.
The chooser sees only current linked records that still match the filter and exact live generation.
For this Program profile, candidates must be face-up in Exile when first offered, when a saved offer
is refreshed, and when an answer is validated. A prior Hideaway viewer's private look permission
never grants a delegated chooser access; registry admission rejects a Hideaway producer for the
matching mandatory Program consumer.

The handwritten [Currency Converter definition](../data/currency_converter.ron) and
[review map](review-maps/currency_converter.json) pin its three Oracle lines to the actual-card
scenario coverage. The first triggered-ability owner, `discard_to_exile`, uses a separate finite
Program profile: the discard trigger supplies its exact captured event object to one optional,
free Controller `Choose`; its sole `EffectsApplicable` branch performs only linked
`Exile(TriggerObject, "exiled_cards")`, and its `Otherwise` block is empty. The compiled
`TriggerLinkedExileObject` requirement admits only an event that supplies that object (or an
explicitly captured trigger-object reference), and the choice checks the existing object-generation
and zone rules before offering the branch. For this Currency trigger, that means the exact discarded
card must still be the same graveyard incarnation. An empty/declined choice makes no linked record.
This does not migrate `draw_then_discard` or alter the later `return_linked_card` consumer. Its
compiler profile and exact linked subject are covered by
`currency_discard_to_exile_program_admits_only_its_exact_linked_exile_choice` and
`linked_exile_program_input_requires_an_exact_event_object_source`; actual-card tests cover
multi-card identity, replacement, decline, stale generation, and accepted-command replay. The
source-owner departure scenario has the Converter owner leave while the other player controls and
resolves the already-begun discard ability. CR 800.4a removes the departed owner's permanent; CR
608.2m lets the begun ability finish. This checks source removal and retained last-known trigger
context, not chooser delegation under CR 800.4g.

Mimic Vat's `imprint` owner admits one separate exact receipt profile: its optional Controller
branch exiles only the captured `TriggerObject` under `imprinted_card`, then immediately feeds the
typed 0..1 `TriggerObjectLinkedExileReceipt` to `ReturnOtherLinkedExiledCards` for that same owner
and link. The receipt identifies the newly linked event incarnation so the return skips only that
object. This does not expose a general TriggerObject-Exile result slot or permit another consumer,
link, instruction, or owner. The compiler also records a private optional-cost departure policy for
this exact profile; although the choice has no separate `ResolutionCost`, a departed Controller has
not paid the optional action cost and takes `Otherwise` under CR 800.4f (an inference from that
rule). Ordinary non-cost Program choices continue to delegate under CR 800.4g. The Mimic Vat
compiler and scenario evidence is indexed by its [review map](review-maps/mimic_vat.json).

The begun mandatory choice keeps its original ability-controller attribution and
Program cursor through the narrow active/saved departure path for this profile. When that ability
has begun resolving, CR 800.4a makes its departed controller's noncard stack object cease to exist;
CR 608.2m requires the already-resolving ability to finish. The retained Program is execution state,
not a new stack object. A source leave does not rebind the begun occurrence to a later source
incarnation. If the chooser has left, CR 800.4g lets the ability controller choose a replacement
chooser. If that controller has also left, this runtime applies CR 800.4h so the next live player
makes that selector choice. This two-step selector policy is an explicit engine interpretation, not
an example stated by the rules.
Delegation changes who supplies the resolution choice, not the ability-controller attribution or
result's recorded card owner. If every offered card is no longer available, the instruction completes
with an empty cohort. A later `CreateTokens(..., who: Controller)` instruction does not create a
token for the departed original controller (CR 800.4b). This is specialized to the existing Currency
Converter return ability and does not widen other linked choices or general ability-departure
behavior. Nautiloid Ship's optional `ChooseLinkedExiledCard` return-to-controller-battlefield path
remains unchanged. `PlayLinkedExiledCard` remains a separate legacy path for Hideaway play
permissions, with one exact Program exception: Mosswort Bridge `activated_02` has a root body with
one optional `Choose` by `Controller`, one branch requiring `GameCondition(BattlefieldAggregate)`
over `Controller` creatures with `TotalPower` at least 10 and no maximum, and a branch body with
exactly one terminal resultless `PlayLinkedExiledCard` configured for `Play`, no filter, and no
source return to its owner's hand if cast. The aggregate filter must have its default exclusion
fields. The Program has no root tail or yields, no alternate or nested branch, no trailing branch
instruction or yields, and no target binding, bind, input, or yield. This optional non-cost choice
uses `DelegateNonCostChoice`; it does not use Mimic Vat's optional-cost departure policy. Other
optional linked choices, linked-play configurations, and destinations remain excluded. The terminal
shape is required because linked-play completion closes the Program frame instead of resuming it.

`Mill` may bind a typed `CardResultCohort`. In a spell or triggered-ability program, `ChooseCard`
accepts two exact source shapes: `WholeCurrentGraveyard`, which searches the controller's current
graveyard without an input, or `CardResultCohort(players: ...)`, which requires
`input: Some("earlier_mill")` bound to an earlier `Mill` in scope. The cohort form is exactly that
Mill instruction's committed output, including when empty; it does not fall back to cards already
in the graveyard. `ActionFacts` entries from other actions are not movement evidence and cannot
satisfy this selector. Current CardResult bindings for this source are limited to Mill.

The filtered picker considers only exact, still-live, face-up public output incarnations in the
graveyard, battlefield, exile, stack or command zone. Hidden destinations and `UnknownHidden`
outputs are ineligible and never appear in the candidate projection. The selector checks current
rules characteristics; battlefield continuous effects apply, while `printed_power` remains a
printed characteristic. A stack candidate must be the exact physical spell cast, not a copy or
ability. `ChooseCard` currently accepts only `Hand`, `LibraryTop` or `LibraryBottom` destinations in
a spell or triggered-ability program. Commander replacement uses the commander's owner's choice
and the same movement path.

```ron
(id: "mill_four", bind: ["milled"], input: None,
  kind: Perform(Mill(count: 4, who: Controller))),
(id: "return_milled_land", bind: [], input: Some("milled"),
  kind: Perform(ChooseCard(
    source: CardResultCohort(players: Controller),
    filter: (card_type: Some(Land)),
    destination: Hand,
    optional: true,
  ))),
```

`WholeCurrentGraveyard` remains a separate, explicit source. Do not replace it with the Mill cohort
when an effect allows any currently matching graveyard card, or reverse that substitution when the
Oracle instruction limits the choice to cards moved by one specific instruction.

Combustible Gearhulk uses a deliberately narrower targeted-choice profile. `Choose` may use
`chooser: TargetedPlayer(OpponentPlayer)` only when it is optional and every branch has `cost: None`.
Its compiled target binding pairs exactly one `DamagePlayer` with that same singleton opponent
target group. The group is resolved recursively through nested blocks and through the owning
ability or mode's local target context. In `Otherwise`, `Mill(who: Controller)` must bind a named
result; the immediately following `DamagePlayer` in that same block must read it with
`input: Some("milled")` and `Count(PreviousMillManaValueSum)`. No wrapper, ambient previous-result
read, intervening instruction, alternative player recipient, or paid targeted-chooser form is
accepted. Delegating the pending choice after target departure changes the chooser only. The
original target remains the damage recipient, and the resolving trigger retains Gearhulk source LKI
after source departure (CR 113.7a, 608.2b, 608.2h, 608.2m, 800.4a, 800.4g). Its actual-card scenarios
cover the exact Mill receipts, short-library draw branch, Narset's controller-scoped draw, target
departure, delegated replay, and ordered prevention. Compiler regressions also reject paid Mana and
Waterbend branches, non-immediate or wrapped Mill inputs, and mismatched target groups. The model
tests `program_compiler_accepts_gearhulk_targeted_choose_and_damage_group`,
`program_compiler_recursively_accepts_gearhulk_targeted_choose_in_otherwise`,
`program_compiler_accepts_gearhulk_typed_mill_input_for_targeted_damage`,
`program_compiler_rejects_unbound_or_wrapped_gearhulk_mill_sum`,
`program_compiler_rejects_invalid_gearhulk_target_group_contracts`,
`program_compiler_rejects_paid_mana_targeted_optional_choose`, and
`program_compiler_rejects_paid_waterbend_targeted_optional_choose` cover the compiler contract.
The controller loses when the chosen draw-three instruction attempts to draw from a library with
fewer than three cards (CR 121.2, 104.3c).
This does not change existing `Controller`, `AffectedPlayer`, or `SourceController` chooser profiles;
other paid or targeted-player forms remain outside this subset. Ordinary Mill receipt coverage does
not establish a general Mill zone-replacement contract; Rest in Peace is not covered.

`Discard` produces a typed result containing committed discard batches, captured card-type
knowledge and a separate outcome. `IfResult` can inspect an earlier in-scope discard binding:
`DiscardCount(result: "discarded", players: Controller, card_type: Some(Nonland), min: Some(1))`
or `DiscardOutcome(result: "discarded", outcome: ChosenToPay)`. Counts use committed receipts;
an unrevealed discard into a hidden destination has unknown characteristics and cannot satisfy a
card-type predicate. A public madness exile retains known characteristics. Results become usable
only after the discard transaction, replacement choices and chosen ordering finish.

`Discard(quantity: OptionalExact(2))` offers either zero or exactly two eligible cards, with the
same rule for any positive literal. The server rejects a partial nonzero answer before mutation.
An empty answer yields `Declined`; too few eligible cards yields `Unpayable` without a prompt.
An exact choice yields `ChosenToPay` independently of replacement destinations or receipt count.
Use that outcome for a subsequent "if you do" instruction. Mandatory discard yields `Performed`;
do not infer optional payment from a nonempty receipt or an ordinary `UpTo` selection.

`CounterTargetSpell(unless_controller_pays: Some(amount))` exposes one optional typed result only
when `amount` is a positive fixed literal. Bind it to consume the accepted payment outcome with
`IfResult(CounterPayment(result: "payment", outcome: Paid))`; the status type also supports
`Declined`, `NotOffered` and `ForcedUnpaid`. `Paid` is published only after the payer accepts and the payment
transaction commits. `Declined` records the accepted decline even when the target spell cannot be
countered. `ForcedUnpaid` records an already-offered payment whose payer leaves before answering;
after departure cleanup the engine consumes that standalone choice and resumes the retained
source's parked cursor without delegation or an accepted decline. `NotOffered` records a completed
counter instruction that begins without its captured
spell still on the stack, including when an earlier instruction removed it or its zone-change
generation no longer matches. It also records a target-group skip when another legal group keeps
the program resolving. A failed/unpayable Pay submission leaves the existing offer pending and
publishes no result, and the all-targets-illegal exit still bypasses the program. Binding is optional
so plain counter leaves and existing fixed-cost counter leaves that do not consume the result remain
valid; `unless_controller_pays: None` has no output. Programs continue
to reject `unless_controller_pays_by_cast_cost`, which needs cast-time receipt inputs outside this
typed result contract. Conditions must read a dominating in-scope binding of the matching result
type; missing, forward and wrong-type references fail compilation.

`CounterTargetAbility` can bind an optional `CounteredAbilitySource` result (`0..1`), which stores
the exact source ObjectId and zone-change generation only when the targeted stack ability was
actually countered and had a permanent source. `IfResult(CounteredAbilitySourceMatches(result:
"name"))` is false for a completed counter with no receipt; otherwise it checks that the captured
incarnation is still a battlefield permanent and currently an artifact, creature, or planeswalker.
This follows Tishana's Tidebinder's ruling that the source types are checked as its triggered ability
resolves. Its narrow typed consumer is `RemoveAllAbilities(subject: PreviousEffectObject, duration:
WhileSourceOnBattlefield)` in an ability Program, with `input: Some("name")` bound to that receipt.
The former implicit flat-list `PreviousEffectObject` route is rejected recursively; other flat
`RemoveAllAbilities` subjects remain valid, including The Wondrous Wasp's chosen-creature effect.
Teferi, Temporal Pilgrim's `activated_01` and The Wondrous Wasp's ETB exercise the reusable
`StackAbility` role with real activated and triggered abilities, respectively. The countered-source
receipt is a narrow composition for Tidebinder, not a general-purpose stack-object receipt.
The card's current-characteristic and duration behavior follows CR 608.2b-c and 611.2b; a returned
source is a new object under CR 400.7. No protocol or client targeting contract is introduced.

For example, the land-choice tail of Frantic Search is:

```ron
spell_program: Some((body: (
  steps: [
    (id: "choose_lands", bind: ["lands"], kind: Perform(ChoosePermanents(
      chooser: Controller,
      filter: (kind: AnyPermanent, permanent_types: [Land]),
      min: 0,
      max: 3,
    ))),
    (id: "untap_lands", input: Some("lands"), kind: Perform(UntapChosenPermanents)),
  ],
  yields: [],
))),
```

This fragment still needs the preceding draw/discard instructions and complete-card evidence.
[Brainstorm](../data/brainstorm.ron) demonstrates fixed draws followed by private ordered hand
selection and bound library-top placement; that body also fits Brainstone and Jace, the Mind
Sculptor's draw/put-back ability, without admitting their other mechanics or ability contexts.
Selection does not move cards; the placement instruction uses generation-bound selected identities.
The two hand-selection/placement leaves are program-only and are rejected in ordinary effect lists.
The retired `DrawDiscard` effect and its legacy source-lowering path are no longer accepted. Express
an ordered sequence as separate `Draw` and `Discard` program instructions, adding a typed
`IfResult` branch when later actions depend on the discard outcome. The finite authored face, mode,
activated-ability and triggered-ability programs, including the target signatures described above,
are admitted.

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

`CreateTokenBatch(tokens: [...])` creates one of each distinct registered token simultaneously
for the resolving controller (Triplicate Titan, Bestial Menace). It requires at least two IDs;
declare every member in the review map's token dependencies. Separate `CreateTokens` instructions
retain their separate event boundaries. Before entry commits, timestamp-order candidates carry
complete recipient-private token characteristics; the image picker keeps readable variant labels
when the external token artwork is unavailable. Public `TokenCreated` supplies physical identity
only after replacement and ordering choices finish.

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

For activated abilities, `SacrificePermanent(filter: ..., count: 3)` represents one simultaneous
three-permanent sacrifice instruction (Kuldotha Forgemaster). Count defaults to one, is omitted
when serializing singleton costs, and must be positive. Bolas's Citadel demonstrates the same
contract with ten nonland permanents. Use separate costs only for separate authored instructions;
repeating three singleton costs does not represent one sacrifice-three instruction. Plural
activation payments use the existing generation-bound battlefield cohort and explicit confirmation.

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

Resolution-time branches use typed Program `Choose` steps, with each branch's `presentation`
mapped to the printed line. See [Finite authored programs](#finite-authored-programs).

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

The following list examples describe existing `spell_effect` syntax. Within a supported
`spell_program`, use explicit `Perform` instructions and typed bindings as described in
[the current program subset](#finite-authored-programs); do not insert `DrawDiscard`
as a program leaf.

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

Use a direct Program `Perform(LookChooseToHand(count: 2, filter: None, min: 1, max: 1,
reveal: false, bottom_order: Chosen))` for Sleight of Hand's mandatory private selection.
Flow State uses a `ProgramIf` to select the exact two-card or one-card pick after checking the
controller's graveyard for both card types at resolution. The Program-only shape and its limits
are specified above. These cards are put into hand, not drawn; the owner orders any multi-card
remainder. Existing effect-list callers retain filtered, revealed, optional, and random-bottom
forms, which are not admitted as Program leaves.

1. Name two real cards or two mechanics supported by the proposed shape.
2. Put the variant in the appropriate `tricerules-card-model/src/primitives/` module.
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

## 7. Keep card behavior data-driven

Do not add card-specific Rust resolution handlers. Describe behavior with supported
`spell_effect`, `modal_spell`, or typed `spell_program` data and widen an engine primitive when a
second real card or mechanic needs it. The legacy `custom_effect` field remains parseable only so
registry validation can name the unsupported key; any non-empty value rejects that card before
registry admission.

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

When useful, scaffold the clerical source fields for a selected card from
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

For the direct-RON route, keep incomplete drafts outside `data/`. A complete copied definition may
be edited directly in `data/` during the normal red/green workflow; no scaffold/promotion ceremony
is required. Create a version-1 JSON review map. Select one pinned source identity with either
`oracle_id` or `exact_name`. Cover every
normalized Oracle line exactly once with either `typed_paths` (JSON pointers into the validated
runtime-shaped definition) or a nonblank `unresolved_reason`. Primitive references are optional
navigation hints: omit routine symbol/path inventories already evident in the typed definition.
List every referenced token ID and semantic fixtures. The tool does not infer these mappings.

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

Follow the [execution path](../../../docs/AUTHORING-WORKFLOW.md) for preparation, frozen semantic
review and delivery. Commands, worker settings, metadata Refresh/Check and final affected-side
gates are owned by [AGENT-VERIFICATION.md](../../../docs/AGENT-VERIFICATION.md).
Use focused red/green checks during iteration and one final full gate on stable reviewed content;
reuse valid evidence. Reusing mechanics does not waive actual-card semantics or applicable acceptance.

### Reusable semantic evidence

Keep four contracts separate: primitive behavior, composition (for example, an ETB draw or a
selected modal effect), each card's reviewed semantic mapping, and unusual interaction regressions.
Registry shape checks alone do not execute effects; successful command admission alone does not
prove resolution. New successful generic conformance cases need no baseline entry. Unclassified
fixture gaps, execution failures and changes to existing reviewed baseline rows still fail.

Reusing mechanics does not require a new bespoke scenario function for every card. For the same
tested pattern with different parameters, prefer adding the actual card to a shared scenario with
independent expected values. Reuse existing primitive and unchanged interaction regressions rather
than duplicating their full test matrix. For new combinations or meaningful differences, add focused
coverage of the affected behavior; a dedicated scenario is needed only where the shared fixture
cannot prove it. A new card must still execute with all relevant clauses asserted: copying a RON
definition or passing registry-shape checks alone does not satisfy this requirement.

This reduces repeated test authoring, not the required verification gates. Keep focused red/green
checks, independent semantic review, applicable illegal-path coverage and the final affected-side
gate. Do not remove existing regression coverage or manufacture a generic test abstraction for
hypothetical future cards.

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

### Offline authoring workbench

Use `scripts/card-author.ps1` for discovery and clerical preparation. It reads current RON through
the runtime validator, builds structural search features in memory, and searches the local pinned
Oracle bulk when requested. There is no hand-maintained feature catalogue to update. Output is
readable JSON with paths, structural features, source text when loaded, and existing review-map
test references. Scores are advisory; they do not prove complete support or replace rulings.

```powershell
# Structural analogues for an implemented card (actual effect/target/cost trees).
./scripts/card-author.ps1 search --like divination --limit 5
# Search typed vocabulary, or rank implemented cards against an exact local Oracle name.
./scripts/card-author.ps1 search --query "Destroy Creature" --limit 5
./scripts/card-author.ps1 search --name "Exact New Card Name" --limit 5

# Copy a reviewed analogue into a NEW directory outside embedded data/.
./scripts/card-author.ps1 clone --from divination --name "Exact New Card Name" --out build/new-card-draft
```

Clone updates identity and printed characteristics from the SHA-verified local source, preserves
the analogue's typed mechanics, and emits `.draft.ron`, an **unconfirmed** review map, `source.json`
and `differences.json`. The report includes source-field differences, copied-file hash and existing
test references. No semantic expectations or approval are copied. Both unresolved sentinels prevent
normal validation until the author finishes mechanics and presentation. Review self-references,
costs, targets and all changed clauses; inherited presentation mappings may be wrong for the new
Oracle lines. Never interpret copying as approval. Output collisions and embedded-data destinations
are rejected. The initial clone path supports normal single-face cards with numeric printed stats;
use existing scaffold/manual authoring for other layouts, variable stats and color indicators.
Search itself includes multi-face definitions. `--bulk` selects a different already-local snapshot;
it must have matching adjacent metadata and does not download or update anything.

#### Reusable rows and draft execution

The checked-in [`authoring_rows.json`](../../tricerules-core/tests/scenario/authoring_rows.json)
demonstrates independently specified spell/ETB/modal draw and single-target destruction families.
Mana arrays are `[W,U,B,R,G,C]`. Draw rows assert exact hands/libraries, source destination and
generation, recipient, selected mode and Food count. Destruction rows prove illegal-target rejection
without state change, the chosen opposing object's destruction, unaffected bystanders and physical
identity. These families cover only their named behavior. Add a dedicated scenario for other clauses,
choices, unusual timing or interactions; do not broaden an assertion-free generic command DSL.

For a new card, edit its draft outside `data/`, resolve both sentinels, and write a batch JSON file:

```json
{
  "drafts": ["new_card.draft.ron"],
  "rows": [
    {"family":"draw", "card":"new_card", "mana":[0,1,0,0,0,2],
     "surface":"spell", "recipient":0, "count":2, "food":0}
  ]
}
```

Draft paths resolve relative to that JSON file. Expectations must be authored from reviewed source
semantics, never inferred from the draft. Then run:

```powershell
./scripts/test-card-drafts.ps1 -BatchPath build/new-card-draft/batch.json
```

This builds the optional authoring harness once and reads fresh draft RON and row JSON on each run.
Editing those external files does not recompile embedded card data. Each draft must have an executed
row; empty batches, malformed schemas, unresolved drafts and duplicate IDs fail. The explicit
authoring constructor uses the same engine/registry validators, without changing the global registry
or any production startup path. The harness does not prove client presentation or Oracle correctness.
New token definitions and replacements for already-implemented IDs are outside this initial overlay
path; use normal authoring/testing for those cases.

After review, promote the finished RON/map, add its independently reviewed rows to the checked-in
matrix (or dedicated scenarios), and run preparation plus the ordinary full embedded Rust/CardData
gate. A draft-only pass never substitutes for that gate or independent semantic review.

#### Small research queue

Save completed research notes with the actual source/rulings, analogue and fixture dependencies:

```powershell
./scripts/card-author.ps1 queue-save --name "Exact New Card Name" --note "Copy from X; differences Y; remaining checks Z" --depends build/new-card-draft/source.json --depends path/to/rulings.txt --depends path/to/analogue.ron --depends path/to/fixture.rs --out build/new-card-queue.json
./scripts/card-author.ps1 queue-check --entry build/new-card-queue.json
```

Entries fingerprint those inputs plus engine/schema/helper sources, Cargo.lock and local bulk
metadata. Directory dependencies include membership, so additions/deletions also invalidate them.
Missing inputs are stale. Unrelated commits and unrelated card additions do not invalidate research
merely by changing HEAD. Include every additional source/token/fixture dependency actually used.
`fresh: true` means only that recorded inputs match; it is not implementation readiness or semantic
approval. Recheck corpus membership, ownership and unrecorded assumptions before selecting a batch.
Keep just a few compatible prepared candidates; do not turn queue upkeep into a per-card gate.

For prepared multi-card packets, early structural readiness, expanded semantic row families,
frozen review bundles and phase timing, see [BATCH-WORKBENCH.md](BATCH-WORKBENCH.md).

Workbench wrappers honor ambient Cargo jobs and Rust test threads. Existing required gates remain
unchanged. Source availability, narrower clone/family support or a stale queue never authorize
guessing missing mechanics, broadening admissions or waiving verification.

### Batch evidence and review discipline

#### Short path for reuse-only batches

Use the [routine reuse-first execution path](../../../docs/AUTHORING-WORKFLOW.md#routine-cards-reuse-first).
The exact map/reference requirements below remain authoritative. Routine batches need no separate
issue, narrative dossier, optional index upkeep or pre-review generated packet. Keep one map per
handwritten card; optional primitive inventories/empty token arrays may be omitted. Successful new
conformance cases require no manual baseline rows. Investigate exceptions rather than accepting them.

For pure reuse, unchanged protocol, client presentation and interaction paths need no repeated
GUI demonstration solely because a new card was added. Trace those surfaces and state N/A with a
reason; new or changed visible behavior still requires its relevant acceptance evidence. Never
claim that generic conformance, copied RON or a successful metadata refresh proves card semantics.

If source access fails with a sandbox socket denial, use the host's supported permission path for
the same narrow read. Distinguish that from a real HTTP response. Do not spend repeated research
attempts treating a local network restriction as a missing mechanic or external service outage.

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
Semantic references use `scenario module::test` for tricerules-core or `suite::module::test`
for tricerules-cards. Card registry tests share five explicit Cargo suites under `tests/suites/`:
`deck_registry`, `generated_registry`, `batch_registry`, `mechanic_registry`, and
`standard_registry`. Add a new test source to exactly one suite with a `#[path]` module;
the suite-membership regression rejects omitted or duplicated sources. Keep card assertions
in their existing source modules rather than adding another executable per card.
Load shared helpers once in the suite harness; cases import `crate::common` rather than
declaring `mod common` repeatedly inside the same executable.
For example, run one exact registry regression with
`cargo test -p tricerules-cards --test deck_registry deck_coverage_duplicant::duplicant_has_its_exact_printed_identity -- --exact`.
The gate resolves exact Cargo identifiers and rejects missing or ignored
tests. Canonical CardData Check includes it. Listing is not proof of execution: the full Rust suite
must pass on the same content, and review must confirm the cases exercise the named cards and
clauses. Draft packets may still describe planned fixtures before tests exist.

Use the current campaign model/effort policy for implementation and independent review. Run formatting and focused lint before freezing the patch.
Resolve blocking defects and missing required evidence before the final full gate. Optional polish
may be deferred without another review cycle. Changes after a passing gate require affected
reverification; avoid optional changes after that gate. Reuse unchanged passing evidence.
Use focused/package checks during iteration, then the full affected-side gate on stable content.
Do not routinely run the full suite before review and again inside the final gate. Broaden testing
early only when failures, changed contracts or concrete unresolved risks justify it. Keep all
required final checks; a later delivery request alone does not invalidate passing evidence.

The root owns all command execution that can write artifacts, including tests, builds, Cargo,
formatting, verification, generators, metadata checks and review packets. Independent reviewers
inspect the frozen patch and existing evidence using the [reviewer contract](../../../docs/CARD-REVIEW.md).
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
- [ ] The actual card executes in shared or dedicated scenarios with independent expectations for every relevant clause, applicable illegal paths and prompt/choice behavior; unchanged primitive regressions are reused.
- [ ] Any optional index entry relied on was checked against current evidence and corrected or qualified if stale. No new entry, hit/miss report or unrelated catalogue upkeep is required for routine authoring.
- [ ] Genuine deferrals are recorded in `partial-cards.tsv`, not runtime RON.
- [ ] `CARDS.md`, generator checks, Rust gates, and `git diff --check` pass as applicable.
- [ ] The final report includes the governed MTG concepts and compliance or deferral note.

### Token copy sources

`CreateTokenCopies(count: 1, source: Chosen((kind: Creature, controller: You)))`
uses one chosen permanent target (Cackling Counterpart). `source: Source` is untargeted
and requires a battlefield ability source (Colorstorm Stallion). It uses the current
source generation or its last battlefield copy values after departure. Populate uses
the same owned snapshot and token-entry pipeline with a resolution-time choice.
`source: TriggerObject` is also untargeted; Mirrorworks uses the exact permanent observed by its
entry trigger, including that generation's last-known copy values if it has left the battlefield.
Actual Transform and ModalDfc sources produce tokens owning both faces and retaining
the active face. Copy provenance cannot turn a single-faced Clone into a double-faced
object. New copy-with-modification effects still require explicit implementation.
