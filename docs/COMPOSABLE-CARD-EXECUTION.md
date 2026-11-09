# Composable card execution contract

This document owns the architectural constraints for composing card operations, suspending
resolution, publishing typed results, and answering engine prompts. It is not a change log,
card-support inventory, authoring syntax reference, or manual-test backlog.

| Information | Maintained source |
|---|---|
| Accepted RON, finite profiles and their restrictions | [Card authoring reference](../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md#finite-authored-programs) |
| System ownership, identity and privacy | [Architecture](ARCHITECTURE.md) |
| Implementation workflow and evidence | [Authoring workflow](AUTHORING-WORKFLOW.md), [verification guide](AGENT-VERIFICATION.md) |
| Client payment and choice behavior | [Ruled client reference](../cockatrice/src/game/ruled/README.md) |
| Card-specific sources and semantic assertions | [Review maps](../tricerules/tricerules-cards/authoring/review-maps/) and the referenced tests |
| Outstanding work and delivery history | GitHub Issues and Git history |

## Outcome and boundaries

A new combination of supported operations should be expressible as validated typed card data,
including choices that suspend resolution. It should not require another card-named continuation
or an implicit dependency on the immediately preceding effect. New mechanics still require
engine support and their own semantic evidence; this contract does not widen compiler admission.

Tricerules remains the single state writer. Keep the immutable registry, explicit replacement
transactions, generation-bound identity, deterministic logged decisions and recipient-filtered
presentation. Combat, turn structure, continuous effects, costs, triggers, state-based actions
and replacement ordering retain their own semantic owners.

Card programs are finite instruction trees. They do not provide arbitrary scripts, functions,
jumps, author-directed loops or a general-purpose virtual machine. The retired custom-effect
escape hatch is not an authoring option. Specialized algorithms belong in typed engine operations
with concrete inputs, outputs and completion contracts.

Programs and supported effect lists share operation evaluation and observation boundaries.
Their drivers retain their own cursors, suspension, retry and publication responsibilities.
A sequence of operations cannot replace an inherently simultaneous rules transaction.

## Authored programs and compilation

One exact spell face, modal mode, activated ability or triggered ability owns each program.
Its stable authored IDs, compiled identity and target bindings remain distinct from runtime
objects and prompt IDs. Modal sibling admission and supported owner contexts are defined by the
authoring reference, not inferred from a nearby successful card.

The [card-model program compiler](../tricerules/tricerules-card-model/src/primitives/program.rs)
validates the finite tree, operation signatures, result references, cardinality, target roles and
required context. The immutable registry supplies compiled programs to core; it does not make
core depend on the embedded-card crate. Runtime frames resolve the exact compiled owner and
fingerprint, never a serialized pointer or mutable closure. Replay requires compatible binaries
and card data.

Display text, Oracle line mappings and card names do not define data flow or legality. Target
requirements are compiled through nested blocks; a resolution-time condition cannot postpone
announcement targeting or bypass the initial target-legality check.

### Typed bindings and evaluation time

- Each binding has one producer and an operation-declared type. Reject forward, out-of-scope,
  cross-ability, wrong-type and unsupported references before execution.
- Blocks have lexical scope. Child results escape only through explicitly supported typed yields.
  Every reachable arm must supply compatible outputs; decline and Otherwise do not silently
  fabricate a zero or absent value. Unsupported joins remain rejected.
- An unwritten result differs from a completed empty cohort, a declined choice, failed payment
  and an operation that is still awaiting work.
- Selections, committed action receipts, produced objects and captured counts remain distinct.
  Selecting a card does not prove it was discarded, sacrificed or moved. Receipts preserve their
  action, affected player, cardinality and provenance, including replacement-expanded cohorts.
- Live conditions and amounts evaluate when their instruction begins. Event-time, cast-time,
  payment-time and last-known facts require explicit captured inputs. Do not hoist live reads
  to the start of the program or reevaluate an entered branch after suspension.
- Object references retain ObjectId, zone-change generation and applicable domain/zone constraints.
  Movement receipts distinguish input and output incarnations. Never find a later incarnation by
  physical card ID or name. Player references remain a separate typed domain.
- Results and provenance are private engine state. Candidate presentation is a separate authorized
  projection; private slots do not belong in public events or logs.

[OperationInput and OperationResults](../tricerules/tricerules-core/src/state/operation_results.rs)
carry explicit typed inputs and tagged operation facts. Multiple outputs and completed empty
results are valid. Partial facts survive suspension separately from the parent's published slots.

Brainstorm and Faithless Looting ground sequential draw/hand operations; Gorging Vulture and
Gerrard's Verdict distinguish committed receipts from counts; Divert Disaster consumes an explicit
paid outcome. These examples explain the abstractions, not additional authoring admission.

## Execution and completion

The [program runner](../tricerules/tricerules-core/src/engine/resolution/program.rs) retains the
resolving context, exact program owner, block/cursor stack, typed slots and in-flight operation.
Controller, source incarnation, targets, cast/trigger facts and deferred stack exit survive
suspension. Non-stack choices keep their own owner rather than fabricating a resolving stack item.

1. Resolve the immutable instruction and capture its required inputs at the proper evaluation
   boundary. Enter a selected conditional block once.
2. Run or resume the existing rules transaction. That operation owns replacement ordering,
   intermediate progress, committed events and its partial state.
3. While it awaits a choice or child rules work, keep the parent cursor and unpublished outputs.
   Do not repeat committed substeps or consume an incomplete receipt.
4. Publish completed outputs exactly once, after all work owned by the operation finishes.
   Preserve the immediate observer and replacement boundaries of that operation.
5. Advance the owning cursor once. Complete the resolving stack object only after its nested
   instructions and required immediate work finish.

Observation is finer-grained than instruction completion. Individual draws retain their separate
replacement and failed-draw handling. Simultaneous discard, sacrifice, damage, token creation and
entry preserve the whole semantic cohort and its ordering. Record applicable triggers at the
correct event boundary, including state-trigger observations; ordinary trigger placement, priority
and state-based actions retain their prescribed boundaries.

Draw, counter-placement and battlefield-entry transactions keep their domain-specific completion
adapters. A parked replacement or provisional entry is not a completed instruction. Shared owner-zone
movement likewise collects required destination and placement choices before committing movement;
private hand-selection transactions retain their separate selection and reveal boundaries.

Accepted earlier instructions are not undone because a later instruction cannot do everything it
requests. Invalid player input must leave the pending frame, results, events, RNG, command index
and active offer unchanged. Internal invariant failures are not ordinary player illegality:
the failing command's checkpoint must restore its complete state, caches, pending work, IDs and
events without accepting that command into replay. Earlier accepted commands remain intact;
do not automatically retry an invariant failure.

Copies have independent frames and results. Source departure does not by itself end an independent
ability; exact incarnation and explicit last-known information govern subsequent source uses.
Player departure and game termination must retire or resume owned work once, including reveal
permissions, child transactions, state-trigger guards and deferred exits. Keep player-set-generic
behavior and the operation's specific departure/delegation policy.

## Prompt identity and component contract

The runtime correlation key is (game, deciding player, resolution_prompt_id). It is independent
of authored choice/instruction IDs, transaction IDs and candidate object generations.

Allocate deterministic nonzero prompt IDs monotonically per deciding player. Store allocator state
in engine checkpoints and never wrap or reuse an actor's ID. A global visible counter would disclose
other players' private prompt activity. Reconnect and unchanged active-offer republication keep the ID.

Replace an offer when its answer semantics change: actor, kind, candidates/generations, bounds,
allowed decisions, cost, source or ordering. A mana pool readiness change alone does not replace an
otherwise identical payment offer. Child work that supersedes a parent retires the old parent ID;
restoring that parent revalidates it and allocates a fresh ID without repeating completed work.

Validate the actor, current ID, payload shape, exact candidates, bounds and decision legality before
taking pending state or mutating anything. Duplicate, late and wrong-player answers must reject
without advancing state, replay or ID allocators. Clients echo engine-issued IDs rather than creating
their own. Missing or zero resolution IDs reject.

Every authoritative gameplay batch, including initial, reconnect and terminal state, publishes the
final resolution_prompt_state after automatic work settles. An active snapshot identifies the
deciding player publicly but carries its nonzero optional ID only to that player. An inactive snapshot
retires staged answers. Pure standalone payment previews preserve existing prompt state.
Non-authoritative combat previews may omit the snapshot only under their existing restricted
preview/log/identity-map contract, without a legal-action map.

Only a choice event matching the final active player and prompt ID can install an answerable offer.
Informational or superseded events may publish authorized reveal effects but cannot stage answers.
An unchanged active ID without another choice event retains the exact existing offer and callback.
A new ID without its offer, a missing authoritative snapshot, or changed answer semantics under the
same ID fails closed. Relay reconnect caches follow this same final-state rule.

Menus, dialogs, payment drafts and resolution-time casts capture the offer identity and revalidate
it before submission. Resolution-time land play also echoes the prompt ID; ordinary land play uses
zero. Stale resolution work cannot become an ordinary cast or land play. Retiring an offer expires
callbacks even if submission already emptied the local choice holder. Leaving a game clears its
prompt state.

| Component | Responsibility |
|---|---|
| Card model / registry | Compile immutable owners, signatures, typed bindings and required context |
| Core | Execute operations, retain continuations/results, validate offers, checkpoint and replay |
| Protobuf | Carry commands and recipient-authorized projections; preserve tags and visibility |
| Servatrice | Authenticate, bind physical cards, relay opaque identities and filter private data |
| Cockatrice | Present engine offers, retain exact local staging and discard stale answers |

Program frames and result slots do not cross the wire. A new visible interaction needs its own
producer/consumer trace and acceptance evidence. Keep ruled product entry points and guarded shared
presentation boundaries as described in the architecture guide.

## Authoring and verification

Handwritten RON and generated definitions use the same compiler and admitted operation profiles.
Generation matches reviewed grammar and consumes complete relevant clauses; it does not interpret
arbitrary prose or evaluate game state. Reject ambiguous, unsupported or partially consumed input.
Preserve stable owner IDs, hygienic local bindings, source/ruling provenance and semantic assertions.

Use the existing authoring reference and review maps rather than adding another capability list
here. Retire a legacy family only after its real callers move and its behavior is covered; preserve
shared transaction owners with remaining callers. Compiler acceptance alone is not complete-card
support or semantic approval.

For changed contracts, evidence must distinguish compiler rejection, correct operation results,
suspension/resume, simultaneous and replacement ordering, exact incarnation, independent copies,
departure, atomic failure, deterministic replay and recipient privacy. Use the affected-side gates
in the verification guide. Automated engine, model and network checks do not establish visible
two-client acceptance; report user acceptance or explicit deferral separately.

Oracle and the current official Comprehensive Rules govern every operation and card. This
architectural contract authorizes no rules simplification or card admission. Keep exact card-specific
rules analysis with the implementation's review evidence rather than a historical source table here.
