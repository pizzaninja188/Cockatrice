# Capability pattern index

This directory documents bounded, reusable rules patterns that card authors can look up while
preflighting a complete card. An entry describes reviewed evidence and its limits. It is not a
rules source, generator input, registry, or substitute for reviewing the full card.

Phase 2 seeds ten entries from shipped card definitions, review maps, engine paths, and semantic
scenario assertions. The planned three-batch pilot remains pending; these entries are not pilot
measurements. The [pilot status and measurement protocol](PILOT.md) records that status and defines
how to measure future use. The template below is not evidence and must not be counted as catalog
content.

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
Keep paths and symbols exact so a later lightweight checker can verify references and locate
symbols. Link to card definitions and review maps with ordinary Markdown links relative to the entry
file.
List every implementation, card, review-map, presentation, and test path that supports the entry's
claims under **Reviewed paths**. A future checker may warn when those paths changed after the entry's
explicit reviewed revision; a warning requests review and does not decide semantic support.

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
| [Lotus Petal chosen-color mana](entries/lotus-petal-chosen-color-mana.md) | Mana ability | Taps and sacrifices itself for one of five colors; does not cover unrestricted mana choices beyond those options. |
| [Artifact sacrifice draw](entries/artifact-sacrifice-draw.md) | Activated ability | Mind Stone pays {1}, taps and sacrifices itself before one draw resolves. |
| [Ichor Wellspring event draw](entries/ichor-wellspring-multizone-draw.md) | Triggered ability | One ability triggers on entry and battlefield-to-graveyard; the fixture checks the second event while the entry trigger waits. |
| [Myr Retriever death recovery](entries/myr-retriever-dies-recovery.md) | Triggered ability | Selects another artifact card from the trigger controller's graveyard after simultaneous deaths; target departure before resolution is untested. |
| [Voltaic Key artifact untap](entries/voltaic-key-artifact-untap.md) | Activated ability | Targets an artifact under any player's control, including itself or one already untapped; resolution-time target departure is untested. |
| [Prized Statue event Treasure](entries/prized-statue-event-treasure.md) | Token creation | One Treasure follows entry and one follows its battlefield-to-graveyard move; its token mana ability is outside this claim. |
| [Quicksmith Genius optional discard then draw](entries/quicksmith-genius-optional-discard-draw.md) | Triggered ability | One controlled artifact entry presents an optional discard-then-draw; repeated events are outside the cited fixture. |
| [Prosperity X for each player](entries/prosperity-x-each-player-draw.md) | Group draw | Three-player X=2 and two-player X=0 cases are covered; the index does not make a whole-card readiness claim. |
| [Scavenger Grounds exile all graveyards](entries/scavenger-grounds-all-graveyards-exile.md) | Graveyard effect | Sacrifices a controlled Desert (including itself) as a cost, then exiles cards in all players' graveyards at resolution. |

The entries inspect the cited RON, review-map, test, and engine source at the recorded revision.
Tests were inspected, not run in this documentation phase. No manual client acceptance is claimed;
whole-card readiness remains unassessed in each entry. The three-batch pilot is still pending.

## Catalogue boundary

Only Markdown entries in `entries/` are catalog content. The exact file `entries/ENTRY-TEMPLATE.md`
is excluded. The template must be copied to a file named for its ID and completed before it is
treated as an entry. Do not put generated reports or pilot measurements here.

An entry is lookup evidence for authoring and review. It never changes card admission, generation,
engine behavior, or verification gates.
