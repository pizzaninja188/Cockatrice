# <Behavior or composition title>

> Placeholder only. Replace every instruction and TODO before creating a catalog entry.

## Identity

- **Pattern ID:** `TODO-lowercase-kebab-id`
- **Category:** `TODO`
- **Search terms:** `TODO, TODO`

## Behavior and limits

- **Behavior:** TODO. State the precise rules behavior in plain language.
- **Composition and prerequisites:** TODO. Name the required order, choices, targets, costs, or
  other behavior that must compose with it; use `None` with a reason when appropriate.
- **Boundaries:** TODO. State what this entry covers and the important limits.
- **Near misses:** TODO. Name similar-looking behavior this evidence does not cover.

## Runtime support

- **Status:** `Unassessed` (`Supported`, `Partial`, `Unsupported`, or `Unassessed`).
- **Typed symbols:** TODO. Give the exact typed RON/Rust variant or field and its repository path.
- **Implementation references:** TODO. List role-labeled `path#ExactSymbol` references for the
  emitter/loader, validator, and engine consumer as applicable. For missing or unreviewed roles,
  say so explicitly.
- **Evidence and limits:** TODO. Explain why the cited path supports this exact behavior, including
  relevant interaction coverage and any unresolved limits. Symbols alone are not evidence.

## Generator recognition

- **Status:** `Unassessed` (`Recognized`, `Partial`, `Not recognized`, or `Unassessed`).
- **Recipe and input shapes:** TODO. Name the exact recipe, if any, and the shapes it recognizes.
- **References and limits:** TODO. Give `path#ExactSymbol` references and say what the route does
  not recognize. A missing route is not a runtime-support gap.

## Shipped card evidence

- TODO: list each exact card ID from its RON definition, with links to that definition and its
  review map. Example format: `card_id` — definition: [file](relative/path.ron) — review map:
  [file](relative/review-map.json).
- **Whole-card readiness:** `Unassessed`. Record a separate `Ready`, `Partial`, or `Unassessed`
  result for each named card, with its remaining requirements. Do not infer readiness from this
  pattern's runtime or generator status.

## Semantic test coverage

- TODO: use the exact Cargo test identity and the repository path/function anchor, then classify
  the exercised scope and independent expected result. Example format:
  `scenario module::test_function` — `path/to/test.rs#test_function` — `Exercised`: precise
  behavior and assertions. Keep the target/module/test identity even when the function name is
  unique in the current tree.
- **Uncovered behavior:** TODO. List applicable uncovered behavior, or state `None` with a reason.
- **Inapplicable cases:** TODO. Use `N/A` with the surface and reason; use `Fixture-blocked` when a
  required fixture prevents an assessment. Neither classification proves support.

## Presentation prerequisites

- TODO: name required presentation mappings, prompts, choices, client consumers, or assets and link
  their source paths. State how applicable presentation behavior is covered. If none apply, say
  `None` and explain why.

## Review provenance

- **Reviewed revision:** TODO. Use the full Git commit SHA for the reviewed source state.
- **Reviewed paths:** TODO. List each repository-relative source, definition, review-map,
  presentation, and test path that supports this entry, one per line. Use `None — unassessed` when
  no review has been performed.
- **Review note:** TODO. Record relevant semantic decisions or limits not already captured above.
