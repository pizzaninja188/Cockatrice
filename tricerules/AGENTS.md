# tricerules agent guidance

The repository-root `AGENTS.md` still applies. This file owns Rust rules, card data, card generation, Oracle/CR research, and Rust verification details.

## Two card databases — never mix them

| Database | Owner | Purpose |
|---|---|---|
| `cards.xml` (Oracle/Scryfall) | Cockatrice client and freeform | Display only: images, names, type lines, search |
| `tricerules-cards` (`data/`, `primitives/`, and custom Rust) | tricerules engine | Rules logic, costs, types, abilities, and effects |

Rules come from tricerules and display wording comes from Oracle. Never query `CardDatabaseQuerier`, `cards.xml`, or external Oracle data for a ruled mechanical decision. Rules RON stores no copied Oracle prose.

Card identity is engine-owned. Decks cross IPC as Oracle names; the engine resolves them through `CardRegistry`, emits the server-only `CardCatalog`, and Servatrice maps catalog IDs without deriving them from names. RON IDs must equal `slugify(name)`, enforced by registry tests.

Game start is gated on `ValidateDeck`. An unimplemented mainboard card blocks ruled game start, identifies the missing cards, and unreadies the players. Never add a silent casual fallback for missing implementations. A genuinely unreachable sidecar may still fall back loudly through the existing path.

## Card authoring

Before adding or changing a card, card generator, card-specific primitive, or custom effect, read
[the canonical card authoring guide](tricerules-cards/authoring/CARD-AUTHORING.md). It owns the
Scryfall and rulings lookup workflow, implementation-tier decision, RON and custom-Rust shapes,
stable identity, Oracle presentation mappings, partial-card tracking, generation, and card
completion checklist. Do not copy nearby legacy RON as a substitute for following the guide.

## Rust verification

Read `../docs/AGENT-VERIFICATION.md` before running commands. During red/green iteration, run the best matching scenario or registry test. Before completion of a Rust change, run:

- Full `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `cargo fmt --check`
- Card checklist `--check` when card data or registry names changed
- `git diff --check`
