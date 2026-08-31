# tricerules agent guidance

The repository-root `AGENTS.md` still applies. This file owns Rust rules, card data, card generation, Oracle/CR research, and Rust verification details.

## Two card databases — never mix them

| Database | Owner | Purpose |
|---|---|---|
| `cards.xml` (Oracle/Scryfall) | Cockatrice client and freeform | Display only: images, names, type lines, search |
| `tricerules-cards` (`data/`, `primitives/`, and custom Rust) | tricerules engine | Rules logic, costs, types, abilities, and effects |

Rules come from tricerules and display wording comes from Oracle. Never query `CardDatabaseQuerier`, `cards.xml`, or external Oracle data for a ruled mechanical decision. Rules RON stores no copied Oracle prose: each spell, identified ability, modal option, cast-cost choice, resolution branch, heterogeneous search slot, and mana restriction carries stable identity plus either non-mechanical `OracleLines([..])` references into one external face or an explicit `Fallback` decision. Freeform choice labels and cast-cost prompts are forbidden. `TargetGroupDef.prompt` is the narrow exception: keep it short, effect-specific targeting guidance rather than copied Oracle wording. Missing or invalid external data must use the deterministic engine fallback, never a partial line selection.

Card identity is engine-owned. Decks cross IPC as Oracle names; the engine resolves them through `CardRegistry`, emits the server-only `CardCatalog`, and Servatrice maps catalog IDs without deriving them from names. RON IDs must equal `slugify(name)`, enforced by registry tests.

Game start is gated on `ValidateDeck`. An unimplemented mainboard card blocks ruled game start, identifies the missing cards, and unreadies the players. Never add a silent casual fallback for missing implementations. A genuinely unreachable sidecar may still fall back loudly through the existing path.

## Card model — use the lowest tier

1. **Data:** RON in `tricerules-cards/data/`; ordered `spell_effect` entries use `SpellEffectKind`.
2. **Generic primitives:** typed variants and composable filters in `tricerules-cards/src/primitives/`.
3. **Custom Rust:** only for a unique resumable resolution algorithm that static `(effect_kind, parameters)` data cannot describe.

Prefer widening a primitive whenever the effect can be fully described by static parameters. Use custom Rust for live mid-resolution or interdependent player choices such as Brainstorm and Gifts Ungiven, not for merely unusual effects.

Custom-effect registration is drop-in:

- Create `tricerules-core/src/custom/<card_id>.rs` outside `support/`.
- Export `pub(crate) static EFFECT: &dyn CardEffect = &YourType;`.
- Match the card definition ID, RON `custom_effect`, and file stem exactly.
- Keep each custom implementation one-to-one with a card ID; two cards sharing an algorithm signal that a primitive should be widened.
- Use the capability-narrowed `ResolutionCtx`; never give a custom effect `&mut GameState`.
- Reuse generic `resolution_choice_required` and `SubmitResolutionChoice`; do not add per-card protobuf.
- Cite Oracle text and applicable CR concepts in the implementation header and add happy plus illegal scenario coverage.

`SpellEffectKind` is shared by spells, activated abilities, and triggered abilities. `TargetKind::Self_` binds the source without targeting under CR 115 and is invalid in spell effects. Preserve that distinction instead of treating every effect subject as a chosen target.

## Implementing a card

1. Fetch exact Scryfall Oracle data with a User-Agent and then fetch `rulings_uri`. If lookup fails or the name is ambiguous, stop before authoring RON or Rust.
2. Copy `mana_cost` exactly in brace syntax. Verify type line, power/toughness, Oracle text, and rulings. Record presentation only as exact one-based `OracleLines` for the correct face, or explicitly choose `Fallback`; never paste Oracle prose into RON. Verify exact CR citations against the current official Comprehensive Rules text.
3. Drop RON anywhere under `tricerules-cards/data/`; `build.rs` embeds it automatically. Touch shared primitives only when the card cannot be expressed with existing data.
4. For Rust engine behavior, return `EngineError::Illegal` rather than panicking, reject ambiguous combat, keep steps and priority explicit, and add happy plus illegal scenarios with step, priority, and zone assertions.
5. Regenerate `tricerules/CARDS.md` for card-data changes and run the checklist name gate from the repository root:

```powershell
./scripts/gen-card-checklist.ps1 --check
```

Record genuine implementation gaps in `tricerules-cards/authoring/partial-cards.tsv`. Checklist tracking metadata must not be placed in rules RON or loaded by the runtime registry.

### Scryfall lookup

```powershell
$headers = @{ 'User-Agent' = 'CockatriceFork/1.0'; 'Accept' = 'application/json' }
$card = Invoke-RestMethod -Uri 'https://api.scryfall.com/cards/named?exact=Howling%20Mine' -Headers $headers
$rulings = Invoke-RestMethod -Uri $card.rulings_uri -Headers $headers
```

### Batch generation

Vanilla and supported french-vanilla creatures come from the Scryfall bulk dump; do not hand-author them. Use `fetch-scryfall-bulk` followed by `gen-cards --dry-run`, then generate. Generated RON contains stable face/ability IDs and Oracle line references, never Oracle prose. Refresh may replace only files with valid generator provenance; run `gen-cards --check` against the pinned SHA-verified snapshot to detect drift without writing. Oracle Tags are advisory only and cannot select mechanics, IDs, or presentation mappings.

After generation, run full Rust tests plus the checklist name gate. Keep RON `mana_cost` verbatim from Scryfall.

## Rust verification

Read `../docs/AGENT-VERIFICATION.md` before running commands. During red/green iteration, run the best matching scenario or registry test. Before completion of a Rust change, run:

- Full `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `cargo fmt --check`
- Card checklist `--check` when card data or registry names changed
- `git diff --check`
