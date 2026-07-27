# `cockatrice/src/game/ruled/` — the client's ruled-mode view model

Fork-owned. All ruled-mode client logic lives here; upstream files keep 1–3-line hooks.
For the system-wide picture (identity glossary, redaction, the life of a command) see
[docs/ARCHITECTURE.md](../../../../docs/ARCHITECTURE.md).

**The client is a mirror, not a rules engine.** Every legality question — can this be played, is
this a legal target, is this a creature — is answered from what the engine sent in the last
`RuledEventBatch`. Never re-derive one from the Oracle card database.

---

## The four units

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

Widget behaviour is covered offscreen by `tests/game_prompt/` (`game_prompt_widget_test`); use
`isHidden()`, not `isVisible()`, since the widget is never shown. Qt module additions for tests go
in `cmake/FindQtRuntime.cmake` (`_TEST_NEEDED`), not per-test `CMakeLists`.
