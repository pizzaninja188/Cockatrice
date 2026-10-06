# Card authoring architecture improvements

## Authorized outcome

Reduce repeated engine and verification work while retaining complete-card admission gates.
This work adds capabilities and fixtures; it does not resume the paused four-deck campaign,
refresh external datasets, admit partially implemented cards, or claim deck completion.

## Implementation increments

1. Share strict conformance evaluations within one process. Use an explicitly bounded,
   deterministic scheduler for independent fixtures; preserve legacy seeds, command budgets,
   errors, integrity assertions and reviewed coverage baseline. Compare serial and parallel results.
2. Extract the exhaustive effect dispatcher. Conditional instructions reuse ordinary handlers
   with the same instruction identity and parked continuation. Add conditional discard and
   library selection, preserving validation boundaries. Shoreline Looter and Accumulate Wisdom
   are the concrete consumers. Conditions select once; accepted answers finish the selected
   operation and then the tail.
3. Extend the existing simultaneous discard transaction with zero-to-N selection bounds.
   Count actual discard receipts separately for each affected player without changing controller
   relationships. Flux, Steal the Show and Tersa Lightshatter demonstrate the behavior.
   Publish each accepted selection's count before the next player chooses, keeping identities
   private until commit. Restrict the scoped amount to prior-result resolution contexts.
4. Extend draw transactions to park while replacement actions make library choices.
   Preserve replacement application identity, per-draw sequencing and the optional parent
   continuation. Abundance and Tomorrow, Azami's Familiar demonstrate distinct consumers.
   Reconnect republishes the frozen action stage. Source departure does not cancel an applied
   action for a surviving drawer; drawer departure retires only that player's action. An optional
   decline is recorded on the current draw, including when another replacement modifies it.

Minds Aglow's variable mana payments and Zur's Weirding's persistent public hands require
separate work. These capabilities do not establish their complete-card readiness.

## Review and verification

Independent inspection-only design review precedes implementation; independent inspection of
the frozen patch follows it. Root owns all writes and executes serialized focused red/green
checks after behavior increments and the final Rust/CardData gate. Passing conformance
evidence must retain the exact reviewed baseline, seeds and fixture command budgets.
Measure the same complete conformance command before and after; report timings as observations.

Completed verification on October 5, 2026: CardData Check, the full Rust test suite,
all-target Clippy with warnings denied, all four package format checks and diff checking
passed. After the full suite passed, a Clippy-only correction replaced `% 3 == 0` with
`is_multiple_of(3)` in the scheduler test. Its three focused regressions and full Clippy
were rerun; the passing full-suite and CardData evidence was retained. Independent reviews
closed the implementation and corrected concession evidence. C++ gates are N/A because
the protocol, relay and client contracts and source are unchanged.

## Rules interaction checklist

- Authority, ordering and determinism: engine owns choices, mutations, replacement application,
  simultaneous discard commits and logged resumption. No Oracle interpretation at runtime.
- Identity: candidates retain zone-change generations; wrong-player, duplicate, stale and
  out-of-range answers fail without advancing the command log or continuation.
- Hidden information: private look/hand candidates stay with the deciding player; Abundance
  reveals publicly. Variable simultaneous choice counts and identities are reviewed separately.
- Targets and payments: existing target binding and payment semantics remain; new variable
  mana payment is outside scope. Conditional instructions retain their outer effect index.
- Multiplayer and lifecycle: recipients use APNAP order; concessions reconcile pending
  transactions; source changes cannot reapply a replacement already used on a draw.
  A draw instruction already resolving preserves its captured parent and surviving-player
  tail when its controller departs (608.2m); departed drawers' requests are retired.
- Zones and results: committed discard receipts include replacement destinations. Direct
  replacement puts into hand do not become draws, draw triggers, or failed draw losses.
- Protocol, relay, physical identity and client: existing choice/event vocabulary is reused.
  Reassess these surfaces if implementation needs new messages or visibility behavior.
- Freeform: N/A, engine-only ruled behavior. No client command paths change.
- Automated versus manual evidence: fixtures prove engine semantics; visible two-client
  acceptance remains a separate card-delivery requirement and is not claimed here.

## MTG applicability

Current official rules (September 25, 2026) govern APNAP choices (101.4), simultaneous choices
and instructions (608.2e), draws and replacement actions (121, 614, 616), and discards (701.9).
Oracle and rulings for the named consumers were checked during design. Full card admission
and mechanics listed as separate work remain deferred.

Sources: [official rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt),
[Flux](https://scryfall.com/card/wth/39/flux),
[Steal the Show](https://scryfall.com/card/sos/130/steal-the-show),
[Tersa Lightshatter](https://scryfall.com/card/tdm/127/tersa-lightshatter),
[Abundance](https://scryfall.com/card/cmm/884/abundance),
[Tomorrow](https://scryfall.com/card/bok/58/tomorrow-azamis-familiar).

## Bounded conformance workers

Ordinary test runs keep one internal coverage worker and share strict results within their
process. For an explicitly bounded parallel coverage run, set
`TRICERULES_CONFORMANCE_WORKERS=4` and use `--test-threads=1`. Coverage sweeps share a lock;
results are keyed and restored to deterministic order. Worker panics/errors remain failures.
No results are cached across processes or source changes.

```powershell
$env:TRICERULES_CONFORMANCE_WORKERS = '4'
$env:RUST_TEST_THREADS = '1'
$env:CARGO_BUILD_JOBS = '4'
./scripts/verify.ps1 -Side Rust -CardData
```

The unchanged serial conformance baseline at `0ca47b3f7` took 468.10 seconds on this host.
The updated suite with four internal workers took 119.53 and 137.31 seconds in two runs
(26 passed, one ignored),
including three new scheduler regressions, against the same reviewed coverage baseline.
That is about 3.4 to 3.9 times faster for this stage. These are host-specific observations excluding
compilation, not a prediction of total authoring throughput.

## Card admission acceptance

Before admitting cards using these primitives, run their full actual-card evidence and real
two-client flow. Recommended checks: threshold/one-versus-all library choices; zero/partial/full
discard with count-only announcements and private identities; Abundance's public revealed
prefix with private bottom ordering; Tomorrow's private selection; reconnect at each pending
stage; source/drawer/controller concession; exact physical card movement; effect tail once.
These visible checks have not been performed by the architecture fixtures.
