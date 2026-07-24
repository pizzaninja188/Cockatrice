# Cockatrice fork — agent context

## ⚠️ Mandatory workflow (read first)

1. **Build and test after every code change.** Do not report a change as done until the build **exits 0** and the relevant tests pass — check the exit code (`echo $?` / `$LASTEXITCODE`), don't eyeball the log. A build can compile every source file and still **exit 1 at the link step** because a stale app/server `.exe` is still running and the linker can't overwrite it (Windows file lock / Linux `ETXTBSY`, "text file busy"). If a build fails *only* at link with a busy/locked binary, kill the running process and rebuild before treating the change as broken. Read-only investigation needs no build.
2. **Determine your OS before running any build/test command.** Linux/macOS → the **bash** blocks. Windows → the **PowerShell** blocks. **Never run the other platform's commands.** Your current platform is in the environment context at the top of the session.
3. **Ruled work is end-to-end.** Unless explicitly scoped backend-only, ship **engine + proto + Servatrice relay + Cockatrice UI** together (commands, prompts, targets, phases, visible state). Minimal viable UI (button/menu/click) is enough. Any `.proto` change must keep **both C++ and Rust** buildable.
4. **Don't break freeform.** Gate all new UI/paths on ruled mode.
5. **Small diffs.** Preserve legacy paths unless migrating is the task.
6. **Structural refactors follow [docs/REFACTOR-ROADMAP.md](docs/REFACTOR-ROADMAP.md)** — it fixes the execution order and the standing rules (upstream files get extraction-only treatment with thin hooks, never in-place restructuring; new fork-owned C++ files use the `ruled_` prefix; stay player-set-generic). Read it before any refactor or cross-component structural change.

---

## Architecture

- **Freeform** (legacy casual) vs **ruled** (server-authoritative MTG-style engine).
- **Servatrice** (C++ server): auth, lobby, rooms, chat, replays. It **relays** protobuf between clients and the engine and **filters** hidden info per player.
- **`tricerules`** (Rust sidecar): the single writer of ruled match state. Rules logic lives here and nowhere else.
- **Determinism**: seeded RNG; replays reconstruct via `(seed, command log) → state`.

### Where to look

| Area | Path |
|------|------|
| Rust rules + sidecar | `tricerules/` (`tricerules-core`, `tricerules-cards`, `tricerules-proto`, `tricerules-server`) |
| Shared protobufs | `libcockatrice_protocol/libcockatrice/protocol/pb/` (`ruled_v1.proto`) |
| Server ruled integration | `libcockatrice_network/.../server/remote/game/` (`server_game`, `rules_relay`) |
| Desktop client | `cockatrice/` |
| Ruled prompt UI | `cockatrice/src/game/prompt/game_prompt_widget.{h,cpp}` |

---

## Two card databases — never mix them

| Database | Owner | Purpose |
|----------|-------|---------|
| `cards.xml` (Oracle/Scryfall) | Cockatrice client + freeform | **Display only**: images, names, type lines, search. Loaded by `CardDatabaseManager`. |
| `tricerules-cards` (RON `data/` + `primitives.rs`; later `custom/` Rust) | tricerules engine | **Rules logic**: types, costs, abilities, effects. Queried via `CardRegistry::global()`. |

**Rules from tricerules, display from Oracle — never the other way around.**
- For any ruled/mechanical decision, query tricerules (via protobuf or `CardRegistry`), **not** `CardDatabaseQuerier` / Oracle. Functions that query Oracle for ruled decisions are bugs — fix them.
- Oracle is intentionally absent from `tricerules/` and must stay that way.

**Card identity is engine-owned.** Decks cross IPC as Oracle *names* (`PlayerDeck.mainboard_card_name`); the engine resolves them via `CardRegistry::id_for_name` and answers with a server-only `CardCatalog` event (engine `card_id` ↔ Oracle name + types). Servatrice maps through `Server_Game::ruledCardIdForName/ruledCardNameForId` and **never derives ids from names** (the old C++ slug function is gone — don't reintroduce it). RON convention: `id == slugify(name)` (`tricerules-cards/src/slug.rs`, enforced by a registry test). Keep the catalog stripped from client broadcasts (`stripRuledServerOnlyEventsForBroadcast`).

**Game start is gated on deck validation.** `doStartGameIfReady` calls the stateless `ValidateDeck` IPC (registry lookups, no engine session) before starting. Any unimplemented mainboard card **blocks** the start: game-log message + `Event_NotifyUser CUSTOM` popup naming the cards per player, players un-readied, pregame continues. **Never reintroduce a silent casual fallback for unimplemented cards** (sidecar being *unreachable* still falls back to casual, but loudly via game log). The same `IpcResponse.missing_card_names` field is filled by a failing `SessionStart`.

---

## The card model — three tiers, prefer the lowest

Rules logic for a card lives in `tricerules-cards`, validated at startup (`registry.rs`). Use the lowest tier that works:

1. **Data (RON in `tricerules-cards/data/`)** — `spell_effect` is a `Vec<SpellEffectKind>` resolved in order, e.g. `spell_effect: [DamageTarget(amount: 3, target: (kind: AnyTarget))]`.
2. **Generic primitives (`primitives.rs`)** — `SpellEffectKind` variants + composable `TargetFilter { kind, not_artifact, tapped, … }` (replaces the old flat `TargetSpec`; enables characteristic-based restrictions without new variants).
3. **Custom Rust (`tricerules-core/src/custom/` `CardEffect`)** — the escape hatch for a *resolution algorithm* data can't express. One impl per file, keyed by a card's `custom_effect: Some("key")` marker (mutually exclusive with `spell_effect`, enforced at registry load). Resolution is **resumable**: `begin`/`resume` drive a capability-narrowed `ResolutionCtx` and either finish or return a `ResolutionInterrupt` (a player choice); the engine parks it in `GameState::pending_resolution`, emits the generic `RuledEvent.resolution_choice_required`, and resumes on the `SubmitResolutionChoice` command. Determinism holds because every choice is a logged command. Implemented: **Brainstorm**, **Gifts Ungiven**.

`SpellEffectKind` is shared by spells, activated abilities, and triggered abilities. `TriggeredAbilityDef.effect` is a plain `SpellEffectKind` (the old `TriggeredEffect`/`PumpSelf` wrappers are gone). A self-referencing ability uses `TargetKind::Self_` (auto-bound to the source, not "targeting" per CR 115; rejected in `spell_effect` via `EffectContext::Spell`), e.g. `PumpTarget { target: (kind: Self_) }`.

### Primitive vs. custom — where to draw the line

**Add a primitive** when the effect is fully described by `(effect_kind, parameters)` static data — even if unusual or wide-reaching: `DestroyAll { kind }` (Wrath), `DamageAll { amount, kind }` (Pyroclasm), `SearchLibrary { filter, destination, shuffle }` (Demonic Tutor), graveyard-scoped `TargetFilter { zone: Some(Graveyard), … }` (Reanimate).

**Use custom Rust** only when the *resolution algorithm itself is unique* — a mid-resolution player choice over live objects, or multiple players choosing interdependently over one revealed set: **Brainstorm** (draw 3, choose 2 from hand to put back in an order), **Gifts Ungiven** (you search 4, opponent picks 2 for your graveyard).

When in doubt: *can I describe this completely with `(effect_kind, parameters)`?* If yes, it's a primitive.

**Tier-3 review rule (the gate that keeps `custom/` from becoming a scripting dump):** a card may land in `custom/` only if a reviewer agrees **no `(effect_kind, parameters)` description exists** — prefer widening a primitive every time it's close. Custom code never touches `&mut GameState`; it drives `ResolutionCtx` (audited, zone-integrity-preserving mutators only). The generic `resolution_choice_required` / `SubmitResolutionChoice` proto pair is reused by *every* tier-3 card (and later X-spells / modal spells), so a new custom card adds **no** per-card proto. Each `CardEffect` impl cites its Oracle text + CR in a header comment and carries happy + illegal scenario coverage in the appropriate `tests/scenario/<themed>.rs` submodule (same standard as engine changes).

### Design for reuse — the one rule

Before committing **any** new `SpellEffectKind`, `TriggerCondition`, `AbilityCost`, `Keyword`, engine helper, proto field, `GameState` field, or `LegalActions` entry: **name at least two real cards (or two distinct mechanics) it covers.** If you can only name one, widen the parameters until you can name two.

- ✅ `WheneverPlayerCastsSpell { caster, spell_type }` (Talrand, Young Pyromancer, Guttersnipe, Argothian Enchantress) — ❌ `WheneverControllerCastsEnchantmentSpell`.
- ✅ `valid_targets_by_hand_slot` (map → full inclusion set, scales to any targeted spell) — ❌ `hexproof_permanent_ids` in `LegalActions`.
- ✅ helpers over `effects: &[SpellEffectKind]` + `caster: PlayerId` (serve spells, activated, triggered uniformly) — ❌ a helper that takes `card_name: &str` and branches on it.

---

## Implementing a card

**1. Look up Oracle data first — never code from memory.** Fetch:
```
https://api.scryfall.com/cards/named?exact=<Card+Name>
```
If the fetch fails or the name is ambiguous, **surface that before writing any RON/Rust** — don't fall back to memory. Use the response for:
- **`mana_cost`** — copy the Scryfall brace string **verbatim** into RON (`mana_cost: "{1}{R}"`, `""` for lands). Parsed by `ManaCost`/`ManaSymbol` (`tricerules-cards/src/mana.rs`); `AbilityCost::Mana`/`TapAndMana` use the same syntax. Supported pips: `W U B R G C X` + generic integers. Hybrid/Phyrexian are supported; snow is representable but **rejected at registry load**. `{X}` is supported: the value is chosen at cast time and paid as that much generic mana (CR 107.3b). Never hand-write the old flat `"1R"` form.
- **`power`/`toughness`** — exact values, never guessed.
- **`oracle_text`** — authoritative; CR takes precedence for mechanics not spelled out on the card.
- **`type_line`** — supertypes, types, subtypes.

**2. Drop the RON anywhere under `tricerules-cards/data/`** — `build.rs` embeds it automatically (no `registry.rs` edit). Touch `primitives.rs` only when adding a new primitive.

**3. Build + test** (see below). For `tricerules/**/*.rs`: server-authoritative, return `EngineError::Illegal` (never panic), reject ambiguous combat, keep priority/steps explicit, and add/update tests in `tricerules-core/tests/scenario/` (happy + illegal path; assert steps/priority/zones). Add to the best-fit existing submodule (e.g. `combat.rs`, `spell_effects.rs`, `triggers.rs`), or create a new file if the topic warrants it — the existing split is a starting point, not a constraint. When creating a new file, add a matching `#[path = "scenario/<name>.rs"] mod <name>;` entry to the root `tests/scenario.rs`.

**4. Regenerate the card tracker** so `tricerules/CARDS.md` stays accurate, and commit it with the card change:
```bash
./scripts/gen-card-checklist.sh    # Linux/macOS — defaults to ~/.local/share/Cockatrice/Cockatrice/cards.xml
```
```powershell
./scripts/gen-card-checklist.ps1   # Windows — reads the registry + Oracle cards.xml
```
- **Partial cards:** add `partial: "<what's missing>"` to the RON (unimplemented mode, unenforced targeting restriction, …). Omit for fully-implemented cards. Tracking-only; ignored by the engine. Renders as `[x]` full · `[ ] 🟡 partial: <note>` · `[ ]` not implemented.
- **Name gate:** run with `--check` before committing — exits nonzero if any registry card name has no Oracle match (an unmatched name is an uncastable card). Local/pre-commit gate; the CI-side guarantee is the `id == slugify(name)` registry test.

### Batch-generating vanilla / french-vanilla creatures

For creatures with no rules text, or text that is **only** supported keyword abilities, don't hand-author — generate from the Scryfall bulk dump (`gen-cards`, feature-gated like `gen-checklist`):

```bash
./scripts/fetch-scryfall-bulk.sh         # Linux/macOS — downloads oracle-cards.json
./scripts/gen-cards.sh --dry-run         # preview qualifying count + skip reasons
./scripts/gen-cards.sh                    # write RON into data/generated/<letter>/
```
```powershell
./scripts/fetch-scryfall-bulk.ps1        # Windows
./scripts/gen-cards.ps1 --dry-run
./scripts/gen-cards.ps1
```

Filter: `layout == "normal"`, type line contains `Creature`, integer power/toughness, `mana_cost` parses with the supported `ManaSymbol` set (no `{X}`), text empty or solely supported keywords. Cards already in `data/` (by id or name) and slug collisions are skipped and reported. `mana_cost` is copied verbatim from Scryfall. After generating, **always** `cd tricerules && cargo test` (the registry load + `conformance` test validate and resolve every generated card) then run the checklist `--check`, before reviewing and committing. Re-running after a new set release is the incremental ingestion path (skip-existing handles overlap).

---

## MTG rules (CR + Oracle)

- **Authority**: Comprehensive Rules for mechanics, **Oracle** for card-specific behavior. Concepts are fine from memory, but **verify any exact CR rule number or verbatim citation against the official Comprehensive Rules before writing it down** (rule numbers are easy to misremember), and look up Gatherer rulings for non-obvious card interactions. Don't silently ship "almost right." Make intentional simplifications explicit.
- Keep `ruled_v1.proto` aligned across all consumers (C++ and Rust).

**Final summary** — for substantive ruled/proto/relay/UI edits, end with a short **MTG applicability** block: (1) does CR/Oracle govern this? (2) if yes — concepts + compliance or stated deferral; (3) if no — "No MTG rules surface area."

---

## Ruled prompt UI (`game_prompt_widget`, `tab_game`)

- Logic lives in `cockatrice/src/game/prompt/game_prompt_widget.{h,cpp}`; **TabGame** does placement + signals only.
- Prompt sits in the right **Messages** dock, **above** the game log; no extra dock unless asked.
- Reuse existing paths (e.g. **Pass Priority** → `GameEventHandler::handleNextTurn()`) and existing signals (`ruledEnginePromptFeed`, `logActivePhaseChanged`, `logActivePlayer`) over new proto for UI text. Ruled + non-replay only; leave freeform/replay unchanged. Compact, action-first.

---

## Build & test

> **Run only the block for your current platform** (shown in the session's environment context). The build flag `WITH_RULES_ENGINE` drives `cargo` for the sidecar. Touching Rust triggers CI `cargo test` + `clippy -D warnings` + `fmt --check`.
>
> Qt module additions for new tests go in `cmake/FindQtRuntime.cmake` (`_TEST_NEEDED`), not per-test `CMakeLists`. Widget visibility checks use `isHidden()` (not `isVisible()`), since the widget isn't shown during tests.

### Linux / macOS (bash)

```bash
# Configure + build (debug; client + server + oracle + tests):
cmake --preset unix-ninja-debug -DWITH_CLIENT=ON -DWITH_SERVER=ON -DWITH_ORACLE=ON -DTEST=ON
cmake --build build/unix-ninja-debug -j$(nproc)

# C++ tests (headless Qt needs the offscreen platform):
QT_QPA_PLATFORM=offscreen ctest --test-dir build/unix-ninja-debug --output-on-failure

# Rust / CI checks (run all three before pushing Rust changes):
cd tricerules
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```
Release preset: `unix-ninja-release` (drop `-DTEST=ON`).

### Windows (PowerShell)

```powershell
# Configure + build (Ninja tree; the script enters the VS dev shell and configures on first run):
./scripts/build-ninja.ps1                       # full tree
./scripts/build-ninja.ps1 --target servatrice   # targeted (any cmake --build args pass through)

# C++ tests — Qt DLLs must be on PATH; use offscreen for headless (single-config: no -C):
$env:PATH = "C:\Users\pizza\CodingProjects\Cockatrice\6.6.3\msvc2019_64\bin;$env:PATH"
ctest --test-dir build/windows-ninja-all --output-on-failure

# Rust / CI checks:
cd tricerules; cargo test; cargo clippy -- -D warnings; cargo fmt --check
```
Needs `cargo` + MSVC (Qt kit is vendored at `6.6.3/msvc2019_64` via the preset). The MSBuild
presets (`windows-msvc-all` / `windows-msvc-all-release`, now with `jobs: 16`) remain for CI
parity and VS IDE use — but the Ninja tree is the dev loop: measured 2026-07, no-op build
0.4 s vs 7.9 s and one-file rebuild 7.3 s vs 12.8 s against MSBuild. Debug flags were also
measured only ~11% faster per compile than Release on a heavy TU, so Release stays the only
iteration config (tests run against it anyway).

### Targeted verification (the iteration loop)

The full build + full suite is the **pre-commit** gate, not the per-iteration loop. While
iterating, build and test only the components your change touches:

| Change touches | Build | Tests that satisfy "relevant tests" |
|---|---|---|
| `tricerules/**/*.rs` or `data/*.ron` only | no C++ build | `cd tricerules; cargo test -p tricerules-core --test scenario <filter>` (or `-p tricerules-cards` for registry/data); clippy + fmt |
| `ruled_v1.proto` | everything (C++ **and** Rust; near-full C++ recompile is expected) | full C++ ctest + `cargo test` |
| Server (`libcockatrice_network`, servatrice) | `--target servatrice` + test targets | `ctest -R "ruled_batch_test|ruled_utils_test|ruled_e2e_smoke_test"` |
| Client only (`cockatrice/`) | `--target cockatrice` + test targets | `ctest -R game_prompt_widget_test` (plus any touched client test) |

- `ctest -R <regex>` selects by test name (see `ctest -N` for the list); `cargo test -p
  tricerules-core --test scenario <filter>` runs matching scenario tests only.
- `ruled_e2e_smoke_test` (~1 s, in the default ctest run) drives a full scripted ruled game
  through real servatrice + sidecar processes — run it after any change to the relay,
  `ruled_v1.proto`, or ruled `server_game` paths, and before/after every extraction PR
  (roadmap Step 3). It SKIPs if the servatrice or tricerules-server binary is missing.
- Before **commit**: full build of the affected side(s) + full ctest and/or `cargo test`,
  clippy, fmt — per the blocks above.
