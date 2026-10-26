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

Follow [the execution path](../docs/AUTHORING-WORKFLOW.md). Routine cards default to adapting the
closest shipped definition, map and relevant test assertions. Retain its valid representation;
investigate semantic differences rather than rediscovering unchanged APIs. Use
[the authoring reference](tricerules-cards/authoring/CARD-AUTHORING.md) for those differences,
unfamiliar source/authority/tier rules, or a concrete capability gap. Do not reread its opening
sections for every card. Similarity alone never establishes correctness or complete-card support.

## Rust verification

Use the applicable commands in `../docs/AGENT-VERIFICATION.md`; reuse already-read guidance and supplied exact commands. During red/green iteration, run the best matching scenario or registry test through the quiet runner with working directory `tricerules`.

For final verification, use `scripts/verify.ps1 -Side Rust` from the repository root. Add `-CardData` when card data or registry names changed; use `-Side Both` if the change also affects C++ contracts. The script runs the full Rust test, Clippy, format, and diff gates with exact exit codes. When required by card implementation, refresh Oracle presentation metadata from existing local inputs with `scripts/update-card-data.ps1 -Mode Refresh` and review the diff before the final gate; this needs no separate approval. Refresh never rewrites card RON. Updating external source datasets requires separate authorization.
