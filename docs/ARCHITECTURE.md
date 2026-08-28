# Architecture — ruled mode

Read this before any cross-component work. It describes how the three processes fit together,
what each one owns, and the conventions that are impossible to re-derive from a single file.

Companions: **[AGENTS.md](../AGENTS.md)** (day-to-day rules for implementing cards and shipping
changes) and **[REFACTOR-ROADMAP.md](REFACTOR-ROADMAP.md)** (structural work, standing design
rules, the trigger-gated backlog). Where they overlap, they win on their own subject: AGENTS.md
on workflow, the roadmap on what to restructure, this file on how the pieces relate.

---

## 1. System

Two game modes share one client and one server:

- **Freeform** — the legacy casual mode. The client is trusted; players move cards by hand;
  there are no rules. Untouched by this fork except where ruled code is gated off.
- **Ruled** — server-authoritative MTG. A Rust engine is the only thing that decides what is
  legal, and the client can only ask.

```
┌────────────────────┐   Command_RuledPayload    ┌──────────────────────┐   IpcEnvelope    ┌───────────────────┐
│ Cockatrice client  │ ────────────────────────► │ Servatrice           │ ───────────────► │ tricerules-server │
│ (Qt, untrusted)    │                           │ (C++, trusted)       │  (TCP, loopback) │ (Rust sidecar)    │
│                    │ ◄──────────────────────── │                      │ ◄─────────────── │                   │
└────────────────────┘   Event_RuledPayload      └──────────────────────┘   IpcResponse    └───────────────────┘
        view model                                  relay + redaction                       the rules engine
```

| Process | Owns | Explicitly does not own |
|---|---|---|
| **tricerules** (Rust) | *All* rules state: zones, stack, priority, turn structure, combat, mana pools, continuous effects, legality, targeting, RNG. The single writer. | Anything visual. No Oracle/`cards.xml` data exists in `tricerules/`, by design. |
| **Servatrice** (C++) | Accounts, lobby, rooms, chat, replays; the *physical* Cockatrice card objects and zones; per-player hidden-info redaction; seat↔player identity. | Rules. It never decides legality and never derives engine card ids. |
| **Cockatrice** (Qt) | Rendering, input, and a read-only mirror of what the engine told this seat. | Everything else. It is a display; treat any client-side "rule" as a bug. |

**Determinism.** A ruled session is reproducible from `(seed, command log, card_data_hash)`.
Every shuffle derives its `StdRng` from `seed` (opening deal) or from `seed` + `command_index`
(mulligan, library search), and `command_index` also stamps CR 613.7 continuous-effect
timestamps. It advances **only for accepted commands** (`engine/mod.rs::apply_command`) — a
rejected command must not perturb replay, since replay re-applies only the accepted ones.
Servatrice appends every accepted `RuledCommand`'s bytes to `GameReplay.ruled_command_log` and
stamps `ruled_card_data_hash` from the sidecar's `SessionStart` handshake. Any mid-resolution player decision is itself a logged command
(`SubmitResolutionChoice`), which is why tier-3 resolution is park/resume rather than a
callback — see §6.

### Where things live

| Area | Path |
|---|---|
| Rules engine + sidecar | `tricerules/` (`-core`, `-cards`, `-proto`, `-server`) |
| Shared protobuf | `libcockatrice_protocol/libcockatrice/protocol/pb/ruled_v1.proto` |
| Server ruled integration | `libcockatrice_network/libcockatrice/network/server/remote/game/ruled_*.{h,cpp}`, `rules_relay.{h,cpp}` |
| Client ruled view model | `cockatrice/src/game/ruled/` — see its [README](../cockatrice/src/game/ruled/README.md) |
| Ruled prompt panel | `cockatrice/src/game/prompt/game_prompt_widget.{h,cpp}` |

---

## 2. Life of a command — casting Lightning Bolt

One concrete trace. Every ruled action follows this shape; only the `RuledCommand` variant and
the engine entry point differ.

**1 — Click (client, `cockatrice/src/game/ruled/`).**
`CardItem`'s upstream click handler calls the guard
`if (RuledActions::tryHandle…(this)) return;`. `RuledActions::resolveHandActionIndex(state,
HAND_ACTION_CAST_SPELL, card)` maps the clicked `CardItem` to an **engine hand slot** using the
`RuledHandActionSet` the last batch delivered. `handActionNeedsTarget(kind, slot)` says Bolt
needs a target, so `PlayerActions` enters its pending-cast state machine and asks
`RuledClientState::isValidSpellTarget(slot, face, oid)` about every click — the legal target set
is engine-supplied (`LegalActions.valid_targets_by_hand_slot`), never re-derived from Oracle text.

**2 — Pay and send (client).** Tapping a land is itself a ruled command (`ActivateAbility` on a
mana ability), relayed and broadcast the same way; the mana pool the client shows is the engine's
`ManaPoolUpdated` snapshot, not a local tally. On confirm, `PlayerActions` builds
`ruled::v1::RuledCommand{ cast_spell: { hand_card_index, targets, x_value, flex_payments,
face_index } }`, and `GameEventHandler::sendRuledCommand` wraps the serialized bytes in
`Command_RuledPayload` (a `GameCommand` extension).

**3 — Admit (Servatrice, upstream hook).**
`Server_AbstractParticipant::processGameCommand` runs the **ruled command allowlist**: in a ruled
game only chat/leave/concede/judge/kick, the pregame deck commands, `RULED_PAYLOAD`, and the
still-freeform visual `SET_CARD_ATTR`/`INC_COUNTER`/`SET_COUNTER` are accepted; everything else is
`RespInvalidCommand`. The `playerId` passed on is the participant's own server-side member — a
client cannot act as another seat. Then
`Server_Game::processRuledPayload` → `RuledGameDriver::processRuledPayload`.

**4 — Relay (Servatrice → sidecar).** The two `Preview*` commands are answered locally (they are
UI courtesy, never sent to the engine). Everything else goes to
`RulesRelay::playerCommand`, which writes a length-prefixed `IpcEnvelope{PlayerCommand}` on the
loopback socket and blocks for the `IpcResponse` — synchronously, under `Server_Game::gameMutex`.
That is deliberate (see the roadmap's "Do NOT do": don't make the relay async).

**5 — Decide (engine).** `GameEngine::player_command_ipc` → `apply_command` → `dispatch_command`,
which rejects previews, routes concede first, gates on a parked resolution, and dispatches to
`cast_spell`. On success `dispatch_command` finishes the batch uniformly: `sweep_life`,
`apply_sbas` (skipped while a resolution is parked), `ev_zone_view_sync`, one
`ev_mana_pool_updated` per player, then `legal_actions::fill_legal`. Illegal input returns
`EngineError::Illegal` — the engine never panics on a bad command.

**6 — Mirror onto physical cards (Servatrice).** Only after the engine accepts does the driver
touch Cockatrice objects: for a cast it moves the physical `Server_Card` from HAND to the
**canonical stack zone** (the lowest player-id STACK zone, so every client sees one merged stack)
and queues a `PendingRuledCastVisual` to bind to the next `StackPushed`. Then
`RuledBatchSynchronizer::applyBatch(resp)` runs its load-bearing ordered pipeline (§4), and the
accepted command's bytes are appended to the replay log.

**7 — Broadcast (Servatrice).** `RuledBroadcastRouter::broadcast` copies the batch, calls
`appendServerObjectMaps` to inject the server-built identity maps, then for **each participant**
produces a redacted copy via `redactBatchForParticipant` (§5) and sends it as a private
`Event_RuledPayload`. Each seat gets a different byte stream.

**8 — Apply (client).** `GameEventHandler`'s `RULED_PAYLOAD` case is one line into
`RuledEventDispatcher::processPayload`, which resets the per-batch legal-action state and calls one
private method per event kind, mutating `RuledClientState` and accumulating a `BatchContext`.
`finishBatch` emits the state's signals; `TabGame`, `GamePromptWidget`, the board and the message
log re-render. Bolt's effect reaches the player as `LifeChanged` (life counter + log),
`StackPushed`/`StackResolved` (stack window), a fresh `ZoneViewSync`/`BattlefieldObjectMap`, and a
new `LegalActions`.

---

## 3. Identity glossary

The most confusing thing in the codebase: six different ids for what a player thinks of as "a
card". Nothing here is interchangeable.

| Name | Type | Minted by | Scope / lifetime |
|---|---|---|---|
| **engine `ObjectId`** (`oid`) | `u32` / proto `uint32` / `quint32` | engine (`GameState::next_object_id`) | One game object, from creation until it leaves the game. Allocation starts **above** the highest `PlayerId` so a player target and an object can share the `TargetRef.object_id` field unambiguously. |
| **engine `PlayerId`** | `i32` | Servatrice seat id, mirrored into the engine | The seat. Also travels as `TargetRef.object_id` for player targets. |
| **tricerules `card_id`** (`cardId`) | `String`, `snake_case` slug | `tricerules-cards` registry; `id == slugify(name)` (enforced by a registry test) | Printing-independent identity of a *card definition*. Tokens live in a separate id namespace and are deliberately absent from the session catalog. |
| **Oracle name** | `QString` / `String` | `cards.xml` (Scryfall) | Display and deck lists. The **only** card identity that crosses IPC in `PlayerDeck`; the engine resolves it with `CardRegistry::id_for_name` (trim + lowercase; every face name of a multi-face card resolves to the one card id). |
| **`Server_Card.id`** (`serverCardId`) | `int` | Servatrice, per physical card | Unique within its owner's zones. This is what a client `CardItem` carries on the wire. |
| **engine hand slot** (`handSlot`) | index into `PlayerState.hand` | engine | Valid **only for the batch that reported it**. Any hand change renumbers it. |
| **face index** | `usize` / proto `uint32` | card definition | Index into `CardDefinition.faces`; 0 = front/primary (CR 709/712/715). |

**Rules from tricerules, display from Oracle — never the other way around.** Any mechanical
decision (is it a creature? can it be targeted? what does this ability cost?) must come from the
engine. `CardDatabaseQuerier` answers "what does it look like", nothing more.

### Who converts what

| Map | Built by | Direction | Notes |
|---|---|---|---|
| `CardCatalog` | **engine**, once, in the `SessionStart` response | `card_id` ↔ Oracle name (+ types, `is_permanent`, per-face labels and exact display-database names) | `FIELD_VISIBILITY_SERVER_ONLY` — it enumerates deck contents, so it is stripped from every client copy. Servatrice reads it through `RuledGameDriver::ruledCardIdForName` / `ruledCardNameForId` / `ruledFaceDisplayName` and **never derives an engine id from a name itself**. |
| `BattlefieldObjectMap` | **Servatrice**, every batch | `(player_id, oid)` → `Server_Card.id`, plus ordinal, `summoning_sick`, `keywords`, `is_creature` | Covers the battlefield **and** the stack zones. Client-side it fills `ownerCardIdToEngineOid` / `engineOidToCardId` / the keyword and creature maps. |
| `HandSlotMap` | **Servatrice**, on change | `(player_id, hand_index)` → `Server_Card.id` | Recipient-private: redaction keeps only the recipient's own rows. Fills `ownedCardToEngineHandSlot`. Sent only when the mapping differs from the last broadcast (plus whenever the participant set changes — a joiner/reconnector starts empty — and on `resetForNewGame`). **Absent means unchanged; present is a full replacement**, so `applyHandSlotMap` clears before filling. |
| `GraveyardObjectMap` | **Servatrice**, every batch | `(player_id, oid)` → `Server_Card.id` | Graveyard targeting (`ReturnFromGraveyard`). Fills `graveyardEngineOidToServerCardId`. |
| `RuledPlayerBinding` | **Servatrice**, per player, from each `ZoneViewSync` | `oid` ↔ `Server_Card.id` (battlefield + hand + stack), separately for the graveyard | The server-side source the three maps above are generated from. |

Public graveyard and exile moves record their post-move physical IDs immediately. A zone view
preserves those bindings and reorders the physical pile to the reverse of the engine's
oldest-first vector; it must not reassign known IDs by position. For example, a resolving
Lightning Bolt and its dying target can reach the relay's graveyard in a different order from
the engine because physical moves and stack resolution run in separate passes. A changed
public-pile order requires a full recipient-filtered game-state refresh.

**One trap worth naming.** For the concealed-zone resolution choices (`LIBRARY_SEARCH`,
`REVEALED`, `OPPONENT_HAND`) there is no real `Server_Card.id` to hand out, so the relay emits
**sequential indices 0, 1, 2 …** in `candidate_server_card_ids`. Those collide head-on with the
genuine ids of cards in hand and on the battlefield. Every id-keyed pick query must therefore be
gated on `RuledActions::isResolutionPickZoneCard` first.

### Naming convention

In new and touched code, name the variable after the id: `oid` (engine `ObjectId`), `cardId`
(tricerules slug), `serverCardId` (`Server_Card::getId()`), `handSlot` (engine hand index),
`faceIndex`, `playerId`/`seatId`. Rename stragglers when you are already editing the line;
do not do sweeping renames in an otherwise-unrelated change.

---

## 4. Ruled server facade and collaborators

`RuledGameDriver` is the fork-owned facade for one `Server_Game`. The upstream class keeps only a
`ruledGame` flag, the owning `unique_ptr`, `friend class RuledGameDriver`, a `ruled()` accessor,
and one-line delegators. The facade coordinates three single-owner collaborators:

- `RuledGameSession` owns the `RulesRelay`, deck admission, command canonicalization, seed,
  engine-loss state, and auto-pass policies;
- `RuledBatchSynchronizer` owns engine-object/physical-card bindings, catalog and stack state,
  accepted-command visuals, and authoritative physical batch projection;
- `RuledBroadcastRouter` owns server-built identity-map injection, fail-closed per-recipient
  redaction, the hand-map change cache, and reconnect resolution-choice state.

`RuledBatchSynchronizer::applyBatch` runs the following **authoritative, load-bearing sequence**.
Never merge or reorder these stages:

0. `indexCardCatalogEvents` — refresh the engine card-id/name index when the batch carries a
   catalog. A dev conjure can introduce a name absent from the session's original deck catalog,
   so the refresh must happen before any physical-card creation or reconciliation resolves it;
1. `applyDevCardConjures` — mint and bind physical cards created by dev commands. This precedes
   the identity snapshot because a conjured battlefield card may also move in the same batch, and
   `PermanentMoved` must be able to recover its newly registered physical identity;
2. capture the pre-batch `oid → Server_Card.id` map per player. The engine has already removed
   dead permanents, so the upcoming zone-view sync would otherwise lose the mapping
   `PermanentMoved` needs;
3. `applyTokenCreations` — mint physical token cards (CR 111) so the zone-view sync can bind them;
4. `applyPermanentMoves` — translate `PermanentMoved` into `moveCard`, using the pre-batch map;
5. `applyPhaseStackAndZoneViews` — apply phase/priority and stack push/resolve events, then
   reconcile each player's `ZoneViewSync`, rebuilding the identity maps;
6. `applyFaceDisplays` — apply face and effective-display names only after zone-view
   reconciliation has produced fresh object-to-physical-card mappings;
7. `applyAttachmentRestores` — `AuraAttached` → `Event_AttachCard`, using those refreshed maps;
8. `applyLifeManaAndCombatEvents` — apply life totals, mana-pool counters, combat declarations,
   combat damage, and removal from combat.

`RuledBroadcastRouter::broadcast` then stages: `appendServerObjectMaps` (inject the server-built
identity maps) → per-participant `redactBatchForParticipant` → private `Event_RuledPayload`.

---

## 5. Hidden information and the trust model

**Freeform mode is out of scope.** It is trust-based by upstream design; do not try to harden it.

| Boundary | Trust | What it checks |
|---|---|---|
| Client → Servatrice | **Untrusted.** | Authenticated session; the acting `playerId` is the participant's server-side member, never a client-supplied field. In ruled games the command allowlist (§2 step 3) rejects every freeform manipulation. Ruled legality itself is checked by the engine, not here. |
| Servatrice → sidecar | **Fully trusted.** | Nothing. The sidecar assumes its peer is Servatrice, so the socket is a full-trust boundary: `tricerules-server` binds `127.0.0.1:$TRICERULES_PORT` (default 17381) only, and the relay dials `$TRICERULES_HOST` (default `127.0.0.1`). Never expose that port to a network. |
| Servatrice → clients | Servatrice is authoritative. | Two-stage redaction, below. |

### Redaction is fail-closed, by classification

Every field reachable from `RuledEventBatch` carries a `(field_visibility)` option in
`ruled_v1.proto`: `PUBLIC`, `PER_PLAYER`, or `SERVER_ONLY`. A reflection test in the ruled batch
tests **fails the build when a broadcast-reachable field is unclassified**, so a new field cannot
leak by omission.

`redactBatchForParticipant` then, for each recipient:

1. keeps only that player's `legal_by_player` entry; drops `LogMessage`s routed elsewhere
   (`visible_to_player_id` / `hidden_from_player_id`); trims `HandSlotMap` to the recipient's rows;
   redacts private resolution candidates and injects the physical card ids the recipient is
   allowed to have;
2. captures those explicitly-authorized values, clears **every** `PER_PLAYER` field recursively by
   reflection, restores only the captured ones, then clears every `SERVER_ONLY` field the same way,
   and drops events left empty.

So the default for anything new is *removed*, and a value survives only because a reviewed line
puts it back.

**Private choice kinds** (`isPrivateChoiceKind` in `ruled_utils`): `CHOICE_KIND_HAND_CARDS`,
`CHOICE_KIND_LIBRARY_SEARCH`, `CHOICE_KIND_OPPONENT_HAND` expose a concealed zone, so their
candidates reach only the deciding player; `REVEALED`, `TARGET_OBJECTS` and `LEGEND_KEEP` are
public. Unknown values are treated as private.

**Known, accepted exposures:** replays contain both players' hidden information by design (the
full command log). Document that before any replay-sharing feature ships — it is on the roadmap's
security-audit backlog entry.

---

## 6. Effect ordering — where ordering is decided

Do not re-derive this per card.

- **Owner vs controller.** A permanent carries both at once and they can differ (reanimation).
  `GameObject::owner` is fixed for the object's life and decides which zone a card returns to
  (CR 400.3), zone membership everywhere off the battlefield, and hidden-info redaction.
  `GameObject::base_controller` is the CR 613 layer-2 base value; `GameObject::controller` is the
  materialized CR 110.2 current controller after continuous effects and decides untap and
  summoning sickness, attack/block legality, ability control and APNAP order, and anthem scoping.
  The per-player `battlefield` list is the **control index** — `oid ∈ players[i].battlefield` iff
  `objects[oid].controller == players[i].id` (asserted in `apply_sbas`). Continuous
  control-change effects are evaluated in layer 2, then the SBA boundary rebuilds that index,
  applies summoning sickness, and removes transitioned permanents from combat. Servatrice mirrors
  the same authoritative membership by moving the existing physical card between player TABLE zones.
- **Characteristics.** `GameEngine::characteristics(oid)` (`engine/characteristics.rs`) is the
  **single** entry point for derived names, controller, types, supertypes, colors, keywords, and P/T. It
  walks the CR 613 layers explicitly: 1 copy → 2 control → 3 text → 4 type → 5 color → 6
  ability adding/removing → 7 P/T (CDA, setters, modifiers, counters, switches). Layers 1–5 and
  the unused layer subparts are explicit stages. Layers 3–5 currently support name, type-line,
  and color replacement effects; implementing another effect in a layer means filling its slot,
  not adding another characteristics path. The calculation is pure
  (state + registry + oid), so it can be memoized without touching callers.
- **Timestamps.** Continuous effects carry CR 613.7 timestamps stamped from `command_index`;
  `ordered_effects` sorts by timestamp with the vector index as a deterministic tiebreak. That
  function is also the documented insertion point for CR 613.8 dependency ordering, which is
  deferred until the first Humility-class card (roadmap backlog).
- **State-based actions.** `engine/state_based.rs` runs the CR 704.4 fixed point: `apply_sbas`
  loops `apply_sbas_once` until nothing changes, with per-rule passes (704.5f/g/h/j/m/p, plus
  CR 122.3 counter annihilation). SBAs are **not** checked while a tier-3 resolution is parked;
  they run when it completes.
- **Continuous-effect lifecycle** (creation on ETB, expiry, LTB drain) lives in
  `engine/continuous.rs` — separate from evaluation on purpose.
- **Replacement effects.** `engine/replacement.rs` owns the shared proposed-event choice channel:
  applicable replacement/prevention applications use opaque ids, the CR 616 affected
  controller/owner chooses, and the engine applies one effect then re-evaluates. Damage prevention
  and battlefield entry use that channel today. Entry state is committed before CR 603 triggers,
  so "enters tapped" is never emitted as a later tap event. Regeneration remains a specialized
  destruction-replacement path until it first overlaps another applicable effect.
- **Resolution.** `engine/resolution/mod.rs` owns stack setup, the fizzle check, the custom
  (tier-3) handoff, and the **single exhaustive `SpellEffectKind` match**. Arms contain no logic:
  each delegates to a domain submodule (`damage`, `life`, `zones`, `pump_counters`, `mass`,
  `tokens`, `stack_ops`, `misc`). A new primitive is therefore still a compile error until an arm
  exists.
- **Mid-resolution choices** never block. A tier-3 effect returns a `ResolutionInterrupt`; the
  engine parks it in `GameState::pending_resolution`, emits `resolution_choice_required`, refuses
  every command but `SubmitResolutionChoice`/`Concede`, and resumes on the answer. Because the
  answer is a logged command, determinism holds.

---

## 7. Runtime performance posture

**Card-base size does not affect per-game runtime.** The registry is parsed once per process into
`HashMap`s (`CardRegistry::global()`); lookups are O(1), and a game touches ~120 cards regardless
of whether the registry holds 800 or 35,000.

What grows is **board complexity**. The hot paths, in the order they are likely to matter:

1. legal-action enumeration, once per priority window (`engine/legal_actions.rs`);
2. triggered-ability scans, once per internal `GameEvent` (`engine/triggers.rs`);
3. the SBA fixed-point loop, which re-queries characteristics per permanent per pass;
4. characteristics recomputation (currently unmemoized by design — see §6);
5. zone-view serialization + per-participant redaction, once per batch.

**Measure before optimizing.** The paired stress scenario in
`tricerules-core/tests/scenario/performance.rs` drives a large board through a full turn under
`--release` with a wall-time assertion, so growth shows up as a failing test. Determinism makes
any profile exactly reproducible. Two deferred wins have named triggers in the roadmap backlog
(binary card-data embed; two-letter sharding of the generated RON tree) — don't pre-build them.

---

## 8. Fork ownership

The merge manual. Upstream Cockatrice is still a live source of features, so the fork delta
inside upstream files must stay small enough to rebase.

| Tier | What | Rule |
|---|---|---|
| **Fork-owned** | `tricerules/`; `ruled_v1.proto`, `command_ruled_payload.proto`, `event_ruled_payload.proto`; `libcockatrice_network/.../game/ruled_game_driver.*`, `ruled_player_binding.*`, `ruled_utils.*`, `rules_relay.*`; `cockatrice/src/game/ruled/`; `cockatrice/src/game/prompt/game_prompt_widget.*`; `tests/ruled_batch_tests/`, `tests/ruled_client_tests/`, `tests/ruled_e2e_smoke/`, `tests/game_prompt/`, `tests/ruled_utils_test.cpp`; `docs/`; the ruled `scripts/`. | Restructure freely. |
| **Upstream with hooks** | `server_game.{h,cpp}`, `server_abstract_participant.cpp`, `server_abstract_player.h`, `server_player.cpp`; `game_event_handler.{h,cpp}`, `player_actions.{h,cpp}`, `card_item.cpp`, `tab_game.{h,cpp}`, the `zones/` and `player/` files. | **Extraction only.** The delta converges toward a member pointer, one friend declaration, and 1–3-line call-site guards. Never rename, reorder, or rewrite upstream code in place. |
| **Pristine upstream** | Everything else, including `webclient/` and `doc/carddatabase_v3` / `_v4`. | Leave alone — deleting them buys nothing and costs permanent conflicts. |

Conventions that make this greppable:

- Every fork-owned C++ file is prefixed `ruled_` (`rules_relay` predates the convention).
  `grep -rl ruled_` shows fork territory.
- New client fork files go in `cockatrice/src/game/ruled/`.
- `RuledActions::isRuledGame(game)` is the **only** place that reads the `ruled_game` proto flag.
  Never re-inline `game->getGameMetaInfo()->proto().ruled_game()`.
- Gate every new UI path on ruled mode; freeform must keep working.

Representative residual hooks, so you know what "small" looks like: `Server_Game` keeps the flag +
`unique_ptr` + friend + accessor + two delegators; `Server_AbstractPlayer` keeps one
`friend struct RuledPlayerBinding`; `GameEventHandler` keeps `ruled()`, the dispatcher pointer, and
the one-line `RULED_PAYLOAD` case, and implements `RuledClientHost`.

---

## 9. Extension recipes

Each is a checklist of the exact files. Build and test per AGENTS.md after every one.

### Add a data-tier card

1. Fetch Oracle data — `https://api.scryfall.com/cards/named?exact=<Card+Name>`. Never code from
   memory; if the fetch fails or the name is ambiguous, say so before writing anything.
2. Drop a `.ron` anywhere under `tricerules-cards/data/` (`build.rs` walks it; no `registry.rs`
   edit). Copy `mana_cost` verbatim from Scryfall; exact `power`/`toughness`/`type_line`.
   Add `partial: "<what's missing>"` if anything is unmodeled.
3. Scenario coverage in `tricerules-core/tests/scenario/<best-fit>.rs` — happy **and** illegal path.
4. `cargo test` + `clippy -- -D warnings` + `fmt --check`.
5. Regenerate `tricerules/CARDS.md` (`scripts/gen-card-checklist.ps1`, `--check` before commit) and
   commit it with the card.

No C++ build needed. Batch vanilla/french-vanilla creatures with `scripts/gen-cards.ps1` instead of
hand-authoring.

### Add a primitive (`SpellEffectKind`, `TriggerCondition`, `AbilityCost`, …)

1. **Name two real cards it covers**, or widen the parameters until you can. This gate is the
   whole scaling strategy.
2. Variant in the right `tricerules-cards/src/primitives/` submodule (`effects`, `targeting`,
   `costs`, `abilities`, `keywords`) — the re-exports keep `primitives::X` paths and RON serde
   names stable.
3. One arm in the exhaustive match in `engine/resolution/mod.rs` (a one-liner) plus the
   implementation in the matching domain submodule.
4. Registry validation in `tricerules-cards/src/registry.rs` if the primitive has authoring
   constraints; scenario tests; the RON card that motivated it.

### Add a keyword

`Keyword` variant in `primitives/keywords.rs` with its CR citation, the behaviour where it belongs
(`engine/combat.rs`, `state_based.rs`, `targeting.rs`, …), and scenario coverage. **No proto
change**: battlefield keywords cross the wire as `repeated string keywords` (serde variant names)
on `BattlefieldObject` / `BattlefieldObjectMap.Entry`, and the client reads them generically.

### Add an engine event (new mechanic visible to clients)

1. Message + `RuledEvent` oneof field in `ruled_v1.proto` — **with a `(field_visibility)` option on
   every field**, or the reflection test fails.
2. Emit it from the engine (`engine/events.rs` helpers).
3. Servatrice: translate it inside the correct `RuledBatchSynchronizer::applyBatch` pass (§4);
   add redaction handling only if it carries per-player data.
4. Client: **one new private method in `RuledEventDispatcher` plus one `has_*()` line** — never an
   inline block. State it needs lives on `RuledClientState`; anything it needs from the Qt UI goes
   on `RuledClientHost`, never as a direct `AbstractGame`/`Player`/`CardItem` include.
5. Tests: `ruled_batch_test` (server translation), `ruled_client_test` (client translation), and
   `ruled_e2e_smoke_test` if it is on a path that test drives.

A proto change means rebuilding **both** C++ and Rust and running the full suites.

### Add a UI prompt mode

`GamePromptWidget::PromptMode` value + a case in `effectiveMode()`, `updateCombatButtonsVisibility()`
and `applyPromptStateText()`; push it through the single `setRuledPromptState()` entry point from
`TabGame::refreshRuledPromptState()`. Coverage in `tests/game_prompt/`. Combat, priority and sticky
errors stay orthogonal inputs — they legitimately coexist with the modes.

### Add a tier-3 (custom Rust) card

Only when the *resolution algorithm itself* is unique — a mid-resolution choice over live objects,
or interdependent choices over one revealed set. A reviewer must agree no
`(effect_kind, parameters)` description exists; prefer widening a primitive every time it is close.

1. `custom_effect: "<card_id>"` in the RON (mutually exclusive with `spell_effect`). The value
   must equal the card definition's `id`.
2. Create `tricerules-core/src/custom/<card_id>.rs` (subdirectories are allowed; `support/` is
   skipped) and export `pub(crate) static EFFECT: &dyn CardEffect = &YourType;`. The file stem is
   the lookup key and must match both the RON `custom_effect` and card definition id; `build.rs`
   registers it automatically, so no shared Rust file is edited. Its generated private module
   name is prefixed, allowing registry-valid ids that begin with a digit or match a Rust keyword.
   Keep effects 1:1 with card ids; a shared algorithm belongs in a widened primitive instead.
3. Cite Oracle text + CR in the implementation's header comment. `begin`/`resume` drive the
   capability-narrowed `ResolutionCtx`; custom code never touches `&mut GameState`.
4. **No new proto.** Reuse `resolution_choice_required` / `SubmitResolutionChoice`; pick the right
   existing `ChoiceKind` (and check whether it is private — §5).
5. Scenario coverage in `tests/scenario/custom_resolution.rs`: happy + illegal.

---

## 10. Test stack

| Layer | Where | Runs |
|---|---|---|
| Rules | `tricerules-core/tests/scenario/` (+ `conformance.rs` resolving every registry card) | `cargo test` |
| Server translation | `tests/ruled_batch_tests/` | `ctest -R ruled_batch_test` |
| Server helpers | `tests/ruled_utils_test.cpp` | `ctest -R ruled_utils_test` |
| Client translation | `tests/ruled_client_tests/` (headless: dispatcher + state + a `FakeHost`) | `ctest -R ruled_client_test` |
| Presentation | `tests/game_prompt/` (offscreen widget) | `ctest -R game_prompt_widget_test` |
| Everything wired | `tests/ruled_e2e_smoke/` — real servatrice + sidecar processes, two protobuf-level clients, one seeded game (~1 s) | in the default `ctest` run |

There is deliberately **no full-GUI click automation**; real-GUI verification stays manual.
Run the E2E smoke after any change to the relay, `ruled_v1.proto`, or ruled `server_game` paths.
