# Selected implementation examples

**Optional navigation aid, not a capability inventory.** The capability index is a narrow collection
of reviewed examples. Start ordinary authoring with the [implemented-analogue workflow](../CARD-AUTHORING.md#find-implemented-analogues).
Consult this collection when useful; neither lookup nor hit/miss reporting is mandatory. An absent
entry says nothing about whether the engine can support a card. Do not expand this collection as
a separate coverage project.

This directory documents bounded, reusable rules patterns that card authors can look up while
preflighting a complete card. An entry describes reviewed evidence and its limits. It is not a
rules source, generator input, registry, or substitute for reviewing the full card.

Phase 2 seeded ten entries from shipped card definitions, review maps, engine paths, and semantic
scenario assertions. The pilot added source-counter-scaled mana after Astral Cornucopia and life-paid
modal land entry after two MH3 cards. All three pilot batches completed on 2026-09-27; the
[pilot protocol](PILOT.md) records the baseline and comparison, including its limits. Entries remain
lookup aids, not pilot measurements or proof of support for a new card. Measurement records stay
outside the catalogue. The template below is not evidence and must not be counted as catalog content.

## Keep three questions separate

- **Runtime support:** Can the engine express and resolve this exact behavior, including its
  relevant validation and interactions? A typed variant, enum, or Rust symbol alone does not
  prove this. A supported claim needs actual semantic tests with independent expected results.
- **Generator recognition:** Can the current generator recognize and emit the stated input shape?
  Name the exact recipe and its limits. Recognition does not establish runtime support or prove
  that composed output works.
- **Whole-card readiness:** Is a named card complete across all faces, clauses, costs, targets,
  choices, cross-face relationships, and presentation prerequisites? Judge this per card. A
  supported pattern or a present registry entry does not make a whole card ready.

Record each status independently. Use `Unassessed` when the evidence has not been reviewed. Explain
partial or unsupported cases precisely; absence of an index entry also means unassessed, not
unsupported. Do not promote a claim because an enum, recipe, card definition, review map, or test
name exists by itself.

## Entry contract

Copy [`entries/ENTRY-TEMPLATE.md`](entries/ENTRY-TEMPLATE.md) to a new file beside it named for its
stable pattern ID, such as `entries/draw-cards.md`. Keep one entry focused on one precisely described
behavior or composition. Use lowercase kebab-case IDs; once published, an ID keeps the same meaning
and is never reused. Change the title or search terms when wording evolves.

Every entry keeps the template's headings and fields. In particular, it records:

- a stable ID, category, and useful search terms;
- exact behavior and composition, prerequisites, boundaries, and near misses;
- separate runtime-support and generator-recognition statuses, with typed symbols and role-labeled
  implementation references;
- shipped card IDs, definition links, and review-map links that demonstrate the claim;
- exact semantic test identities, file/function anchors, and the behavior each test actually
  covers, using the checklist's `Exercised`, `N/A`, or `Fixture-blocked` classifications;
- presentation prerequisites, or a reasoned statement that none apply; and
- the reviewed Git revision and the repository paths on which the review depends.

Write implementation and test references as repository-relative paths with forward slashes. For
their anchors, use `path#ExactSymbol`. For each semantic test, include both the exact Cargo test
identity (the target/module/test form used by the test runner or review map) and its file/function
anchor. For example, Divination is recorded as
`scenario spell_effects::cast_divination_draws_two_cards` plus
`tricerules/tricerules-core/tests/scenario/spell_effects.rs#cast_divination_draws_two_cards`.
Keep paths and symbols exact so the optional lightweight checker can verify references and locate
symbols. Link to card definitions and review maps with ordinary Markdown links relative to the entry
file.
List every implementation, card, review-map, presentation, and test path that supports the entry's
claims under **Reviewed paths**. The optional checker can warn when those paths changed after the
entry's explicit reviewed revision; a warning requests review and does not decide semantic support.

Use repository links to the existing [card authoring guide](../CARD-AUTHORING.md),
[dependency report contract](../DEPENDENCY-REPORT.md),
[card identity catalog](../../../CARDS.md), and existing
[review maps](../review-maps/). For example, the Divination review map links its typed path and
semantic fixture to the [Divination definition](../../data/divination.ron) and the
[scenario test](../../../tricerules-core/tests/scenario/spell_effects.rs). These sources remain
authoritative for their respective content; the index points to them instead of copying them.

## Seeded patterns

| Entry | Category | Evidence and boundary |
|---|---|---|
| [Permanent taps for {C}](entries/permanent-taps-for-colorless.md) | Mana ability | Mind Stone and Scavenger Grounds each tap for one colorless mana without the stack; covers these two shipped definitions. |
| [Mana scaled by a source counter](entries/source-counter-scaled-mana.md) | Mana ability | Astral Cornucopia multiplies one chosen-mana option by its current Charge-counter count; zero-counter output remains empty while the mana-ability choice is retained. |
| [Lotus Petal chosen-color mana](entries/lotus-petal-chosen-color-mana.md) | Mana ability | Taps and sacrifices itself for one of five colors; does not cover unrestricted mana choices beyond those options. |
| [Artifact sacrifice draw](entries/artifact-sacrifice-draw.md) | Activated ability | Mind Stone pays {1}, taps and sacrifices itself before one draw resolves. |
| [Ichor Wellspring event draw](entries/ichor-wellspring-multizone-draw.md) | Triggered ability | One ability triggers on entry and battlefield-to-graveyard; the fixture checks the second event while the entry trigger waits. |
| [Myr Retriever death recovery](entries/myr-retriever-dies-recovery.md) | Triggered ability | Selects another artifact card from the trigger controller's graveyard after simultaneous deaths; target departure before resolution is untested. |
| [Voltaic Key artifact untap](entries/voltaic-key-artifact-untap.md) | Activated ability | Targets an artifact under any player's control, including itself or one already untapped; resolution-time target departure is untested. |
| [Prized Statue event Treasure](entries/prized-statue-event-treasure.md) | Token creation | One Treasure follows entry and one follows its battlefield-to-graveyard move; its token mana ability is outside this claim. |
| [Quicksmith Genius optional discard then draw](entries/quicksmith-genius-optional-discard-draw.md) | Triggered ability | One controlled artifact entry presents an optional discard-then-draw; repeated events are outside the cited fixture. |
| [Prosperity X for each player](entries/prosperity-x-each-player-draw.md) | Group draw | Three-player X=2 and two-player X=0 cases are covered; the index does not make a whole-card readiness claim. |
| [Scavenger Grounds exile all graveyards](entries/scavenger-grounds-all-graveyards-exile.md) | Graveyard effect | Sacrifices a controlled Desert (including itself) as a cost, then exiles cards in all players' graveyards at resolution. |
| [Single library search result to battlefield](entries/single-library-search-to-battlefield.md) | Library search | Rampant Growth and Farseek put one filtered result onto the battlefield tapped and shuffle; multiple results or split destinations are outside the evidence. |
| [Modal land face with life-paid untapped entry](entries/modal-land-life-paid-entry.md) | Battlefield-entry replacement | Both named MH3 back faces may pay 3 life to avoid entering tapped; direct-RON definitions and exact life/color outcomes are tested. |

The ten Phase 2 entries inspect the cited RON, review-map, test, and engine source at their recorded
revisions; those tests were inspected, not run during that documentation work. The two entries
added during the pilot record separate card evidence and tests for Astral Cornucopia and the two
named MH3 cards. No manual client acceptance is claimed for any entry. Whole-card readiness remains
unassessed for the ten Phase 2 entries. The completed pilot did not produce comparable lookup-time
measurements; see [PILOT.md](PILOT.md) for the results and limits.

## Catalogue boundary

Only Markdown entries in `entries/` are catalog content. The exact file `entries/ENTRY-TEMPLATE.md`
is excluded. The template must be copied to a file named for its ID and completed before it is
treated as an entry. Do not put generated reports or pilot measurements here.

An entry is lookup evidence for authoring and review. It never changes card admission, generation,
engine behavior, or verification gates.

## Entry maintenance and scope

When relying on an entry, inspect material changes to its behavior boundary, implementation,
generator route, or supporting card, review-map, test, or presentation evidence. Correct or qualify
stale claims found during that work before relying on them. A freshness warning for an entry in use
is a prompt to inspect the reported path diffs: retain the claim only when the reviewed evidence still
supports it, and narrow the behavior or downgrade a status when it does not. Update **Reviewed
revision** only after actually reviewing the relevant paths and the claims that depend on them. A
new commit, a clean checker run, or an unrelated change is not a reason to blanket-bump revisions.
Neither a checker pass nor a revision update automatically promotes runtime support, generator
recognition, or whole-card readiness.

Routine authoring does not require running the checker or resolving unrelated catalogue warnings.
Do not turn a shared-source-file change into mandatory review of every entry. Keep known misleading
claims out of evidence used for admission; direct definitions and tests can be used without an entry.

Add an entry only when repeated real authoring work demonstrates that the reference would provide
a useful shortcut. A lookup miss or a newly supported mechanic alone does not require one. Do not
try to catalogue every mechanic or create an entry for each card. An outside-
deck card may help validate related semantics, but it does not expand the selected campaign or admit
that card. Keep pilot measurement records outside this catalogue as directed in [PILOT.md](PILOT.md).

## Optional source-only checker

From the repository root, run the checker when reviewing index edits:

```powershell
powershell -NoProfile -File scripts/check-capability-index.ps1
powershell -NoProfile -File scripts/check-capability-index.ps1 -CheckFreshness
```

It accepts `-RepositoryRoot` for an isolated checkout or fixture; otherwise it uses the checkout
containing the script. Exit code 0 means the catalogue structure and checked references passed;
exit code 1 reports structural or reference errors. Freshness warnings do not assert support or
readiness and do not fail an otherwise valid catalogue.

The default pass reads local Markdown, RON, JSON, and source text only. It checks required headings
and fields, IDs and statuses, local link targets, cited-path coverage by **Reviewed paths**, card IDs
against linked RON definitions, and exact semantic test IDs against linked review maps. It also
checks that referenced symbols and test functions appear in their cited source files. These are
small lexical checks, not general Markdown, RON, JSON-schema, or Rust parsers; they do not compile or
run tests, establish that Cargo exposes an exact test identity, prove test assertions, or determine
semantic support.

`-CheckFreshness` compares each entry's full **Reviewed revision** with only that entry's listed
**Reviewed paths**, including tracked working-tree changes and paths currently untracked. Unrelated
changes do not invalidate entries. An unresolved revision or unavailable Git status is reported as
unable to assess, never as fresh. The checker performs no build, network access, or file writes and
is not part of `verify.ps1` or card-data verification.
