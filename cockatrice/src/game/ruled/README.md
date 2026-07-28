# `cockatrice/src/game/ruled/` — the client's ruled-mode view model

Fork-owned. All ruled-mode client logic lives here; upstream files keep 1–3-line hooks.
For the system-wide picture (identity glossary, redaction, the life of a command) see
[docs/ARCHITECTURE.md](../../../../docs/ARCHITECTURE.md).

**The client is a mirror, not a rules engine.** Every legality question — can this be played, is
this a legal target, is this a creature — is answered from what the engine sent in the last
`RuledEventBatch`. Never re-derive one from the Oracle card database.

---

## The four units

(Plus `ruled_autopilot.{h,cpp}`, which is dev tooling rather than view model — see
[Dev-loop autopilot](#dev-loop-autopilot) at the end.)

```
             RuledEventBatch (Event_RuledPayload)
                        │
          GameEventHandler ── one-line RULED_PAYLOAD case
                        │
             RuledEventDispatcher ──writes──► RuledClientState ──signals──► TabGame /
                        │                            ▲                      GamePromptWidget /
                        └──────────┬─────────────────┘                      CardItem / PlayerTarget
                                   ▼
                            RuledClientHost  (implemented by GameEventHandler)
                                   ▲
                            RuledActions ── clicks, CardItem lookup, isRuledGame()
```

### `ruled_client_state.{h,cpp}` — `RuledClientState`

A `QObject` parented to `GameEventHandler`; the client's whole picture of the ruled game.
Reach it with `game->getGameEventHandler()->ruled()`.

Holds: legal hand actions per `HandActionKind`, engine targeting tables (by hand slot/face and by
ability), the identity maps (§ ARCHITECTURE identity glossary), combat staging and confirmed
combat state, stack order and annotations, the single pending player choice, and the opening
(choose-first / mulligan / bottom) state.

Members are public on purpose — this is a shared view model, and an accessor pair per field would
be noise. The `[[nodiscard]]` query methods are the read API consumers should prefer.

Two writer groups, and they must not be confused:

- **`RuledEventDispatcher` is the only writer of engine-authoritative fields.**
- The state's own `toggle*` / `clear*` / `submit*` methods mutate **local staging** in response to
  clicks, and are the only place that sends a command back (through the host).

### `ruled_event_dispatcher.{h,cpp}` — `RuledEventDispatcher`

`processPayload(bytes)` → parse → `resetPerBatchLegalActions()` → one private `apply*` method per
event kind → `finishBatch(ctx)` emits everything the batch accumulated, in the legacy order.

**A new engine event means a new method plus one `has_*()` line — never an inline block.**
`BatchContext` is the per-batch accumulator (timeline text, prompt-feed text, and the three
"dirty" flags) so a batch emits each signal once rather than per event.

### `ruled_client_host.h` — `RuledClientHost`

The pure-virtual seam the state and dispatcher use to reach the Qt UI: local seat id, turn/phase
writes, synthetic stack cards, the P/T fallback, command transport (with and without an ack), the
modal-choice fallback, and arrow resync. `GameEventHandler` implements it.

> **Keep `ruled_client_state.cpp` and `ruled_event_dispatcher.cpp` free of `AbstractGame`,
> `Player` and `CardItem`.** Anything new they need goes on this interface. That is the only
> reason `tests/ruled_client_tests/` can link and run headless with a `FakeHost`; adding a UI
> include there silently breaks the test target's link.
>
> (`ruled_client_state.h` does include `ruled_v1.pb.h`, for `PhaseId` / `HandActionKind`. Generated
> protobuf headers are fine — the rule is about the Qt game objects.)

### `ruled_actions.{h,cpp}` — `namespace RuledActions`

The bridge to the UI, and the one unit that *may* depend on `AbstractGame` / `Player` / `CardItem`,
because resolving a click is a UI concern. Free functions, so upstream call sites stay guards:

```cpp
if (RuledActions::tryHandleCombatClick(this)) return;
```

Also the home of:

- **`isRuledGame(game)`** (+ `isRuledGameForPlayer` / `isRuledGameForCard`) — the **only** place
  that reads the `ruled_game` proto flag. Never re-inline
  `game->getGameMetaInfo()->proto().ruled_game()`.
- `resolveHandActionIndex(state, kind, card)` — the single click→engine-hand-slot entry point.
- `findBattlefieldCardItemByEngineOid` / `findStackCardItemBy…` / `resolveSpellTargetItem`.
- `isResolutionPickZoneCard` — **gate every id-keyed resolution-pick query on this.** Candidates in
  a concealed zone carry synthetic sequential ids that collide with real `Server_Card` ids.

---

## Signal map

Every signal is declared on `RuledClientState` and emitted by it or by the dispatcher.

| Signal | Consumer | Drives |
|---|---|---|
| `sessionReset()` | `TabGame` | Reset UI derived from the finished session. |
| `engineTimeline(QString)` | `MessageLogWidget` | The authoritative game log line. |
| `enginePromptFeed(QString)` | `GamePromptWidget` | The prompt panel's text (engine phase/priority plus local hints). |
| `combatStateChanged()` | `TabGame`, `GamePromptWidget` | Combat buttons, arrows, and — via `notifyHandUiChanged()` — hand-selection UI. |
| `blockerRejected()` | `GamePromptWidget` | Sticky error label; emitted *before* `combatStateChanged` so the refresh does not overwrite it. |
| `combatDamageUiChanged()` | `TabGame` | Assign-combat-damage prompt (attacker, assigned/power, legality). |
| `spellTargetSelectionChanged()` | `CardItem`, `PlayerTarget` | Repaint target highlights. |
| `spellDamageAllocationUiChanged()` | `CardItem`, `PlayerTarget`, `TabGame` | Repaint per-target damage allocation. |
| `battlefieldMapUpdated()` | `CardItem`, `TabGame` | Repaint after the identity maps change. |
| `stackHasItemsChanged(bool)` | `TabGame`, `GamePromptWidget` | Stack window visibility; pass-priority button text. |
| `stackOrderChanged(QList<quint32>)` | `TabGame` | Re-sort the stack window (LIFO; `Event_MoveCard` may arrive before `stack_pushed`). |
| `triggerGraveyardNeedsTarget(bool)` | `TabGame` | Open/close the graveyard view for a trigger that targets there. |
| `firstStrikeStepPendingChanged(bool)` | `GamePromptWidget` | "First Strike Damage" vs "Combat Damage" button label. |
| `firstStrikeDamageStepActiveChanged(bool)` | `GamePromptWidget` | Same, once inside the substep (CR 510.4). |
| `undoableManaAbilitiesChanged(int)` | `TabGame` | The undo-land-tap affordance (CR 605 float courtesy). |
| `cleanupDiscardUiChanged(int,int)` | `TabGame` | CR 514.1 discard prompt (required / selected). |
| `openingUiChanged()` | `TabGame` | Choose-first / mulligan prompt mode. |
| `openingBottomUiChanged(int,int)` | `TabGame` | London-mulligan bottoming prompt. |
| `resolutionHandPickUiChanged(int,int)` | `TabGame` | Tier-3 pick prompt; `required == -1` means cleared. |
| `librarySearchPickStarted(QStringList,QVector<int>)` | `TabGame` | Auto-open the deck zone view with the candidates. |
| `revealedPickChanged(bool,QStringList,QVector<int>,int,int)` | `TabGame` | Open/close the revealed-cards popup. |
| `triggerNeedsTarget(QString)` | *(none today)* | Emitted on `TriggerNeedsTarget`; the prompt text currently reaches the panel through `enginePromptFeed` instead. Wire it, or drop it, when the trigger-target UI next changes. |

Incoming direction — `GamePromptWidget` signals connect straight to `RuledClientState` slots
(`confirmAttackers`, `skipAttackers`, `confirmBlockers`, `skipBlockers`,
`confirmCombatDamageForCurrentAttacker`, `openingPickFirstSeat`, `openingMulliganKeep`,
`openingMulliganRedraw`, `openingBottomCancel`, `openingBottomDone`, `submitResolutionHandPick`).
The `connect` lines live in `tab_game.cpp` and are the accepted residual fork delta there.

---

## Invariants

1. **One pending choice.** The engine parks at most one decision and blocks, so
   `RuledClientState::pendingChoice` is a single `std::optional`. `setPendingChoice()` tears down
   whatever it replaces (including the revealed-cards popup); `clearPendingChoiceOfKind()` is how a
   follow-up engine event retires the one choice it answers; `sendResolutionChoice()` is the only
   `SubmitResolutionChoice` sender. `TriggerTarget` is answered with `ChooseTriggerTarget` instead.
2. **Trigger stack bookkeeping is not a choice.** `lastTriggerSourceOid` /
   `lastTriggerAbilityIndex` / `lastTriggerControllerPlayerId` are recorded on **every** client,
   because the synthetic stack card and its source arrow are built from them on seats that never
   get to choose. Only the controller parks a choice.
3. **Session teardown is asymmetric.** `RuledSessionResetScope::All` on game stop;
   `KeepCurrentBatch` on game start, because the new session's first batch is broadcast *before*
   the `Event_GameStateChanged` that flips `game_started` — clearing it would strand the opening.
   See the enum's comment before changing either path.
4. **Legal actions are per-batch.** `resetPerBatchLegalActions()` wipes them before every payload,
   so nothing can leak across games. `applyNoLegalActions()` deliberately does *not* clear the
   must-attack / must-block requirement sets (a Servatrice-synthesized preview echo carries no
   `legal_by_player` entry for us).
5. **Labels are display-only.** Gameplay reads `LegalActions.hand_actions` (structured
   `LegalHandAction`); `LegalActions.labels` feeds the prompt text and nothing else.

---

## Testing

`tests/ruled_client_tests/ruled_client_test.cpp` (ctest `ruled_client_test`) links only
`ruled_client_state.cpp` + `ruled_event_dispatcher.cpp` + `libcockatrice_protocol`, feeds synthetic
`ruled::v1` batches through the dispatcher, and asserts on the state — no rendering, no
`AbstractGame`. Add a case there for every dispatcher method you add.

`ruled_dev_command_parser.cpp` links into the same target for the same reason — it is pure text →
protobuf with no widget or game-object dependency.

Widget behaviour is covered offscreen by `tests/game_prompt/` (`game_prompt_widget_test`) and
`tests/dev_console/` (`ruled_dev_console_test`); use `isHidden()`, not `isVisible()`, since the
widget is never shown. Qt module additions for tests go in `cmake/FindQtRuntime.cmake`
(`_TEST_NEEDED`), not per-test `CMakeLists`.

---

## Dev-loop autopilot

`ruled_autopilot.{h,cpp}` — `RuledAutopilot`. Not part of the view model: it is the client half of
`scripts/launch-ruled-game.ps1`, which brings up sidecar + servatrice + two clients already sitting
in a started ruled game. Manual verification is the only check the ruled UI has (there is
deliberately no GUI click automation — see the roadmap's "Do NOT do"), so the pre-game ceremony
being ten clicks every time was the thing making manual runs rare.

Off unless `--autopilot host|join` is passed; a normally-launched client never constructs one.

It drives the pre-game sequence only, and stops at game start — everything from the opening hand on
is the real UI under a human. It sends the same commands the buttons send (`Command_CreateGame`,
`Command_JoinGame`, then `DeckViewContainer::loadDeckFromFile` / `readyAndUpdate`), so there is no
second code path to keep correct, and it never touches engine state.

Two things worth knowing before changing it:

- **Room and game discovery is polled, not signal-driven.** A room's initial game list arrives in
  the `Response_JoinRoom` *response*, and only later changes come as `Event_ListGames`. An
  event-only trigger therefore misses a game created before this client logged in. The joining seat
  polls `Command_GetGamesOfUser` for the host instead, which is race-free either way.
- **Ready is sent exactly once** (`readySent`). A failed ruled deck validation un-readies both
  players; re-readying automatically would loop the game start against an unimplemented card.

Upstream cost: one option pair in `main.cpp` plus one `installFromCommandLine` call, and a
`friend class RuledAutopilot` on `TabSupervisor` (find the game tab) and `TabGame` (find this
seat's deck view).

---

## Dev-loop console

`ruled_dev_command_parser.{h,cpp}` + `ruled_dev_console.{h,cpp}`. The other half of the dev loop:
the autopilot gets you into a game, the console gets you to the board state you wanted to test —
without editing a deck file, playing lands, or passing turns.

Off unless `--dev-console` is passed, and only built for a ruled non-replay game. **That gate is
cosmetic.** The enforcing one is engine-side (`tricerules-core/src/engine/dev.rs`), because a
client is never trusted; see the roadmap's dev-loop backlog entry for both halves.

Split in two on purpose:

- **`RuledDevCommandParser`** is pure `QString` → `ruled::v1::RuledCommand`, with no widget and no
  game-object dependency, so it links into the headless `ruled_client_test` target. This is where
  the text stops being text — everything past it is a typed oneof, which is what keeps the wire
  contract self-documenting and the replay log readable (the fork's no-scripting-DSL rule).
- **`RuledDevConsoleWidget`** follows `GamePromptWidget`'s discipline: it emits
  `commandSubmitted(QString)` and sends nothing. It lives in the existing Messages dock under the
  prompt panel, not in a dock of its own. Command history is the one thing it owns that the chat
  entry could not have given us.

`TabGame::actDevConsoleCommand` parses and sends, via `RuledActions::sendRuledCommand` rather than
calling `GameEventHandler` directly — that class keeps its `RuledClientHost` overrides private so
the view model is normally the only thing that sends, and routing through `RuledActions` keeps this
transport with the others instead of widening an upstream class.

Two grammar rules worth knowing before extending it:

- **`ready` is stripped only as a trailing token, and only when something precedes it**, so a card
  actually named "ready" still parses. None is today; the grammar should not depend on the pool.
- **A leading number is ambiguous for `mana`** — `mana 12` is twelve generic, `mana 2 UU` is the
  second seat. It is read as a seat only when it is a valid ordinal *with* symbols after it, so
  `mana 3 RR` falls back to mana rather than erroring on a seat that cannot exist. `put` has no
  such ambiguity (no zone word is numeric). Seats are 1-based ordinals, never raw player ids.

Adding a primitive is a proto arm, a `parse` case, and an engine handler — no new UI.
