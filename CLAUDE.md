# Cockatrice fork — agent context (condensed from `.cursor/rules`)

## Architecture

- **Freeform** (legacy casual) vs **ruled** (server-authoritative MTG-style engine). Do not break freeform; gate new UI/paths on ruled mode.
- **Servatrice**: auth, lobby, rooms, chat, replays. **Ruled match state** lives in Rust sidecar **`tricerules`**; C++ **relays** protobuf between clients and engine. Single writer of ruled state; Servatrice **filters** hidden info per player.
- **Determinism**: seeded RNG; replays → `(seed, command log) → state`.
- **Card model** (hybrid, ordered by preference): rules logic lives only in **`tricerules-cards`** across three tiers — **(1) Data**: RON in `tricerules-cards/data/`, where `spell_effect` is a `Vec<SpellEffectKind>` resolved in order (e.g. `spell_effect: [DamageTarget(amount: 3, target: (kind: AnyTarget))]`); **(2) Generic primitives**: `SpellEffectKind` variants + `TargetFilter` composable targeting in `primitives.rs` — `TargetFilter { kind, not_artifact, tapped, … }` replaces the old flat `TargetSpec` enum, enabling characteristic-based restrictions without new variants; **(3) Custom Rust**: a `custom/` `CardEffect` for logic data can't express (complex/conditional targeting, unique effects) — *not built yet; add when first needed, trust the tree until then*. Card data is **validated at startup** (`registry.rs`). Prefer the lowest tier that works. **Only source of truth for rules logic.**

### Where to look

| Area | Path |
|------|------|
| Rust rules + sidecar | `tricerules/` (`tricerules-core`, `tricerules-cards`, `tricerules-proto`, `tricerules-server`) |
| Shared protobufs | `libcockatrice_protocol/libcockatrice/protocol/pb/` |
| Server ruled integration | `libcockatrice_network/.../server/remote/game/` (`server_game`, `rules_relay`) |
| Desktop | `cockatrice/` |
| Build | `WITH_RULES_ENGINE` → `cargo` for sidecar; CI: `cargo test`, `clippy -D warnings`, `fmt --check` on Rust touches |

### Ruled work is end-to-end

Unless scoped **backend-only**, ship **engine + proto + Servatrice relay + Cockatrice UI** together for commands, prompts, targets, phases, visible state. Minimal viable UI (button/menu/click) is enough. `.proto` changes must keep **C++ and Rust** buildable.

### Two card databases — never mix them

| Database | Owner | Purpose |
|----------|-------|---------|
| `cards.xml` (Oracle/Scryfall) | Cockatrice client + freeform | Display: images, names, type lines, search. Loaded by `CardDatabaseManager`. |
| `tricerules-cards` (RON `data/` + `primitives.rs`; later `custom/` Rust) | tricerules rules engine | Rules logic: types, costs, abilities, effects. Queried via `CardRegistry::global()`. |

**Rules from tricerules, display from Oracle — never the other way around.**
- Servatrice must query tricerules (via protobuf or `CardRegistry`) for card type/mechanical info, **not** `CardDatabaseQuerier` / Oracle.
- Functions that query Oracle for ruled decisions are wrong; fix them to use tricerules data.
- Oracle is intentionally absent from `tricerules/` and must stay that way.
- Future card abilities go in the hybrid model — RON data → generic `SpellEffectKind` primitive → `custom/` Rust when data is insufficient — never in Oracle text parsing.

**Card identity is engine-owned.** Decks cross IPC as Oracle *names* (`PlayerDeck.mainboard_card_name`); the engine resolves them via `CardRegistry::id_for_name` and answers with a server-only `CardCatalog` event (engine `card_id` ↔ Oracle name + types). Servatrice maps through `Server_Game::ruledCardIdForName/ruledCardNameForId` and **never derives ids from names** (the old C++ slug function is gone; don't reintroduce one). RON authoring convention: `id == slugify(name)` (`tricerules-cards/src/slug.rs`, enforced by a registry test). The catalog enumerates deck contents — keep it stripped from client broadcasts (`stripRuledServerOnlyEventsForBroadcast`).

**Ruled game start is gated on deck validation.** `doStartGameIfReady` calls the stateless `ValidateDeck` IPC (pure registry lookups, no engine session) before starting; any unimplemented mainboard card **blocks** the start — game-log message + `Event_NotifyUser CUSTOM` popup naming the cards per player, players un-readied, pregame continues. Never reintroduce a silent casual fallback for unimplemented cards (sidecar-*unreachable* still falls back to casual, but loudly via game log). The same `IpcResponse.missing_card_names` field is filled by a failing `SessionStart`; `ValidateDeck` is also the extension point for future deck-editor coverage queries.

### After implementing cards — update the checklist

After adding or finishing a card (drop a RON file anywhere under `tricerules-cards/data/` — `build.rs` embeds it automatically, no `registry.rs` edit; `primitives.rs` only when adding a new primitive), **regenerate the tracker** so `tricerules/CARDS.md` stays accurate, and commit it with the card change:

```powershell
./scripts/gen-card-checklist.ps1   # Windows: reads the registry + Oracle cards.xml → tricerules/CARDS.md
```
```bash
./scripts/gen-card-checklist.sh    # Linux/macOS: same, defaults to ~/.local/share/Cockatrice/Cockatrice/cards.xml
```

- **Partial cards:** if a card's mechanics don't fully match Oracle/CR, add `partial: "<what's missing>"` to its RON (e.g. an unimplemented mode or unenforced targeting restriction). Omit the field for fully-implemented cards. The generator renders three tiers: `[x]` full · `[ ] 🟡 partial: <note>` · `[ ]` not implemented.
- Implemented status comes from `CardRegistry`; set grouping (first/original printing) comes from Oracle — consistent with "rules from tricerules, display from Oracle". The `partial` field is tracking-only and ignored by the engine.
- **Name gate:** run with `--check` before committing card additions — exits nonzero if any registry card name has no Oracle match. Deck resolution is by name at session start, so an unmatched name means an uncastable card. (Local/pre-commit gate; CI can't host `cards.xml` — the CI-side guarantee is the registry `id == slugify(name)` test.)

### Editing habits

Small diffs; preserve legacy paths unless migrating is the task.

### Primitives vs. custom Rust — where to draw the line

`SpellEffectKind` is the shared effect type for spells, activated abilities, and triggered abilities — all three use it. The intended future design has the engine resolve everything through a uniform `CardEffect` trait (`HashMap<CardId, Box<dyn CardEffect>>` at startup), with data-driven and custom Rust cards looking identical to the engine. **That trait doesn't exist yet** — currently the engine pattern-matches `SpellEffectKind` directly. The custom tier is intentionally deferred until the primitive layer is mature and a real card forces the design.

**Add a new primitive** when the effect is a parameterized operation fully described by static data — even if it's unusual or affects many objects. Examples that belong as primitives:
- `DestroyAll { kind: TargetKind }` — Wrath of God, Day of Judgment
- `DamageAll { amount: u32, kind: TargetKind }` — Pyroclasm, Earthquake
- `TargetFilter { zone: Some(Graveyard), … }` — graveyard-scoped targeting for Reanimate, Regrowth, etc.
- `SearchLibrary { filter: CardFilter, destination: Zone, shuffle: bool }` — Demonic Tutor, Entomb
- Any new `SpellEffectKind` variant where the card's behavior is fully described by `(effect_kind, parameters)`

**Use the custom tier as an escape hatch** only when the resolution algorithm itself is unique — when the effect requires a mid-resolution player choice over live game objects, or multiple players making interdependent choices over the same revealed set. Examples:
- **Brainstorm**: draw 3, then the player must *choose which 2 cards from their hand* to put back on top *in a chosen order* — multi-step player interaction during resolution
- **Gifts Ungiven**: search for up to 4 cards, then *opponent* chooses 2 to put in your graveyard — two players making sequential choices over the same revealed set

The custom tier is narrow by design. When in doubt, ask: *can I describe this effect completely with `(effect_kind, parameters)`?* If yes, it's a primitive.

**Design primitives for reuse, not for the card at hand.** When adding a new `SpellEffectKind`, `TriggerCondition`, `AbilityCost`, or `Keyword`, always ask: *what is the most general parameterization that covers the current card AND the next 5–10 similar cards?* A specific single-card variant (e.g. `WheneverControllerCastsEnchantmentSpell`) is a missed opportunity; a parameterized one (e.g. `WheneverPlayerCastsSpell { caster, spell_type }`) serves Argothian Enchantress, Talrand, Young Pyromancer, and Guttersnipe from the same code path. Concrete rule: before committing a new primitive, name at least two real MTG cards it covers; if you can only name one, widen the parameters until you can name two.

**Before building the custom tier**, clean up `TriggeredEffect::PumpSelf` — it exists only because triggered effects can't reference self as a target. Add `TargetKind::Self_` to `TargetFilter`, express pump-self as a normal `PumpTarget` with a self-filter, and collapse `TriggeredEffect` into plain `SpellEffectKind`. Then a future `SpellEffectKind::Custom(...)` variant serves spells, activated abilities, and triggered abilities uniformly.

### Engine functions and data structures — same generality rule

The reuse principle that governs primitives applies equally to engine infrastructure: `engine.rs` helpers, proto fields, `GameState` extensions, and `LegalActions` entries.

**Before adding a new engine function or proto field**, ask: *what is the most general form that covers the current need AND the next 2–3 similar features?* A function scoped to a single mechanic is a missed opportunity. Examples:

- `compute_spell_targets` / `fill_legal` are correct: they iterate all objects and players through the existing legality functions uniformly, so every targeted spell and every targeted activated ability gets coverage from the same code path — not a per-card check.
- A hypothetical `fill_lightning_bolt_targets` or `non_targetable_permanent_ids` would be wrong: they embed a mechanic-specific view into general infrastructure.
- A proto field like `hexproof_permanent_ids` in `LegalActions` is wrong; the field `valid_targets_by_hand_slot` (a map from hand slot to full inclusion set) is correct because it scales to any number of targeted spells without a new field per mechanic.

**Concrete rule**: before adding a new field to `LegalActions`, `GameState`, or a shared proto message, name at least two distinct game mechanics that will use it. If only one mechanic would use it, the field is probably too specific — either generalize the key/value type or fold the information into an existing structure.

**Engine helpers follow the same parameterization discipline as primitives.** A helper that takes `effects: &[SpellEffectKind]` and a `caster: PlayerId` can serve spells, activated abilities, and (eventually) triggered abilities. A helper that takes `card_name: &str` and branches on it belongs in neither the engine nor the card layer.

---

## Card implementation — look up before you code

**Before implementing any card**, fetch its current Oracle data from Scryfall:

```
https://api.scryfall.com/cards/named?exact=<Card+Name>
```

Verify and use the response for:
- **Mana cost** (`mana_cost`) — including color, generic, hybrid, Phyrexian pips
- **Power/toughness** (`power` / `toughness`) — exact values; never guess from memory
- **Oracle text** (`oracle_text`) — the authoritative rules text; CR takes precedence for mechanics not spelled out on the card
- **Type line** (`type_line`) — supertypes, types, subtypes

If the fetch fails or the card name is ambiguous, surface that before writing any RON or Rust — don’t fall back to memory.

---

## MTG rules (CR + Oracle)

- **Authority**: Comprehensive Rules for mechanics; **Oracle** for card-specific behavior. Look up CR (and rulings when needed); don’t silently ship “almost right.” Intentional simplifications must be explicit.
- **`tricerules/**/*.rs`**: server-authoritative; `EngineError::Illegal` not panic; reject ambiguous combat; explicit priority/steps; add/update `tricerules-core/tests/scenario.rs` (happy + illegal path, assert steps/priority/zones). Keep `ruled_v1.proto` aligned across consumers.

**Final summary (substantive ruled/proto/relay/UI edits):** End with a short **MTG applicability** block: (1) does CR/Oracle govern this? (2) if yes — concepts + compliance or stated deferral; (3) if no — e.g. “No MTG rules surface area.”

---

## Linux build and test (after repo edits on Linux)

### Prerequisites (Ubuntu/Debian, one-time)

```bash
sudo apt install -y cmake ninja-build \
  qtbase5-dev qtbase5-dev-tools libqt5svg5-dev libqt5concurrent5 \
  libqt5websockets5-dev qtmultimedia5-dev \
  protobuf-compiler libprotobuf-dev libssl-dev ccache pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"   # or add to ~/.bashrc
```

### Build (from repo root)

```bash
# Full build — client + server + oracle + tests:
cmake --preset unix-ninja-debug -DWITH_CLIENT=ON -DWITH_SERVER=ON -DWITH_ORACLE=ON -DTEST=ON
cmake --build build/unix-ninja-debug -j$(nproc)

# Or release:
cmake --preset unix-ninja-release -DWITH_CLIENT=ON -DWITH_SERVER=ON -DWITH_ORACLE=ON
cmake --build build/unix-ninja-release -j$(nproc)
```

### Running C++ tests

```bash
QT_QPA_PLATFORM=offscreen ctest --test-dir build/unix-ninja-debug --output-on-failure
```

`QT_QPA_PLATFORM=offscreen` replaces Windows `-platform offscreen` — required for headless Qt widget tests.

### Running Rust (tricerules) tests

```bash
cd tricerules && cargo test
```

### CI checks (run before pushing Rust changes)

```bash
source "$HOME/.cargo/env"
cd tricerules
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

Qt module additions for new tests go in `cmake/FindQtRuntime.cmake` (`_TEST_NEEDED`), not per-test CMakeLists. Widget visibility checks use `isHidden()` (not `isVisible()`) since the widget is not shown during tests.

---

## Windows verify build (after repo edits on Windows)

From repo root:

```bash
cmake --preset windows-msvc-all && cmake --build --preset windows-msvc-all-release
```

Needs `QTDIR` (Qt 6 MSVC), `cargo`, MSVC. Fix failures; fix warnings you introduced or that are trivial. Pre-existing unrelated warnings — note briefly. Non-Windows: use project’s other CMake presets, not this one. Read-only investigation: no build required.

### Running C++ tests (Windows)

Configure with `-DTEST=ON`, then build and run. Qt DLLs must be on PATH; use `-platform offscreen` for headless execution.

```powershell
cmake --preset windows-msvc-all -DTEST=ON
cmake --build --preset windows-msvc-all-release --target game_prompt_widget_test

$env:PATH = "C:\Users\pizza\CodingProjects\Cockatrice\6.6.3\msvc2019_64\bin;$env:PATH"
.\build\windows-msvc-all\tests\game_prompt\Release\game_prompt_widget_test.exe -platform offscreen
```

Or run all tests via ctest (same PATH requirement applies):

```powershell
$env:PATH = "C:\Users\pizza\CodingProjects\Cockatrice\6.6.3\msvc2019_64\bin;$env:PATH"
ctest --test-dir build/windows-msvc-all -C Release -R game_prompt_widget_test --output-on-failure
```

Qt module additions for new tests go in `cmake/FindQtRuntime.cmake` (`_TEST_NEEDED`), not per-test CMakeLists. Widget visibility checks use `isHidden()` (not `isVisible()`) since the widget is not shown during tests.

---

## Ruled prompt UI (`game_prompt_widget`, `tab_game`)

- Logic in `cockatrice/src/game/prompt/game_prompt_widget.{h,cpp}`; **TabGame** = placement + signals only.
- Prompt in right **Messages** dock **above** game log; no extra dock unless asked.
- Reuse paths: e.g. **Pass Priority** → `GameEventHandler::handleNextTurn()`. Ruled + non-replay only; unchanged freeform/replay.
- Prefer existing signals (`ruledEnginePromptFeed`, `logActivePhaseChanged`, `logActivePlayer`) over new proto for UI text. Compact, action-first.

