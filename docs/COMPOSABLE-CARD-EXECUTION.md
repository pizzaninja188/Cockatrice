# Composable card execution and authoring contract

Status (2026-10-14): target design with a bounded authored-owner implementation increment,
cross-component resolution-prompt identity, exact compiled owner identities, immutable ability
binding, and mandatory retirement of the `DrawDiscard` compatibility family. The runner supports
targetless program operations and four targetful role signatures on spell faces, modal modes,
activated abilities and triggered abilities. The original migration census was 41 existing card
identities, 42 `DrawDiscard` bodies and three additional modal siblings: 45 compiled owners, not 45
card admissions. All 45 owners now use authored programs, and the migration added no card
admissions. The old variant, source lowerer, and draw-after continuation are removed; registry
admission rejects old `DrawDiscard` RON and requires every sibling to be authored when a modal
container uses programs. Typed discard outcomes and replacement/entry/payment continuations retain
the owning frame. Authored-owner conversions include Calamitous Tide's face, Agna Qel'a's
activation, Uthros Scanship's ETB, all three Three Steps Ahead modes, and the exact Recruit
generator pair for Long Lake Nuisance and Patient Instructor. The
[active authoring reference](../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md#finite-authored-programs)
owns accepted syntax and restrictions. Broader operations and contexts remain migration targets.
Prompt identity applies to legacy resolution choices as well as program choices; it does not widen
the accepted syntax.
The bounded 2026-10-14 Divert Disaster increment adds an optional `CounterPaymentOutcome` output
for fixed-cost `CounterTargetSpell` leaves and a typed `IfResult` condition. Its total outcomes are
`Paid`, `Declined`, `NotOffered` and `ForcedUnpaid`. `Paid` follows an accepted, committed payment;
`Declined` follows the payer's accepted decline. `ForcedUnpaid` records an already-offered payment
whose payer leaves before answering. After departure detaches any departed outer source and removes
the payer's objects, the engine consumes the standalone `ManaPayment` choice and resumes the same
parked cursor with `ForcedUnpaid`; it does not delegate or synthesize a decline. `NotOffered` means
the counter instruction began without its captured stack target still present as that exact object,
including when an earlier instruction removed it, or its target group was skipped while another
legal group kept the owner resolving. Invalid or unpayable Pay submissions remain pending and
publish no result. Divert's Lander effect consumes only `Paid`, so a departed payer does not create
one. The `CounterUnlessPaid` branch
receipt condition and now-unused `PreviousResultReceipt` wrapper are retired; the shared counter-
placement/sacrifice receipt matcher and other resolution receipts remain.
The bounded 2026-10-15 Choose increment compiles nested controller choices with homogeneous
all-free or fixed Mana/Waterbend costs, the accepted bounded requirements, typed branch yields,
and nested body/Otherwise blocks. Waterbending Lesson pays Waterbend `{2}` or discards one card;
Chaos Spewer may pay `{2}` or blights 2. Payment, Blight creature selection and counter
replacement choices suspend the same program cursor, which resumes its following instruction
once. The exact admitted syntax and fail-closed limits are in the
[active authoring reference](../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md#finite-authored-programs).
This ports only those two definitions: legacy flat branch resolution remains live for other
callers and has not been retired.
The following five 2026-10-15 Choose ports reuse that runner without runtime changes: Ant-Man's
Army's mandatory Food-or-Treasure choice, Consecrated Sphinx's optional free Draw 2, Mentor of the
Meek's optional `{1}`-for-Draw 1, Mind's Eye's optional `{1}`-for-Draw 1, and only Solemn
Simulacrum's `triggered_02` death-draw owner. Solemn's `triggered_01` ETB basic-land search remains
on its existing legacy branch; the tranche adds no card admissions.
The 2026-10-15 Wash Out port uses an all-free color `Choose` and the resultless
`ReturnAllToOwnersHand` program leaf. Mass and single-card owner-hand returns share the Commander
replacement handler, which parks the exact resolving operation through APNAP ordering and
destination choices. Library-return cohort consolidation remains deferred; its existing path is
still separate, and this port adds no card admission.
The 2026-10-16 Battlefield-search increment adds one closed resultless `SearchLibrary` form:
controller's fixed Library, one Basic Land, bounds 0..1, Battlefield tapped, shuffle, no reveal or
result. Lumbering Worldwagon's ETB and actual-attack triggers and Solemn Simulacrum's ETB use it;
Worldwagon's Crew and Solemn's death-draw program remain unchanged. The existing Library-to-
Battlefield entry batch preserves replacement choices and resumes the parked cursor after the
entry pipeline. A zero-card result still shuffles and fires the search event (CR 701.23b, 701.24e);
entry choices and a begun ability remain governed by CR 614.12, 616.1, 113.7a and 608.2m. This
increment adds no card admissions or protocol/client changes.
The bounded 2026-10-17 Embermouth Sentinel increment exercises a resultless conditional destination
on its `triggered_01`: an optional controller `Choose` offers one free branch whose Library
`BasicLand` search reveals the selected card, shuffles, and puts it on top unless the controller has
a Dragon, when it enters tapped. This is the sole currently authored card use of the accepted
conditional-search shape; compilation validates the closed operation and Dragon-count condition
independent of owner identity. The Battlefield aggregate has no Creature filter; a noncreature
Kindred Dragon qualifies under CR 109.2 and 205.3m. The condition is evaluated after the accepted
selection and fixes the destination before the existing entry-replacement pipeline, which resumes
normally (CR 701.23a, 614.12, 616.1, 608.2m). This uses the existing search and entry transactions;
it adds no core, protocol, client, or other card admission.
The bounded 2026-10-18 `SacrificePermanent` increment adds a fixed one-permanent cost only to
`Controller`-chosen ability Programs. It covers four existing owners: Servant of the Stinger's
combat-damage trigger and West Wind Avatar's enter, attack, and Disappear triggers. Every terminal
sacrifice-filter leaf must select a `Creature` or `AnyPermanent` controlled by `You`; typed Programs
reject `excluded_objects` on `any_of` branches while preserving flat exclusions within the
`source_only: false` bound. The parked occurrence revalidates current candidate identity, generation,
filter, and control before committing the shared cost transaction; optional decline or an
offer with no selectable branch follows `Otherwise`, and the paid body and parent cursor resume
once. Other legacy sacrifice callers remain on their existing path. This adds no card admission or
protocol or client change.
The bounded 2026-10-14 Mill increment adds a typed `CardResultCohort` result from `Mill` and an
earlier-result input to `ChooseCard`. The cohort records committed public output incarnations and
the chooser revalidates their live identity and current characteristics before offering them.
Midnight Tilling and Lluwen, Imperfect Naturalist's ETB use this form. Say Its Name and Fell
Gravship use `WholeCurrentGraveyard` in authored programs because their choice includes any
currently matching graveyard card, including one present before the mill. The old
`ChooseGraveyardCard`/`from_result` schema, adjacency validator, dispatch and parked continuation
are retired; no admitted definition or runtime caller uses that compatibility family. Lluwen's
activated ability that discards a land and creates Worm tokens remains unchanged. `CardResultFilter`
remains available to typed result/count expressions and other operations. The developer
`choice public-zones` fixture exercises the cross-zone candidate projection without moving a card
or changing priority.
The initial design was reviewed against `41e4ccf463930d8384ac00eb60ac21f931646b38` (2026-10-12).
The former deck campaign remains outside scope. Cards require complete-card evidence and the
applicable verification and visible acceptance gates independently of this design.

## Outcome and boundaries

A new combination of supported operations must be expressible and validated as typed card data,
including choices that suspend resolution. Adding such a card must not require another engine
continuation, card-named dispatcher outcome, or hand-maintained adjacency exception. New rules
mechanics still require engine work and independent semantic review.

Keep the Rust single writer, immutable registry, explicit layers and replacement transactions,
generation-bound identity, deterministic logged decisions, and fail-closed privacy/admission.
Retain typed RON as the authoring format. Implement a finite instruction tree with exhaustive Rust
operations; no arbitrary scripts, author-defined functions, jumps, dynamic recursion, or general-purpose
VM. Finite nested blocks are allowed; author-directed loops are not.
The legacy custom Rust escape hatch is retired: card-model registry admission rejects every parsed
`custom_effect` key. New behavior belongs in typed programs and shared engine primitives.

The design replaces composition contracts, not every rules subsystem. Combat, turn structure,
continuous effects, costs, triggers, state-based actions and replacement ordering retain their
semantic ownership. An instruction sequence never substitutes for a simultaneous event batch.

## Existing seams and concrete changes

| Legacy contract at the design baseline | Target contract |
|---|---|
| [`SpellEffectKind::validate_list`](../tricerules/tricerules-card-model/src/primitives/effects.rs) manually pairs preceding producers and consumers | One model-side program compiler checks operation signatures, typed bindings, scope, cardinality and context |
| `Conditional` admits selected single instructions and rejects nesting | Finite nested conditional blocks share the runner; conditions are evaluated on entry |
| [`run_effect_list_with_previous`](../tricerules/tricerules-core/src/engine/resolution/mod.rs) rebuilds flattened branches and advances a flat index | An immutable compiled program plus explicit block/cursor frames owns sequencing |
| [`EffectResult` and `ParkedStackResolution`](../tricerules/tricerules-core/src/state.rs) carry one implicit previous result | Private typed immutable result slots, an in-flight operation and a saved frame stack |
| [`DrawCompletion`](../tricerules/tricerules-core/src/engine/draw.rs) includes `BeginDiscard` and `BrainstormPutBack` sequencing | Draw completion resumes the owning operation/frame; follow-up card actions are instructions |
| [`RecipeEmission`](../tricerules/tricerules-cards/src/bin/gen_cards/recipes.rs) and exact-text matchers assemble separate special combinations | Reviewed parameterized templates emit the same typed programs as handwritten RON |
| [`SubmitResolutionChoice`](../libcockatrice_protocol/libcockatrice/protocol/pb/ruled_v1.proto) answers whichever prompt is current | Every answer names the exact active `resolution_prompt_id` |

This is semantic consolidation. Moving existing branches into smaller files is insufficient.
The previously shipped shared execution work is a foundation to reuse, not proof this contract
already exists; see live issues [#381](https://github.com/pizzaninja188/Cockatrice/issues/381),
[#380](https://github.com/pizzaninja188/Cockatrice/issues/380) and
[#405](https://github.com/pizzaninja188/Cockatrice/issues/405) for related remaining acceptance,
dynamic-count and outcome-receipt work. Their live scope must be rechecked before implementation.

## Authored program and compiled representation

Use one `CardProgram` shape for spell bodies, modes, activated and triggered ability bodies, and
branch bodies. Each instruction has a stable ID, one finite kind, and optional named output bindings.
An operation's existing effect parameters remain typed. `Perform`, `If` and the existing paid/optional
branch-selection semantics are structural program nodes, rather than special cases in effect lists.

`EffectOp` denotes the operation portion of today's `SpellEffectKind`, not a second parallel enum
containing copies of every existing operation. During migration, lower old effect-list syntax into
the same compiled program. Retire old aliases and composition variants with their migrated families.

The following is the broader target type boundary, including future shapes. Use the active
authoring reference for accepted Rust/RON rather than copying this sketch:

```rust
// card-model: deserialized syntax; output types are inferred from operation signatures.
struct CardProgram { body: AuthoredBlock }
struct AuthoredBlock { steps: Vec<Instruction>, yields: Vec<NamedResultRef> }
struct Instruction { id: InstructionId, bind: Vec<ResultId>, kind: InstructionKind }
enum InstructionKind {
    Perform(EffectOp),
    If { condition: GameCondition, yes: AuthoredBlock, no: AuthoredBlock },
    Choose { choice: ChoiceId, branches: Vec<AuthoredBranch>, otherwise: AuthoredBlock },
}
// card-model: immutable, validated representation, supplied through CardRegistry.
struct CompiledProgram { identity: ProgramIdentity, blocks: Vec<CompiledBlock> }
struct CompiledBlock { steps: Vec<CompiledInstruction>, yields: Vec<TypedSlotRef> }
struct CompiledInstruction { inputs: InputBindings, outputs: OutputSignature, kind: CompiledKind }
// core: private execution state; program identity resolves against the immutable registry.
struct ResolutionFrame {
    program: ProgramIdentity, stack_context: ResolvingContext,
    cursors: Vec<BlockCursor>, slots: Vec<ResultSlot>, in_flight: Option<OperationState>,
}
enum ResultSlot { Unwritten, Ready(TypedValue) }
```

`Choose` retains the existing typed chooser, payment, optionality, branch IDs and decision semantics
from `ResolutionBranchDef`; the abbreviated skeleton does not replace them with an arbitrary menu.
An operation signature owns named output fields and types; `bind` assigns local names to those fields,
not author-selected type annotations. Typed values include generation-bound selections, action/card
cohorts partitioned by affected player, produced-object cohorts with provenance, captured card counts,
and choice/payment outcomes. These remain distinct variants rather than interchangeable object lists.
Compiled input/slot references preserve those types and target-role mappings without runtime string
lookup. `Unwritten` is an execution state and cannot be consumed as an empty semantic result.

Representative consumers ground the vocabulary: Brainstorm/Faithless Looting need a program spanning
draw and hand actions; Gorging Vulture/Gerrard's Verdict distinguish action receipts and filtered card
counts; Myr Battlesphere/Eerie Ultimatum use resolution-time selections with later actions;
Divert Disaster binds the fixed-cost counter-payment result and gates its Lander on `Paid`; these
are API examples, not new admissions.

The card-model crate owns compilation and validation. It produces immutable program/block IDs,
instruction addresses, typed result-slot indexes, target-role mappings and required context.
The registry supplies these programs to core without introducing a production dependency on the
embedded-card crate. Runtime frames reference a program identity and fingerprint, never a closure
or serialized pointer. Replay continues to require the matching engine/card-data environment.

Author IDs are scoped to the owning face/ability/program. Result names are mechanical references;
display text and Oracle presentation mappings cannot define data flow. Stable instruction/target-role
IDs replace positional cross-references in migrated definitions; legacy `effect_indices` lower to
those addresses. A compiler walk computes target requirements even in nested blocks. Resolution-time
conditions do not remove target requirements at announcement or change all-targets-illegal handling.

### Typed bindings and evaluation time

- A binding has one producer and an operation-declared type. References must resolve to an earlier
  dominating producer. No forward references, mutable variables, global result dictionary, or
  cross-ability references are admitted.
- Blocks have lexical scope. Child blocks may read ancestor bindings; child bindings do not escape
  implicitly. A structural instruction may bind its selected block's explicit `yields` through a typed
  join. Every reachable arm, including decline/otherwise, must yield the same field types; cardinality
  is the union of the arms' possible cardinalities. No implicit absent/zero value is supplied. An arm
  without exports has no outer result, and direct cross-arm reads always reject. Existing flat branch
  output-to-`PreviousEffect` flows must lower through explicit joins or be rewritten into the relevant
  branch. Preserve Divert Disaster's semantic paid-result dependency through its typed slot; the
  `CounterUnlessPaid` receipt condition and unused `PreviousResultReceipt` wrapper are retired.
- A runtime-empty cohort is a valid value with count zero. It is distinct from an absent binding,
  declined optional action, failed payment, or an operation still waiting to commit. Those distinctions
  use typed status/receipts, not `None` interpreted differently by each consumer.
- Keep selected objects, committed movement/action receipts, produced objects and captured numbers
  distinct. A selection count is not proof that those objects were discarded, sacrificed or moved.
  Replacement-expanded token cohorts retain their provenance. Exact-one consumers reject many-object
  producers statically; runtime disappearance follows the operation's rules, not a loader error.
- Live selectors and amounts evaluate when their instruction begins, unless their type explicitly
  reads a captured receipt or event/cast-time fact. Never hoist live conditions to program start.
  Dynamic counts reuse `Amount`/count expressions across hand, mill and library operations instead
  of introducing another numeric field type for each operation.
- Object selections retain `(ObjectId, zone_change_generation)` and domain/zone constraints. Movement
  receipts distinguish input incarnation from actual output incarnation. Follow-ups never guess a
  new incarnation by physical card ID/name; last-known information is a separate explicit capability.
- Values and provenance stay server-only. Candidate publication is a separate authorized projection.
  Persist frame/results in deterministic state/checkpoint machinery; never serialize private slots
  into a client event or public debug/log field.

Each operation declares its input/result signature, legal contexts, target roles, and whether it may
suspend. Domain validators still check rules-specific constraints. Share traversal and data-flow
validation; do not replace meaningful per-operation restrictions with blanket acceptance.

## Execution frame and operation completion

One `ResolutionFrame` owns the resolving stack context, program identity, nested block/cursor stack,
result slots, and at most one active operation transaction. It preserves controller, source
incarnation, cast/trigger facts, legal target roles and stack-copy/deferred-exit state. Non-stack
choices such as legend selection retain their own owner; they do not fabricate stack frames.

Conceptual operation outcomes are `Complete(typed_outputs)`, `AwaitChoice(operation_state, offer)`,
`AwaitRulesWork(operation_state, child_work)` and `GameEnded`. These are private engine terms.
Complete means the operation committed all work it owns. Awaiting a draw replacement, entry decision,
discard replacement or child cast is not instruction completion.

1. Resolve the current immutable instruction address. Evaluate its live inputs and capture the
   operation's required snapshot once. Enter the chosen conditional block once; never reevaluate
   its condition merely because a child instruction suspended.
2. Run or resume the operation through the existing audited rules transaction. The operation owns
   proposal, replacement ordering, committed events and its typed partial state.
3. While it waits, retain the same instruction address and unpublished result slots. Parent
   sequencing cannot advance, repeat committed substeps or consume an incomplete receipt.
4. On completion, publish outputs to slots exactly once. Drain required immediate observer work at
   its actual event boundary, preserving a typed parent frame. Collect ordinary triggers without
   granting priority or running ordinary SBAs in the middle of resolution. Domain-specific immediate
   work and state-trigger observations retain their required boundaries.
5. Advance the owning cursor exactly once. Exiting a child block resumes its parent cursor. Finish
   the stack object only after all nested instructions and required immediate work complete.

Observation is finer-grained than instruction completion: each committed individual draw/event
records its triggers at that event boundary; a simultaneous cohort is observed after the whole
semantic batch commits. Required immediate replacement/move consequences finish before dependent
work proceeds. Record state-triggered abilities when their condition becomes true, including during
a still-running/parked operation, with their existing suppression rules. Ordinary recorded triggers
wait for their prescribed stacking opportunity; neither priority nor ordinary SBAs occur mid-resolution.

Draws stay individual draw transactions with their replacement/failed-draw handling. Simultaneous
discard, sacrifice, damage, token creation and battlefield entry keep their existing or corrected
batch semantics. A batch snapshots/selects as required, proposes replacements, commits the semantic
cohort and publishes receipts/triggers at the correct boundary. No generic `ForEach` node is admitted
to turn an inherently simultaneous operation into a loop.

| Existing transaction owner | Completion edge retained by the frame adapter |
|---|---|
| `PendingDrawTransaction` / `DrawCompletion` in [`draw.rs`](../tricerules/tricerules-core/src/engine/draw.rs) | Individual draws can already be committed while the operation waits; retain their progress, but publish the instruction output/advance its cursor only after all owned draw work completes |
| `PendingCounterPlacementBatch` / `CounterPlacementCompletion` in [`state.rs`](../tricerules/tricerules-core/src/state.rs) and [`replacement.rs`](../tricerules/tricerules-core/src/engine/replacement.rs) | Resolve proposed placement replacements and commit under the counter operation's batching contract before returning its placement receipts |
| `BattlefieldEntryEvent`, `EntryCounterReplacementResume::{Single,Batch}` and `BattlefieldEntryCompletion` in the same files | Entry plans and entry counters remain provisional; `validate_battlefield_entry_commit` prevents zone commit before planner completion; one completed adapter resumes the frame once |

Accepted earlier instructions are not undone because a later action is impossible. Perform as much
as the rules allow. Invalid player input must be rejected before mutating the pending frame, results,
events, RNG, command index or active offer. Reuse existing complete-command numeric checkpoints for
their governed errors. Internal compiler/runner invariant failures are not ordinary player illegality.
Generalize/reuse the existing complete-command checkpoint to restore all state, pending work, caches,
events and choice-ID allocators to the start of the failing command, then report an internal error
without accepting/logging that command. Keep earlier accepted commands intact; this is not rollback
of a whole multi-command resolution. No unlogged mutation may survive a failure. Do not automatically
retry an invariant error.

Copies get independent frames/results with the copy's captured choices and controller. Do not share
mutable slots between original and copy. Source departure does not end an independent ability; exact
incarnation checks and explicit LKI govern later source uses. Concession/game termination follows the
existing player-removal rules and disposes or resumes owned work once, including state-trigger guards,
temporary reveal permissions and deferred stack exit. No new two-player assumptions are permitted.

## Prompt identity and component contract

Mandatory logical `resolution_prompt_id` correlation connects `PendingResolution`,
`ResolutionChoiceRequired` and `SubmitResolutionChoice`. It distinguishes successive prompts even
when their candidates are identical. Candidate generation checks remain necessary and solve a
different identity problem.

- Allocate deterministic, nonzero IDs monotonically per deciding player within a game. The correlation
  key is `(game, deciding_player, resolution_prompt_id)`. A globally increasing visible number would leak other
  players' private prompt activity through gaps, even if the field were per-player. Store per-player
  allocator state in engine state/checkpoints. Never wrap/reuse an actor's ID. Reconnect and identical
  active-offer republication retain the logical ID.
- Every replacement/new prompt receives a fresh ID, even when actor, kind and candidates are equal.
  Any answer-semantic change (candidate identity/generation, bounds, allowed decisions, cost, source,
  chooser or ordering) replaces the offer. For `ManaPayment`, `payment_currently_legal` is readiness
  derived from the live mana pool, not answer identity: when only readiness changes for the same
  pending payment, republish under the same ID and refresh its enabled state. A changed cost, source,
  actor or logical payment occurrence receives a fresh ID. If another logical prompt supersedes a
  parent offer, retire the parent's old ID. Saved parent operations retain their state; restoring a
  parent offer revalidates it and allocates a fresh ID. Repainting/reconnecting to an unchanged active
  offer is not replacement.
- Check actor, active ID, payload shape, candidates/generations, bounds and decision legality before
  taking pending state or committing any mutation. A duplicate or late answer is an unchanged-state
  rejection, including unchanged choice-ID allocators. Rejected commands do not enter accepted replay
  logs or advance deterministic state.
- Mark the request ID per-player and restore it only for the deciding recipient. Internal counters,
  frame IDs and result slots are server-only. Commands echo the engine-issued ID, not a client counter.
- Update every Rust/C++ constructor, pending/reconnect snapshot, choice UI submission and replay path
  that carries this contract. Updated clients and server must deploy together; missing/zero IDs reject.
  Existing captures require their original compatible binaries/data; no speculative legacy adapter.

Every accepted gameplay batch, including initial state, reconnect and terminal state, carries an
authoritative `resolution_prompt_state` after automatic work settles. An inactive state retires any
staged resolution answer. An active state identifies the deciding player publicly and carries a
nonzero optional ID only for that player. Other recipients receive no ID field. A pure standalone
payment-preview response preserves the current state; a response with gameplay events or legal
actions is not exempt from the snapshot contract. Server-synthesized, non-authoritative combat
previews also preserve the current state when they omit a snapshot and contain only preview,
log or identity-map events with no legal-action map. They never replace an engine gameplay batch.

Only a `ResolutionChoiceRequired` matching the final active player and ID can install an answerable
offer. Informational or superseded choice events may still publish their authorized reveal effects,
but cannot stage answers. The same active ID without another matching choice event retains an existing
exact offer, including its pending submission callback. A new ID without its offer, a missing gameplay
snapshot, or a changed answer contract under the same ID fails closed. Relay reconnect caches follow
the same final-state rule and preserve only the
exact active offer. Generic per-player redaction must restore the ID only for its deciding recipient.

All resolution answer paths capture the offered ID before a menu, dialog or cast draft and revalidate
it before sending. Resolution-time land play also echoes that ID in `PlayLand`; ordinary land play
uses zero. A stale resolution answer cannot become an ordinary cast or land play. Child work retires
the parent offer; returning to the parent revalidates its candidates and issues a fresh ID without
repeating completed instructions or reevaluating an already-entered conditional block.
Retiring an offer expires its pending callbacks even when submitting the answer already emptied the
local choice holder. Leaving a game clears its prompt state; numeric IDs are scoped to that game.

Use an engine-owned `ResolutionPromptId(u64)` newtype. It is distinct from the existing model's authored
`ChoiceId`, which identifies a branch/choice definition and remains stable across executions. Neither
an authored choice ID nor an instruction ID is a valid prompt correlation token.

| Surface | Required change / boundary |
|---|---|
| card-model / cards | Program schema/compiler, operation signatures, generator emission and reviewed definitions |
| core | Runner, operation completion adapters, typed slots, active choice ID, replay/checkpoint and concession integration |
| protobuf / generated Rust and C++ | Request/answer ID fields with correct visibility; preserve reserved tags |
| Servatrice / network library | Relay opaque IDs, authorized redaction/republication, exact physical bindings; no rules evaluation |
| Cockatrice | Store the active offered ID and echo it from every relevant picker/button/payment/cast submission; reconnect replaces stale local selection |
| Freeform | N/A: all new behavior stays behind ruled gating |

The existing choice kinds, candidate presentation and physical-card projections remain the baseline.
New program/result types do not cross the wire. Any later operation needing a new UI interaction must
trace that separate contract through producers/consumers and obtain its own acceptance evidence.

## Authoring and generator contract

Handwritten RON and generation emit the same program. Templates bind typed amounts, filters,
recipients, durations and keywords, and compose reviewed program fragments with hygienic local IDs.
The generator does not interpret arbitrary Oracle prose or evaluate game state. It matches a reviewed
grammar, consumes the complete relevant clauses/faces, and rejects ambiguity, unsupported combinations,
unconsumed text, unavailable contexts or invalid output through the shared compiler.

Use shared templates for proven families (for example numeric pump plus keyword combinations), not
one Rust matcher per literal parameter combination. Preserve negative/exclusion cases and complete-card
admission. Keep source/rulings provenance, Oracle-line presentation and independent semantic assertions
attached to the same evidence record; structural compilation is not semantic approval. Existing local
datasets are sufficient for implementation preparation; external dataset refresh needs its own authority.
Use the existing review-map/fixture workflow for those links; this design adds no evidence tracker.

Custom Rust effects receive narrow operation/choice capabilities and declare the same typed inputs and
outputs. They may own a specialized algorithm state, but cannot bypass replacements, expose GameState,
construct client-legality rules or choose their own stack cleanup. Shared algorithms migrate into an
ordinary operation/program. Do not force genuinely distinct mechanics into loosely typed parameters.

## Migration sequence and acceptance

These are dependencies for the broader migration; the bounded pilot does not complete this list:

1. The model/compiler and temporary source-only lowering established explicit target roles and
   result provenance. For the retired DrawDiscard family, that lowering and provenance are now
   removed after owner and behavior assertions passed.
2. Introduce the frame runner and completion adapters under the current operations. Migrate a bounded
   draw/hand/library family through real cards; prove nested resume, exactly-once completion and unchanged
   simultaneous/replacement semantics. Keep one owner per resolving instruction during transition.
3. Implement prompt-ID propagation through all producers/consumers before promising stale-answer
   rejection. Run both Rust and C++ gates and visible interaction acceptance for the changed path.
4. Complete for the DrawDiscard family: its 45 owners use explicit programs, and its old sequencing
   variant, draw-after continuations, and source adjacency lowering are removed.
5. For this family, existing generator templates already emit authored programs. Source datasets
   were not refreshed as part of the retirement. Broader generator and corpus migrations remain
   separate work.

Deletion is mandatory when no admitted definition, generated template or runtime caller uses a
superseded path and its independent assertions pass through the shared implementation. Legacy
`custom_effect` data fails registry admission; temporary lowering has a family-specific retirement
condition, and there is no permanent second execution architecture.

Pilot retirement inventory: `custom::Brainstorm`, `ResolutionStep::Draw`, `PostDrawPhase`,
`DrawCompletion::BrainstormPutBack`, the custom draw-completion bridge and its library-placement
helper are removed. Brainstorm uses shared draw, private ordered hand selection and bound top
placement through the program runner. The `DrawDiscard` variant, source lowerer and `BeginDiscard`
continuation are now removed; all 45 owners from the original census use authored programs. The
`custom::GiftsUngiven` implementation and generated custom-effect registry are removed. Gifts now
uses a typed retained search, explicit reveal, target-bound partition, movement and shuffle; scenarios cover the
0–4 cohort matrix, target delegation, source/controller departure, commander replacement, and
precommit graveyard order. This composable execution migration does not change Gifts' whole-card
admission status; whole-card review remains deferred. Keep `custom_resolution/*` where it implements
shared mechanic transactions, copy legality or entry/APNAP work; its directory name does not make every
contained mechanism card-specific.
Wash Out uses the shared owner-hand return continuation for matching mass returns and individual
returns. Library-return cohort consolidation remains separate and deferred for a later migration.

Pilot selection and implementation remain separate. Freeze representative existing and later-consumer
examples before coding. Brainstorm and Faithless Looting exercise sequential draw/hand actions; Gifts
Ungiven exercises interdependent private/public choices; Eerie Ultimatum exercises selection followed by
simultaneous entry. They are semantic references, not a claim that one new operation admits all four.
Flashback, search permissions, uniqueness constraints, and entry/replacement behavior remain separate
complete-card prerequisites.

Required evidence includes:

- Compiler rejection of forward/out-of-scope/wrong-type references, invalid cardinality/provenance,
  unsupported contexts, ambiguous targets, malformed nested trees and unsupported templates.
- Nested branch choice/replacement suspension resumes the exact instruction once; skipped branches
  neither prompt nor leak results; impossible later actions preserve earlier legal actions.
- Replacement-expanded and simultaneous operations preserve event count, ordering and committed
  receipts; selected versus actually moved/discarded/sacrificed cohorts remain distinguishable.
- Multi-player simultaneous actions collect their required choices in APNAP order before the cohort
  commits. Target legality is separate from resolution-time choice: an illegal Gifts Ungiven target
  produces no resolution prompt; searched candidates stay private until the explicit revealed cohort
  becomes public. Draw replacements finish per draw; Eerie's entering Auras cannot attach to another
  simultaneous entrant.
- Original/copy independence, source leave-and-return, disappearing candidates, player departure,
  terminal resolution, and deferred spell exit/state-trigger-guard cleanup.
- Duplicate/late/wrong-player choice rejection, reconnect with the same active ID, changed-offer
  invalidation, private candidate/ID redaction, physical identity/order and deterministic replay.
- Inject failures after a transaction commit and after slot/offer allocation: restore the entire
  failing command, including RNG, caches, pending frames, IDs and emitted events, with no replay-log
  advancement. Previously accepted resolution commands remain intact.
- Existing independent card scenarios plus one preselected later consumer that needs no engine edits
  within the newly supported family. Count full-card readiness separately from primitive coverage.

Use focused intended red/green tests, independent design/frozen-code review and the affected-side gates
in [AGENT-VERIFICATION.md](AGENT-VERIFICATION.md). The prompt-ID change requires Rust and C++, relay E2E,
and real two-client GUI checks; protocol E2E does not establish visible acceptance. Record any manual
deferral explicitly. Measure subsequent-card engine edits and phase-separated authoring effort; do not
set success by gross line deletion or extrapolate a pilot into a full-format schedule.

## MTG applicability and source boundary

This design governs resolution ordering, choices, targets, replacements, simultaneous actions, object
identity and private information. Oracle and current official Comprehensive Rules govern every migrated
operation/card. No rules simplifications or new card admissions are authorized by this design.

Sources checked on 2026-10-12: the [official rules page](https://magic.wizards.com/en/rules) links the
[Comprehensive Rules effective 2026-09-25](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf).
The independent Luna MAX rules reviewer fetched that PDF and the following Wizards card-text/ruling
references. They establish architectural constraints; they do not replace exact-identity preflight
and card-specific evidence when the pilot is selected.

| Source | Constraint on the contract |
|---|---|
| [Brainstorm: Final Fantasy release notes](https://magic.wizards.com/en/news/feature/final-fantasy-release-notes) | Three individual draws and their replacement sub-actions finish before the ordered put-back; no priority window between instructions |
| [Faithless Looting: Strixhaven release notes](https://magic.wizards.com/en/news/feature/strixhaven-school-of-mages-and-commander-2021-edition-release-notes-2021-04-16) | Complete the draw events, then the discard instruction, within one resolution; the flashback cast method remains a separate prerequisite |
| [Gifts Ungiven: Double Masters 2022 release notes](https://magic.wizards.com/en/news/feature/double-masters-2022-release-notes-2022-06-24) | Targeted opponent, up-to-four distinct names, public found cohort and opponent's subsequent two-card choice are distinct facts; finding one or two forces those cards into the chosen cohort |
| [Eerie Ultimatum: Tarkir Dragonstorm release notes](https://magic.wizards.com/en/news/feature/tarkir-dragonstorm-release-notes) | Choose the different-name cohort at resolution and return it simultaneously; an entering Aura cannot enchant another member of that same entering cohort |

Relevant CR constraints from that version:

- **608.2a-b:** intervening-if and target-legality checks keep their distinct timing. A conditional
  instruction does not postpone or replace the initial all-targets-illegal check.
- **608.2c-f/h-i/p:** follow printed instruction order, make required resolution choices, evaluate
  information at the required time/LKI boundary, and finish resolution before ordinary priority.
  Partial impossibility is not rollback of earlier legal instructions.
- **121.2/121.2a, 614.11a, 616.1:** multiple draws happen individually; a draw replacement's required
  actions finish before the next draw, with applicable replacement choices/order preserved.
- **400.7:** a zone change normally creates a new object. A retained selection is not permission
  to act on a later incarnation; exceptions/actual movement receipts are explicit.
- **303.4f, 614.13a:** Aura attachment and choices within entry-modifying effects have specific
  entering-object restrictions; 614.13a is not a general model for every entry replacement condition.
  Use the Eerie ruling and 608.2f for its simultaneous cohort, rather than independent ordinary moves.
- **603.3, 101.4:** ordinary triggered abilities wait for the prescribed placement opportunity;
  APNAP choice ordering remains owned by the applicable operation/rules boundary.
- **603.8, 117.3b, 704.3:** observe state triggers during resolution when applicable; ordinary
  priority/SBA processing waits for the prescribed priority boundary after resolution.
- **104.3a, 800.4g, 514.1-3:** concession/player departure and cleanup are independent rules surfaces
  that must survive parked resolution; cleanup/ordinary SBAs are not inserted between instructions.

The interaction checklist is covered by the ownership/identity, completion/timing, rejection/player,
privacy, component and verification sections above. Specific pilot Oracle identities, every applicable
card ruling, exact CR application to that patch, executable evidence and visible acceptance remain
required in the implementation step. No runtime MTG-compliance claim is made for unimplemented types.
