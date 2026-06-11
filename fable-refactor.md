# Card-Pool Scaling Readiness Plan

## Context

A progress evaluation of the ruled-mode stack found that the display database (`cards.xml`, 34,446 names, MTGJSON wizard ingestion) is already at full scale and proven, while the rules database (`tricerules-cards`, 52 cards) grows through mechanisms that don't scale and bake in migration debt with every card added:

- **B1** — `EMBEDDED_RON_CHUNKS` in `tricerules-cards/src/registry.rs:72` is a hand-edited `include_str!` array; a RON file not listed is silently absent; merge-conflict funnel.
- **B2** — Card identity is a lossy name slug (`cardNameToTricerulesId`, `ruled_utils.cpp:3`) duplicated by convention between C++ and RON ids ("must stay in sync" comment); breaks on commas, hyphens, curly apostrophes, unicode, `//` split names; zero enforcement.
- **B3** — One unimplemented card in a deck → `MissingCard` → **silent** fallback to casual play (`server_game.cpp:1866-1873`); only a server-side `qWarning`.
- **B4** — `mana_cost` is a flat string parsed per-character (`engine.rs:4152`): `"15"` reads as 6; X/hybrid/Phyrexian unrepresentable. Every RON written in this format is future migration work.
- **B5** — `CardRegistry::from_embedded()` re-parses all RON per `GameEngine::new` (`engine.rs:130`); the shared `CardRegistry::global()` (`registry.rs:66`) exists but is never used.
- **B6** — Oracle display data drives a rules decision: `ruledResolvedStackSpellGoesToBattlefield` (`server_game.cpp:125`) string-matches the Oracle type line — but only as the legacy fallback; the engine already emits `StackResolveDestination` on every resolve (`engine.rs:2164, 2173-2181`).
- **B7** — Primitive hygiene: single-card variants (`DestroyTargetTapped`, `TriggeredEffect::PumpSelf`), triggered-ability effects skipped by startup validation, and a hardcoded `resolvedName == "counterspell"` branch in the relay (`server_game.cpp:1636`).
- **Risks** — no version handshake between Servatrice and sidecar; manual authoring throughput (~50 cards to date) cannot reach even a curated pool; no batch generation despite the data tier being codegen-shaped.

**User decisions (locked in):**
1. **Engine-owned identity**: decks cross IPC as Oracle *names*; engine resolves names→ids; engine returns a per-game card catalog; the C++ slug function is deleted.
2. **Scryfall brace mana syntax**: `mana_cost: "{4}{G}{G}"`, copied verbatim from Scryfall.
3. **Block ruled game start** when a deck contains unimplemented cards (no silent fallback), with a **popup naming the cards** plus a game-log message.
4. **Defer deck-editor coverage badges** (note the extension point only).

**Phase dependencies:** Phase 0 items are independent and can land anytime. Phase 2 depends on Phase 1 (names-based IPC). Phase 6 is gated on Phases 1 and 3. Phases 3, 4, 5 are mutually independent.

Per CLAUDE.md: every phase touching engine+proto+relay ships end-to-end and keeps C++ and Rust buildable in the same commit; scenario tests (happy + illegal path) accompany engine changes; regenerate `tricerules/CARDS.md` whenever card RON changes.

---

## Phase 0 — Free wins (independent, small diffs) — ✅ DONE 2026-06-10

All four items landed and verified: `cargo test` (153) / `clippy -D warnings` / `fmt --check` green, Windows MSVC build green, manual E2E smoke confirmed resolution destinations in the client (creature → battlefield, instant → graveyard, ability → no card moved).

### ✅ 0.1 Share one `CardRegistry` across all games

**Why:** Each `GameEngine::new` re-parses every embedded RON chunk and holds a private copy of the whole card DB (`engine.rs:130`). At 30K cards that's seconds of game-start latency and a full card-DB memory copy per concurrent game. The fix is nearly free now.

**Files:** `tricerules/tricerules-cards/src/registry.rs`, `tricerules/tricerules-core/src/engine.rs`.

**Steps:**
1. `registry.rs`: change `GLOBAL` from `Lazy<RwLock<CardRegistry>>` to `Lazy<CardRegistry>` (there are no writers; the `RwLock` is speculative). `CardRegistry::global() -> &'static CardRegistry`.
2. `engine.rs`: `GameEngine.registry: &'static CardRegistry`; in `new()` replace the `from_embedded().map_err(...)` call with `CardRegistry::global()`. All `&self.registry` call sites compile unchanged (they take `&CardRegistry`).
3. Keep `from_embedded()` public — `gen_checklist` and registry unit tests use it.
4. Global init panics on invalid embedded data (`expect`) — acceptable: fail-fast at sidecar startup is the documented validation point.

**Acceptance:** `cargo test` green; no behavioral change in scenario tests.

### ✅ 0.2 Complete startup validation: triggered abilities + duplicate ids/names

**Why:** `from_embedded` validates `spell_effect` and `activated_abilities` but **skips `triggered_abilities`** (`registry.rs:35-51`), and `by_id.insert` silently last-wins on duplicate ids. Both gaps grow with card count.

**Files:** `tricerules/tricerules-cards/src/registry.rs`.

**Steps:**
1. Validate each `TriggeredAbilityDef`: for `TriggeredEffect::Effect(inner)` call `inner.validate()`; `PumpSelf` passes (until Phase 4.1 removes it).
2. Duplicate detection: if `by_id.insert(...)` returns `Some`, return `RegistryError::InvalidCard { reason: "duplicate id" }`. Same for duplicate names once the name index exists (Phase 1.2).
3. Unit tests: a RON snippet with an invalid triggered effect fails load; duplicate id fails load.

### ✅ 0.3 Auto-derive the embedded card list (kill the hand-maintained array)

**Why (B1):** Adding a card must not require editing `registry.rs`, and a file dropped from the list must be impossible.

**Files:** new `tricerules/tricerules-cards/build.rs`, `tricerules/tricerules-cards/src/registry.rs`.

**Steps:**
1. `build.rs`: walk `data/` **recursively** (so the corpus can later be organized into per-set subdirectories — 30K files in one flat dir is unworkable), collect `*.ron` sorted by path (determinism), and write `${OUT_DIR}/embedded_cards.rs` containing:
   ```rust
   pub const EMBEDDED_RON_CHUNKS: &[&str] = &[
       include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/plains.ron")),
       ...
   ];
   ```
   Use forward slashes in generated paths (portable for `include_str!`).
2. Emit `cargo:rerun-if-changed=data` (directory-level: triggers on adds/removes/edits).
3. `registry.rs`: replace the hand array with `include!(concat!(env!("OUT_DIR"), "/embedded_cards.rs"));`. Delete lines 72–130.
4. No consumer changes — same `EMBEDDED_RON_CHUNKS` symbol.

**Acceptance:** delete one entry's worth of testing — add a scratch RON file, `cargo test` picks it up without touching registry.rs; remove it again.

### ✅ 0.4 Delete the legacy Oracle permanence fallback

**Why (B6):** The engine emits `StackResolveDestination::Battlefield/Graveyard` on every resolve path (`engine.rs:2164, 2173-2181`); the Oracle-blob inference (`server_game.cpp:73-136`) only runs for `UNSPECIFIED`, which no current engine emits. It's dead legacy that queries Oracle for a ruled decision (explicitly forbidden by project rules).

**Files:** `libcockatrice_network/.../server/remote/game/server_game.cpp`.

**Steps:**
1. Verify by grep that all `StackResolved` constructions in `engine.rs` set a destination (the two known sites do).
2. In the resolve handler (`server_game.cpp:1230-1237`): replace the `else` fallback with `qWarning` + treat as graveyard (safe default; CR 608.3 — only permanent spells go to the battlefield).
3. Delete `ruledOracleTypeBlobForServerCard`, `ruledOracleTypeBlobFromEngineStackDescription`, and `ruledResolvedStackSpellGoesToBattlefield` (`server_game.cpp:73-136`). Check whether `CardDatabaseManager`/`CardDatabaseQuerier` includes are still needed by other code in the file before removing them.
4. Windows verify build (C++ touched).

**MTG note:** spell permanence on resolution (CR 608.3) now comes solely from the rules engine.

---

## Phase 1 — Engine-owned card identity — ✅ DONE 2026-06-10

All five items landed and verified: `cargo test` (160) / `clippy --all-targets -D warnings` / `fmt --check` green; Linux build + full ctest green (`ruled_batch_test` updated to seed the session catalog the way `applyRuledStartupBatch` does); `rg cardNameToTricerulesId` returns nothing; `--check` verified on synthetic Oracle XML (exit 0 all-matched / exit 1 on a missing name). Live sidecar IPC smoke on Linux: SessionStart with Oracle names → `ok=true` + CardCatalog (id+name) in the batch; deck containing "Black Lotus"/"Brainstorm" → `ok=false, error="unimplemented cards: Black Lotus, Brainstorm"` (deduped, sorted — the Phase 2 input). Client E2E ✅ confirmed 2026-06-11 (combined Phase 1/2/3/4.3 launch-script session): names-based wire format works end-to-end — opening hands, zone sync, casting, stack binding all correct with zero slug derivation in C++.

### ✅ 1.1 Rust `slugify` + transitional id test

**Files:** new `tricerules/tricerules-cards/src/slug.rs` (export from `lib.rs`), test in `registry.rs`.

1. `pub fn slugify(name: &str) -> String` — exact mirror of today's C++ (`lowercase, strip ASCII ', spaces→_`). Document: this is an id-*derivation convention* for file authoring, not a wire contract (the wire contract dies in 1.3/1.4).
2. Registry test: `for def in registry: assert_eq!(def.id, slugify(&def.name))`. This immediately catches id/name typos in all current and future RON, and Phase 6's generator reuses the function.

### ✅ 1.2 Registry name index

**Files:** `tricerules/tricerules-cards/src/registry.rs`.

1. Add `by_name: HashMap<String, String>` (key: trimmed, lowercased name → id), built during `from_embedded`; duplicate name → load error.
2. `pub fn id_for_name(&self, name: &str) -> Option<&str>` (normalizes the query the same way).

### ✅ 1.3 Proto: names in, catalog out

**Files:** `libcockatrice_protocol/libcockatrice/protocol/pb/ruled_v1.proto` (keep C++ and Rust buildable together).

1. `PlayerDeck`: add `repeated string mainboard_card_name = 3;`. Reserve field 2 (`mainboard_card_id`) — sidecar and Servatrice ship from the same tree, no external compat to preserve; remove its writers/readers in the same commit.
2. New `RuledEvent` variant `CardCatalog card_catalog = 21;`:
   ```proto
   message CardCatalog {
     message Entry {
       string card_id = 1;       // engine id, e.g. "lightning_bolt"
       string name = 2;          // Oracle name, e.g. "Lightning Bolt"
       repeated string types = 3; // from CardDefinition.types
       bool is_permanent = 4;
     }
     repeated Entry entries = 1;
   }
   ```
   Emitted once in `initial_response_batch()` (`engine.rs:3127`) covering the union of all deck card ids. This single structure serves ≥3 mechanics (zone-sync matching, stack binding, future coverage/UI lookups) — satisfies the two-mechanics proto rule.
3. `StackPushed`: add `string card_id = 5;` (empty for abilities). Ends the snake_case→name guessing for stack binding.
4. **Hidden information:** the catalog enumerates deck contents. It is **server-only**: strip it in the broadcast scrubber alongside `zone_view` (`stripRuledZoneViewForBroadcast`, near `server_game.cpp:1685`) — consider renaming the function to reflect "server-only events".

### ✅ 1.4 Engine: resolve names; Relay: match via catalog

**Files:** `tricerules/tricerules-server/src/main.rs`, `tricerules/tricerules-core/src/engine.rs`, `server_game.cpp`, `server_player.cpp`, `rules_relay.{h,cpp}`, `ruled_utils.{h,cpp}`.

Engine/sidecar:
1. `main.rs` SessionStart: read `mainboard_card_name`, resolve each through `registry.id_for_name()`; collect **all** unresolved names; on any, respond `ok=false` with the full list (consumed by Phase 2). On success pass resolved ids into `GameEngine::new` (its internal id-based path and `MissingCard` invariant stay unchanged).
2. `initial_response_batch`: emit the `CardCatalog` for the session's deck ids.
3. Set `StackPushed.card_id` wherever stack pushes are emitted.

Relay (Servatrice):
4. `startRuledSidecarSession` (`server_game.cpp:1841`): send `node->getName()` (trimmed) — delete the `cardNameToTricerulesId` call.
5. On the startup batch, parse `CardCatalog` into per-game maps on `Server_Game`: `QHash<QString /*card_id*/, Entry>` and `QHash<QString /*lowercased name*/, QString /*card_id*/>`.
6. Replace every `cardNameToTricerulesId` call with a catalog name→id lookup: `server_game.cpp:1394, 1710, 1757`; `server_player.cpp:206` (`trId` lambda in `applyRuledEngineZoneView` — plumb access via `getGame()`).
7. Stack binding: prefer `StackPushed.card_id` + catalog id→name over `normalizeRuledCardName` heuristics (`server_game.cpp:138-141, 165-181`); retain name normalization only where a display string is genuinely the input.
8. Delete `cardNameToTricerulesId` from `ruled_utils.{h,cpp}` (and its "must stay in sync" contract).

### ✅ 1.5 Make the checklist's name check enforceable

**Files:** `tricerules/tricerules-cards/src/bin/gen_checklist.rs`, `scripts/gen-card-checklist.ps1`, CLAUDE.md note.

1. Add `--check`: if the existing `unmatched` set (`gen_checklist.rs:296-322`) is non-empty, exit nonzero. (`gen-card-checklist.ps1` already passes through extra args.)
2. Document: run `./scripts/gen-card-checklist.ps1 --check` before committing card additions. (CI can't easily host `cards.xml`; this stays a local/pre-commit gate. The CI-runnable guarantee is the 1.1 slug test.)

**Acceptance (whole phase):** full ruled game E2E (launch scripts) with a real deck — opening hands, zone sync, casting, stack binding all work with zero slug derivation in C++; `rg cardNameToTricerulesId` returns nothing.

---

## Phase 2 — Block ruled game start on unimplemented cards — ✅ DONE 2026-06-10

Both items landed and verified: `cargo test` (164) / `clippy --all-targets -D warnings` / `fmt --check` green; Linux build + ctest green. Implementation notes vs. the plan: the gate runs in `doStartGameIfReady` right after the ready checks (before `setupZones`/`gameStarted`), using a transient stack `RulesRelay` for the stateless call; the popup goes to **every** player (content lists missing cards per player with `xN` copy counts, alphabetical) via `Event_NotifyUser CUSTOM` through `getUserInterface()->sendProtocolItem` — reachable from `Server_Game`, no `GameEventContainer` fallback needed; the SessionStart belt-and-braces path makes `startRuledSidecarSession` return `bool` (false = blocked on missing cards → `gameStarted` unwound; replay bookkeeping from the aborted start is accepted for this rare race); sidecar-unreachable keeps the casual fallback but posts a loud `Event_GameSay` at validation time. Optional 2.2.5 (deck-select-time warning) was **not** implemented. Client E2E ✅ confirmed 2026-06-11: ready-up with an unimplemented card shows the popup + game-log message and does not start; swapping to a fully-implemented deck starts ruled.

### ✅ 2.1 Stateless `ValidateDeck` IPC

**Files:** `ruled_v1.proto`, `tricerules/tricerules-server/src/main.rs`, `rules_relay.{h,cpp}`.

1. Proto:
   ```proto
   // IpcEnvelope.msg oneof:
   ValidateDeck validate_deck = 4;
   message ValidateDeck { repeated string card_names = 1; }
   // IpcResponse:
   repeated string missing_card_names = 4;  // also filled on SessionStart failure
   ```
2. Sidecar: handle `ValidateDeck` without an engine — pure `registry.id_for_name()` lookups; `ok = missing.is_empty()`. SessionStart's failure path (1.4) fills the same `missing_card_names` field.
3. `RulesRelay::validateDeck(const QStringList &names, ruled::v1::IpcResponse &out)` — same framing as existing calls.
4. This IPC is the extension point for the **deferred** deck-editor coverage feature (a future `ListImplementedCards` sibling).

### ✅ 2.2 Gate game start; popup + game log

**Files:** `server_game.cpp` (`doStartGameIfReady`, `server_game.cpp:534`), client popup handling.

1. Extract the deck-name-gathering loop from `startRuledSidecarSession` (`server_game.cpp:1833-1848`) into a helper returning `QList<QPair<int, QStringList>>` of **names** (shared by validation and session start).
2. In `doStartGameIfReady`, **before** `gameStarted = true` (`server_game.cpp:567`): if `ruledGame`, call `validateDeck` with the union of all players' mainboard names. If any missing:
   - Send `Event_GameSay` to the game (established server pattern, `server_game.cpp:892`) listing missing cards per player: *"Cannot start ruled game — unimplemented cards: Black Lotus, Brainstorm (Alice); …"*. Aggregate duplicates with counts.
   - Send each player a popup via `Event_NotifyUser` with `type = CUSTOM`, `custom_title`/`custom_content` (proto already exists: `event_notify_user.proto`; locate the established server-side sender by grepping `Event_NotifyUser` usages in servatrice — it is a session event, sent through the user's protocol handler, and the client already renders CUSTOM notifications). If the session interface turns out not to be reachable from `Server_Game` context, fall back to a dedicated `GameEventContainer` route — decide at implementation; the popup is a hard requirement.
   - Reset every player's `readyStart` to false and `sendGameStateToPlayers()` so the pregame UI reflects the un-ready state; `return` without starting.
3. Sidecar **unreachable** during validation is a different failure: keep the current behavior for that case only (game starts casual) but add a loud `Event_GameSay` — the user's block applies to unimplemented cards, not infrastructure outages.
4. Belt-and-braces: if `SessionStart` itself later returns missing cards (race with a deck swap), replace today's silent fallback (`server_game.cpp:1866-1873`) with the same block path (game does not start ruled; message + popup).
5. Optional polish: also run `validateDeck` at deck-select time (locate `cmdDeckSelect` in `server_abstract_player.cpp`) and post a *warning* `Event_GameSay` early — non-blocking, purely informational.

**Acceptance:** E2E — deck with one unimplemented card: ready-up does not start the game; popup lists the card; game log message appears; swapping to a fully-implemented deck lets the game start ruled. Scenario: `cargo test` for ValidateDeck handler.

---

## Phase 3 — Structured mana costs (Scryfall brace syntax) — ✅ DONE 2026-06-10

All three items landed and verified: `cargo test` (178) / `clippy --all-targets -D warnings` / `fmt --check` green; Linux incremental build green (no-op — no C++/proto touched). Implementation notes vs. the plan: **no `.proto` change was required** — the `battlefield_activated_ability_mana_costs` field is already `repeated string`; only its *content* moves to canonical brace `Display` (`"{4}"`). **No C++ change was required either** — the client's `PlayerActions::parseSimpleManaCost` already parses both brace and the legacy compact form, and produces identical results for every existing ability cost (`"4"`→`{4}`, `"1"`→`{1}`), so the wire switch is transparent to the client (and fixes multi-digit on that side too). `pay_mana_simple(&str)` → `pay_mana(&ManaCost)`, iterating pips; `{X}` → `EngineError::Illegal("X costs not yet supported")` at payment; `{C}` now pays from colorless mana specifically (was conflated with generic). 52 RON files converted mechanically (47 changed; 5 basic lands keep `""`). `CARDS.md` **not regenerated**: the generator reads implemented-status/`partial` only, never `mana_cost`, so the corpus conversion produces zero checklist diff (and cards.xml isn't on this Linux box). Tests: `mana.rs` unit tests (parse/multi-digit/X/unsupported/colors/Display/serde) + `engine.rs` `mana_payment_tests` (multi-digit paid, insufficient rejected, `{C}` requires colorless, X rejected cleanly). Client E2E ✅ confirmed 2026-06-11: activated-ability mana prompt computes the cost correctly from the brace-string wire format.

**Why (B4):** `"15"` parses as 6; X/hybrid/Phyrexian are unrepresentable; every RON written meanwhile is migration debt. Brace syntax means hand-authoring and Phase 6 codegen copy `mana_cost` **verbatim from Scryfall**.

### 3.1 `ManaCost` type + parser

**Files:** new `tricerules/tricerules-cards/src/mana.rs` (export from `lib.rs`).

1. ```rust
   pub enum ManaSymbol { W, U, B, R, G, C, Generic(u32), X }
   pub struct ManaCost { pub pips: Vec<ManaSymbol> }
   ```
2. Strict parser from `"{4}{G}{G}"`: the whole string must be `{...}` groups; empty string = free/no cost (lands). Recognized: `W U B R G C X` and non-negative integers. **Unsupported symbols parse-error by name** (`"unsupported mana symbol {G/U}"`) — hybrid `{G/U}`, Phyrexian `{B/P}`, snow `{S}` are representable in the syntax later without corpus churn, rejected at registry load until the engine supports them (CR 107.4 family).
3. Methods: `mana_value()` (CR 202.3; X = 0 while not on the stack), `colors()` (CR 202.2a — replaces the char-scan in `card_def.rs:58`), `Display` (canonical braces), `is_empty()`.
4. Serde as string: `#[serde(try_from = "String", into = "String")]` so RON files keep a plain string field.

### 3.2 Migrate `CardDefinition` and the engine

**Files:** `card_def.rs`, `engine.rs`, all 52 `data/*.ron`.

1. `CardDefinition.mana_cost: ManaCost`. The type change makes every consuming site a compile error — that *is* the migration checklist (mana parsing in `pay_mana_simple` at `engine.rs:4133`, any affordability checks in legality computation, `colors()`).
2. `pay_mana_simple` → `pay_mana(&ManaCost)`: iterate pips (multi-digit fixed by construction). An `X` pip ⇒ `EngineError::Illegal("X costs not yet supported")` at cast-legality and payment (clean error, no silent mis-pay).
3. Convert the RON corpus with a one-off script (`"1R"` → `"{1}{R}"`, `""` stays `""`): 52 files, mechanical. Regenerate `CARDS.md`.

### 3.3 `AbilityCost` and the activated-ability wire format

**Files:** `primitives.rs`, `engine.rs`, `server_game.cpp`/`server_player.cpp` (wherever `battlefield_activated_ability_mana_costs` is filled), `cockatrice/src/game/game_event_handler.cpp:1412-1414` (consumer).

1. `AbilityCost::Mana(ManaCost)` and `TapAndMana(ManaCost)`; RON values become brace strings (`"4"` → `"{4}"`).
2. `RuledPerPlayerView.battlefield_activated_ability_mana_costs` currently carries compact strings ("4", "R"). Switch the wire to canonical brace `Display` output and update the client consumer in `game_event_handler.cpp` (trace what downstream parses it — likely the activation menu/mana payment; align its parsing with the brace format or have the client treat it as opaque display text if that's all it is).

**Acceptance:** scenario tests for: multi-digit generic cost paid correctly; X-cost card load succeeds but cast is rejected with the explicit error; unsupported symbol RON fails registry load with a clear message. Windows verify build for the client change.

**MTG note:** mana symbols CR 107.4; X handling (CR 107.3) is explicitly deferred — representable, not castable.

---

## Phase 4 — Primitive & relay hygiene

**Status:** ✅ DONE. 4.1, 4.2, 4.4 landed 2026-06-11 (Rust-only, fully verified by CI-equivalent checks — no client-test debt). 4.3 (counterspell relay generalization) landed in `db5d7a65` via the engine `PermanentMoved` route rather than the plan's `StackResolved` sketch — see below. Verification for the Rust-only items: `cargo test` (147 core scenario+conformance, 24 cards-lib, all suites green) / `cargo clippy --all-targets -D warnings` / `cargo fmt --check` all green. No proto/C++/RON-schema-shape changes for 4.1/4.2/4.4; the only RON edits were mechanical (`royal_assassin.ron` filter + unwrapping `Effect(...)` on five trigger cards), and `CARDS.md` needs no regen (the generator reads implemented-status/`partial` only, and no card's status changed). Client E2E for 4.3 ✅ confirmed 2026-06-11 (combined session): a countered spell moves to its owner's graveyard in the client, including the cross-player case.

### ✅ 4.1 `TargetKind::Self_`; collapse `TriggeredEffect` into `SpellEffectKind`

**Landed.** Added `TargetKind::Self_` (auto-bound to source, not "targeting" per CR 115; `target_filter_legal` returns false for it since it's never *picked*, and the engine binds it directly at resolution). `PumpTarget` gained `target: TargetFilter` with `#[serde(default = "TargetFilter::default_creature")]` — `giant_growth.ron` unchanged. Deleted the `TriggeredEffect` wrapper: `TriggeredAbilityDef.effect` is now a plain `SpellEffectKind`, so triggered and activated abilities resolve through one path (the `pump_self_params` special case in `resolve` is gone). Added an `EffectContext { Spell, Ability }` arg to `SpellEffectKind::validate` so `Self_` is rejected in `spell_effect` but allowed on abilities; `spell_effect_kind_needs_target` returns false for `PumpTarget { Self_ }` (no prompt, auto-resolves). `triggered_effect_needs_target` removed (callers use `spell_effect_kind_needs_target`). Five RON trigger cards (`argothian_enchantress`, `elvish_visionary`, `thieving_magpie`, `scroll_thief`, `flametongue_kavu`) had their `effect: Effect(X)` unwrapped to `effect: X`. Tests: `primitives.rs` (Self_ rejected in spell / allowed in ability), `registry.rs` (`self_pump_trigger_loads_but_self_in_spell_rejected`). No RON uses `Self_` yet — it's the representation for the next self-pump card; the resolution path is covered by the existing `PumpTarget` (Giant Growth) machinery plus the registry load test.

**Landed.** `royal_assassin.ron` → `DestroyTarget(target: (kind: Creature, tapped: true))`. Deleted the `DestroyTargetTapped` variant and all four engine arms (resolution, `effect_target_legal_at_resolution`, `validate_effect_targets`, `spell_target_legality_error`); `DestroyTarget`/`DamageTarget`/`TapTarget`/`PumpTarget` now share one filter-based legality path through `target_filter_legal`, which already honors `tapped` (and `not_artifact`, hexproof/shroud). The now-unused `pump_spell_target_legal` helper was removed. Resolution-time fizzle is generic: if the target untaps before resolution, `spell_has_no_legal_targets_at_resolution` fizzles it (replacing the old hand-written "not tapped" branch). Scenario tests (`scenario.rs`): `royal_assassin_destroys_tapped_creature` (happy) and `royal_assassin_cannot_target_untapped_creature` (illegal path — rejected at validation before any cost is paid), using a new `deploy_to_battlefield` helper.

### ✅ 4.3 Generalize the counterspell relay hack — DONE (`db5d7a65`; relay E2E pending)

**Why (B7):** `server_game.cpp:1636` branched on `resolvedName == "counterspell"` to move the countered card to the graveyard — a per-card check in general infrastructure; every future counter effect would need another branch.

**Landed via `PermanentMoved`, not `StackResolved`** — a stronger fix than the plan's sketch. The plan would have routed the countered card to whichever player's zone physically held it (the shared canonical stack is owned by the lowest player-id), sending an opponent-countered spell to the *counterer's* graveyard. Correct behavior is the **owner's** graveyard, so:

1. **Engine** (`engine.rs`): when `CounterTargetSpell` resolves, emit an explicit `PermanentMoved` for the countered spell **stamped with the spell's owner**, so the relay routes it generically. Serves any counter effect (Counterspell, Negate, Mana Leak, …), no per-card name branch.
2. **Relay** (`server_game.cpp`): generalize the `PermanentMoved` handler to locate a card on the shared canonical stack (stack cards are never in the per-player engine-oid map), and delete the name-matched counterspell branch. **The stack search must precede the mill/deck fallback** — otherwise the countered card's `card_id` name-matches a different copy in the owner's library and moves that, stranding the real stacked card as a ghost and desyncing zone counts (which also blocked land untap).
3. **Test:** `countered_spell_moves_to_its_owners_graveyard` (scenario.rs) — cross-player case (P0 owns the bolt, P1 counters it). `grep` confirms no `resolvedName == "counterspell"` / `counterspell` branch remains in the relay.

Citation: the commit message cites **CR 701.5e**; this plan originally wrote CR 701.6a — the rule number drifts between CR editions, behavior (countered spell → owner's graveyard) is the same. Relay launch-script E2E ✅ confirmed 2026-06-11 (combined Phase 1/2/3/4.3 client session).

### 4.4 Test scaling

1. ✅ **Registry conformance test** (`tricerules-core/tests/conformance.rs`): `every_registered_card_resolves_without_panic` iterates **every** `CardRegistry::global()` definition in sorted order, builds a minimal 2-player game, puts the card under P0's control with ample mana and a vanilla creature on each battlefield, and performs its primary action — `play_land` for lands; for spells, the first target set the engine accepts from `{none, opp, self, enemy creature, own creature}` then drains the stack; for permanents, each activated ability with the same candidate sweep. Contract is deliberately weak so it scales with the corpus: `Illegal` is acceptable, only **panics** fail, and a zone-integrity invariant asserts every object is in exactly one place with none conjured/lost (no token primitive yet). This is the safety net that makes Phase 6's bulk import trustworthy.
2. **Deferred (optional):** split `scenario.rs` (now ~7.7K lines) into `tests/scenario/` modules by area — purely mechanical, skipped here to keep this diff reviewable.

---

## Phase 5 — Version handshake & replay stamping — ✅ DONE 2026-06-11

All items landed and verified: `cargo test` (185 across suites) / `cargo clippy --all-targets -D warnings` / `cargo fmt --check` green; Linux build (client+server+oracle+tests) exits 0 and full ctest 14/14 green. Server-side only — no client UI surface, so no client-E2E debt.

Implementation notes vs. the plan:
- **`CardRegistry::content_hash() -> String`** (`registry.rs`): FNV-1a over the path-sorted `EMBEDDED_RON_CHUNKS` (with a per-chunk separator byte so file boundaries matter), formatted as 16-char hex. No new dependency — FNV is build-to-build stable and that's all the version tag needs. Unit test asserts the digest is 16 hex chars and deterministic within a build.
- **Proto:** `SessionStart.servatrice_build = 5`; `IpcResponse.engine_build = 5`, `card_data_hash = 6`; `game_replay.proto` `GameReplay.ruled_card_data_hash = 7` (proto2 optional string). The sidecar fills `engine_build` (`env!("CARGO_PKG_VERSION")`) + `card_data_hash` only on the **successful** SessionStart response (the one Servatrice reads and stamps into the replay); failure/ValidateDeck/per-command responses leave them empty (the per-command literals in `engine.rs` use `..Default::default()`). The sidecar logs the received `servatrice_build`.
- **Servatrice** (`rules_relay.cpp` sends `VERSION_STRING`; `server_game.cpp` reads the response): logs `engine` build + `card data` hash via `qInfo` and stamps `ruled_card_data_hash` beside `ruled_seed` on the replay.
- **"Build mismatch" warning — deliberate refinement.** The plan sketched `qWarning` on `servatrice_build != engine_build`, but those live in **different version namespaces** (C++ `PROJECT_VERSION_FRIENDLY` vs. the Rust crate version) and would never be equal, making a literal equality check pure noise. Instead the `qWarning` fires when the sidecar reports an **empty** `engine_build` — i.e. it predates the handshake / was built from an out-of-tree commit. That is the genuine, actionable skew signal and directly replaces what the deleted `>2000 bytes` heuristic was crudely proxying. `card_data_hash` is the meaningful card-data version signal: logged on every start and recorded in the replay, so `(seed, command log, data hash)` reproduces a game.
- **Deleted** the `>2000 bytes` eprintln heuristic in `tricerules-server/main.rs`.

**Why:** The only skew guard today is a `>2000 bytes` eprintln heuristic (`tricerules-server/main.rs:106-111`). Card data will change weekly once the pool grows; replays don't record which card data they ran against.

**Files:** `ruled_v1.proto`, `registry.rs`, `tricerules-server/src/main.rs`, `rules_relay.cpp`, `server_game.cpp:1878-1880`, `game_replay.proto`.

1. `CardRegistry::content_hash() -> String`: stable hash over the sorted embedded chunks (add `sha2` or use a small stable FNV — needs build-to-build stability, not crypto).
2. Proto: `SessionStart.servatrice_build = 5` (string); `IpcResponse.engine_build = 5`, `IpcResponse.card_data_hash = 6`. Sidecar fills them on SessionStart responses.
3. Servatrice: log both; `qWarning` on build mismatch (refusal not warranted yet — same-tree deploys are the norm).
4. Replays: `game_replay.proto` already stores `ruled_seed` (`server_game.cpp:1878-1880`); add `ruled_card_data_hash` beside it so `(seed, command log, data hash)` fully determines a replay.
5. Delete the 2000-byte eprintln heuristic.

---

## Phase 6 — Batch vanilla/french-vanilla card generation

**Gate: Phases 1 and 3 must be landed first** — otherwise thousands of generated files bake in the slug contract and the old mana format.

**Why:** Manual authoring throughput is the dominant scaling wall. Vanilla and french-vanilla creatures (keywords ⊆ the supported `Keyword` enum) are fully expressible with existing primitives — an estimated 1–3K cards obtainable in one sweep, with zero new engine code.

**Files:** new `tricerules/tricerules-cards/src/bin/gen_cards.rs` (feature-gated like `gen_checklist`, optional `serde_json` dep), new `scripts/fetch-scryfall-bulk.ps1`, new `scripts/gen-cards.ps1`.

1. **Input:** Scryfall **bulk data** (`https://api.scryfall.com/bulk-data` → `oracle_cards` `download_uri`) — a single JSON download, no per-card API calls, no rate-limit exposure; set a proper User-Agent. This matches CLAUDE.md's Scryfall-as-authority policy.
2. **Selection filter:** `layout == "normal"`; type line contains `Creature`; not a token/funny/digital-only card; `power`/`toughness` parse as `u32` (reject `*`, `X`, negatives — schema can't express them); `mana_cost` parses with the supported `ManaSymbol` set; `oracle_text` is empty **or** consists solely of keyword lines whose every keyword maps into the supported `Keyword` enum (Flying, Reach, Intimidate, Vigilance, Lifelink, Haste, Deathtouch, Menace, Trample, First strike, Double strike, Indestructible, Hexproof, Shroud).
3. **Emission:** one RON per card into `data/generated/<first-letter>/` (recursive walk from 0.3 handles it): `id = slugify(name)` (shared fn from 1.1; on slug collision between distinct names, skip + report — ids only need uniqueness now, the wire uses names), `name`, `mana_cost` verbatim, `types`/`supertypes` from the type line, the `is_*` flags, `power`, `toughness`, `keywords`. First line: a provenance comment (`// generated by gen-cards from Scryfall bulk <date>`). Skip ids already present anywhere in `data/`.
4. **Workflow:** dry-run prints the would-be count and skip reasons → generate → `cargo test` (registry validation + 4.4 conformance test exercise every generated card) → `./scripts/gen-card-checklist.ps1 --check` → review → commit.
5. **Post-import scale checks:** registry global-init time (now once per process thanks to 0.1), sidecar binary size, `cargo test` duration. These produce the first real data for the next scaling decision (e.g., moving RON out of the binary into a data dir).
6. Re-running the generator after new set releases is the incremental ingestion story for this card class: same filter, `skip existing`, new sets fall out automatically.

---

## Explicitly out of scope (recorded, not planned)

- Multi-face cards, tokens (`CreateToken` primitive + CR 111), counters on `GameObject`, X-spell casting (proto/UI for choosing X), hybrid/Phyrexian *payment* (representation arrives in Phase 3), copy effects — each is a future structural design.
- Deck-editor coverage badges — deferred by decision; extension point is the `ValidateDeck`/future `ListImplementedCards` IPC (2.1).
- RON schema migrations tooling — policy for now: additive changes use serde defaults; breaking shape changes ship with a repo-wide migration script in the same PR.

## Verification

- **Rust (every phase):** `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` from `tricerules/` (mirrors CI).
- **C++ phases (0.4, 1, 2, 3.3, 4.3, 5):** `cmake --preset windows-msvc-all && cmake --build --preset windows-msvc-all-release`.
- **E2E (phases 1, 2, 3, 4.3):** `./scripts/launch-tricerules-server.ps1`, `./scripts/launch-local-servatrice.ps1`, `./scripts/launch-test-clients.ps1`; play a ruled game with an implemented deck (zone sync, casting, combat, stack); Phase 2 specifically: ready-up with a deck containing an unimplemented card → start blocked, popup + log message list the card.
- **Scenario tests:** each engine-behavior change adds happy + illegal path cases per CLAUDE.md (`tricerules-core/tests/`).
- **Checklist:** regenerate `tricerules/CARDS.md` whenever `data/` changes (Phase 3 corpus conversion, Phase 6 import).

## MTG applicability

CR/Oracle govern several items: spell permanence on resolution (CR 608.3 — Phase 0.4 makes the engine sole authority); mana symbols and costs (CR 107.4, 202.2a, 202.3 — Phase 3; X per CR 107.3 deferred explicitly); countered spells to owner's graveyard (CR 701.5e — Phase 4.3, landed); self-referencing ability effects are not "targeting" (CR 115 distinction — Phase 4.1). Identity, registration, validation, versioning, and codegen phases have no MTG rules surface beyond fidelity to Oracle names/costs.
