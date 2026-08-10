# tricerules agent guidance

The repository-root `AGENTS.md` still applies. This file owns Rust rules, card data, card generation, Oracle/CR research, and Rust verification details.

## Two card databases — never mix them

| Database | Owner | Purpose |
|---|---|---|
| `cards.xml` (Oracle/Scryfall) | Cockatrice client and freeform | Display only: images, names, type lines, search |
| `tricerules-cards` (`data/`, `primitives/`, and custom Rust) | tricerules engine | Rules logic, costs, types, abilities, and effects |

Rules come from tricerules and display metadata comes from Oracle. Never query `CardDatabaseQuerier` or other Oracle data for a ruled mechanical decision. Oracle must remain absent from the Rust rules crates.

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
2. Copy `mana_cost` exactly in brace syntax. Verify type line, power/toughness, Oracle text, and rulings. Verify exact CR citations against the current official Comprehensive Rules text.
3. Drop RON anywhere under `tricerules-cards/data/`; `build.rs` embeds it automatically. Touch shared primitives only when the card cannot be expressed with existing data.
4. For Rust engine behavior, return `EngineError::Illegal` rather than panicking, reject ambiguous combat, keep steps and priority explicit, and add happy plus illegal scenarios with step, priority, and zone assertions.
5. Regenerate `tricerules/CARDS.md` for card-data changes and run the checklist name gate from the repository root:

```powershell
./scripts/gen-card-checklist.ps1 --check
```

Use `partial: "<missing behavior>"` only for a real implementation gap. It is tracking metadata, not a rules switch.

### Scryfall lookup

```powershell
$headers = @{ 'User-Agent' = 'CockatriceFork/1.0'; 'Accept' = 'application/json' }
$card = Invoke-RestMethod -Uri 'https://api.scryfall.com/cards/named?exact=Howling%20Mine' -Headers $headers
$rulings = Invoke-RestMethod -Uri $card.rulings_uri -Headers $headers
```

### Batch generation

Vanilla and supported french-vanilla creatures come from the Scryfall bulk dump; do not hand-author them. Use `fetch-scryfall-bulk` followed by `gen-cards --dry-run`, then generate. The generator accepts normal-layout creatures with integer P/T, supported mana symbols, and no text beyond supported keywords. It skips existing IDs, names, and slug collisions.

After generation, run full Rust tests plus the checklist name gate. Keep RON `mana_cost` verbatim from Scryfall.

## Rust verification

Read `../docs/AGENT-VERIFICATION.md` before running commands. During red/green iteration, run the best matching scenario or registry test. Before completion of a Rust change, run:

- Full `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `cargo fmt --check`
- Card checklist `--check` when card data or registry names changed
- `git diff --check`
