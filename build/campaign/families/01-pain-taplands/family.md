# Family: 01-pain-taplands (5 cards)

## Selection

Queue-v26 Multi-clause composition, 5 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v26/revalidation.tsv`.

Stable typed shape: Entered-tapped land with `{T}: Add {C}` and `{T}: Add {X} or {Y}` plus 1 self-damage.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 117a5fae-7fc4-4e24-b646-5727ea392fa7 | Skyshroud Forest |  | Land | — | This land enters tapped. / {T}: Add {C}. / {T}: Add {G} or {U}. This land deals 1 damage to you. |
| 2896f01d-003b-4d68-9d0a-64990ba59cbe | Scabland |  | Land | — | This land enters tapped. / {T}: Add {C}. / {T}: Add {R} or {W}. This land deals 1 damage to you. |
| 603dc263-270c-4a16-9b75-41f36fd7dfde | Pine Barrens |  | Land | — | This land enters tapped. / {T}: Add {C}. / {T}: Add {B} or {G}. This land deals 1 damage to you. |
| 7a951bd7-4f7d-44ea-9c2f-fe2f6b2f5289 | Salt Flats |  | Land | — | This land enters tapped. / {T}: Add {C}. / {T}: Add {W} or {B}. This land deals 1 damage to you. |
| c737d27b-db14-4bd4-8f16-bcbd4401c47b | Caldera Lake |  | Land | — | This land enters tapped. / {T}: Add {C}. / {T}: Add {U} or {R}. This land deals 1 damage to you. |

## Copy-from and differences

Copy `adarkar_wastes`'s two mana abilities and add `secluded_steppe`'s unconditional `EntersTapped(affected: Self_)`; only the printed name and the two produced colors differ.

## Rules evidence

CR 614.1d/614.12 (the enters-tapped replacement effect), 605.1a/605.3a-b (mana ability and immediate resolution), 120.3a (damage causes life loss) and 118.3 (tap cost). The cycle has no pinned ruling.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v26/families/01-pain-taplands/rulings/`.

## Canaries

- Simplest: Skyshroud Forest (`skyshroud_forest`).
- Hardest: Caldera Lake (`caldera_lake`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario pain_taplands_batch::pain_taplands_batch_cards_enter_tapped`
- `scenario pain_taplands_batch::pain_taplands_batch_cards_add_colorless_or_each_color_and_deal_one_damage`
- `scenario pain_taplands_batch::pain_taplands_batch_cards_reject_a_second_activation_while_tapped`
- `mechanic_registry::pain_taplands_batch::pain_taplands_batch_maps_definitions`
- `mechanic_registry::pain_taplands_batch::pain_taplands_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended simplest canary red: `build/verification-logs/20261116-213613-424-red-simplest-pain_taplands.log` (missing definition, exit 101).
- Intended hardest canary red: `build/verification-logs/20261116-213806-469-red-hardest-pain_taplands.log` (missing definition at the hardest card, exit 101).
- Focused all-card green: `build/verification-logs/20261116-221517-504-scenario-postfix-pain_taplands.log` (exit 0).
- Registry green: `build/verification-logs/20261116-215415-217-registry-full-queue-v26.log` (full 911-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 1,974 review maps (including all 47 queue-v26 maps) (card-evidence `2f26da74-67cc-4014-8ae0-ee05190d84f4`).
- Metadata refresh: only the 47 expected fingerprint rows and their `CARDS.md` checkboxes plus the `Implemented:` total changed.
- Formatting and focused lint: pending the batch preflight.

## Review and gate

Independent read-only review approved the family in the first pass; no material findings.

The batch queue-v26 combined full gate `verify-00668ab2-d089-4ca9-85bb-c6a48f810a49` passed exit 0 on the unchanged nine-family tree, covering `01-pain-taplands` through `09-mill-two`. Code commit `b01d2c8e9e8f497e0cbb8418e94a2f7f628cefa3`. Metadata refresh changed only the 47 expected fingerprint rows, their `CARDS.md` checkboxes and the `Implemented:` total.
