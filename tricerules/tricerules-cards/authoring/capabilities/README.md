# Capability pattern index

This directory documents bounded, reusable rules patterns that card authors can look up while
preflighting a complete card. An entry describes reviewed evidence and its limits. It is not a
rules source, generator input, registry, or substitute for reviewing the full card.

Phase 1 establishes the entry contract only. There are no capability entries yet. The planned
three-batch pilot remains pending; the template below is not evidence and must not be counted as
catalog content.

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

## Catalogue boundary

Only Markdown entries in `entries/` are catalog content. The exact file `entries/ENTRY-TEMPLATE.md`
is excluded. The template must be copied to a file named for its ID and completed before it is
treated as an entry. Do not put generated reports or pilot measurements here.

An entry is lookup evidence for authoring and review. It never changes card admission, generation,
engine behavior, or verification gates.
