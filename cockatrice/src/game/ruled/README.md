# `cockatrice/src/game/ruled/` — the client's ruled-mode view model

Fork-owned. All ruled-mode client logic lives here; upstream files keep 1–3-line hooks.
The product offers ruled games only. Legacy upstream implementation remains behind mode guards
for periodic merges; it is not a supported freeform or local-game entry point.
For the system-wide picture (identity glossary, redaction, the life of a command) see
[docs/ARCHITECTURE.md](../../../../docs/ARCHITECTURE.md).

`ruled_client_diagnostics`, `ruled_bug_report`, and `ruled_diagnostic_viewer` own per-game capture,
Help-menu report export, and recipient-only offline playback. `ruled_resume_client` restores local
toolbar preferences for a captured session. See [Ruled diagnostics](../../../../docs/RULED-DIAGNOSTICS.md)
for file schemas, maintainer tools, and the separate live-resume launcher.

**The client is a mirror, not a rules engine.** Every legality question — can this be played, is
this a legal target, is this a creature — is answered from what the engine sent in the last
`RuledEventBatch`. Never re-derive one from the Oracle card database.

---

## Core units

(Plus `ruled_autopilot.{h,cpp}`, which is dev tooling rather than view model — see
[Dev-loop autopilot](#dev-loop-autopilot) at the end.)

```
             RuledEventBatch (Event_RuledPayload)
                        │
          GameEventHandler ── RuledGameHooks ── RuledGameController
                        │
             RuledEventDispatcher ──writes──► RuledClientState ──signals──► TabGame /
                        │                            ▲                      GamePromptWidget /
                        └──────────┬─────────────────┘                      CardItem / PlayerTarget
                                   ▼
                            RuledClientHost  (implemented by RuledGameController)
                                   ▲
                            RuledActions ── clicks, CardItem lookup, isRuledGame()
```

### Upstream model and graphics adapters

`ruled_auxiliary_area` lays out independent sections along the right end of each player's land row.
`ruled_public_card_zone` displays actual command-zone CardItems there. The section sizes to its
cards and the land row reserves that width. Empty sections collapse. Emblem and effect displays
can use the same layout and keep their own engine locations and interactions.
Command-zone clicks use engine-offered casts and explicit public physical bindings; payment,
cancellation, stack movement and reconnect use the existing casting transaction.

See [format admission and acceptance](../../../../docs/FORMATS.md) for format data updates,
validation and the pending end-user checks.

Both deck editors use the shared deck model and its Commander board. Native deck files
continue to encode those cards as commander declarations for Servatrice.

Upstream now keeps game state under `libcockatrice_game/libcockatrice/game/` and graphics
under `cockatrice/src/game_graphics/`. `RuledGameController` implements the model-facing
`RuledGameHooks` and the headless state-facing `RuledClientHost`; the tab owns its lifetime.
It routes transport, payloads, snapshot transitions, mana-counter projections and synthetic graphics.
`RuledPlayerActions` subclasses the primary upstream `PlayerActions` object and keeps payment
and targeting entry points on that one QObject. `ruled_client_ui` maps `PlayerLogic` / `CardState`
to their existing graphics objects, without creating duplicate model or action objects.
`RuledActions::isRuledGame` is defined in the model-only `ruled_game_mode.cpp`, so core guards
do not pull widgets into the game library.

`ruled_game_mode_ui` owns fixed ruled-creation controls and local-game action suppression,
including upstream attempts to re-enable that action. The main-window slots and saved startup
setting remain behind its product-policy guard. Servatrice rejects absent/false `ruled_game`
before allocating a game; the dialog and autopilot always send true. Historical replay viewing
and shared upstream presentation implementations remain available.

The generic upstream `CardMenu` fallback is suppressed for ruled cards at
`PlayerMenu::updateCardMenu`, covering both right-click and the top-level Card menu.
Engine-authored right-click actions run before that fallback. Hover and middle-click inspection
keep their existing recipient-permission checks. Batch playback exports `card_context_menus`
probes after seeking; these record only whether a legacy menu exists, never its action labels.

`ruled_zone_menus` restricts the shared player/zone menu objects to read-only graveyard, exile,
sideboard and command-zone views. Battlefield, empty-hand/hand-counter and library popup
bindings are cleared, including the library's legacy double-click draw. Player-focus refreshes
activate only inspection shortcuts from these menus; the separate counter/mana shortcut signal
path is preserved. Upstream menu implementations and translation ownership remain intact.
Batch `zone_context_menus` probes exercise actual popup slots and check inspection-only actions
and the absence of legacy zone shortcuts after focus changes.

`ruled_game_menu` hides manual turn-order and phase controls, their shortcuts, and the empty
Selected cards submenu. Persistent action guards survive role, translation and shortcut refreshes.
The existing Next turn action becomes Pass priority through `ruled_game_mode_ui`: its availability
and activation follow the prompt button, preserving engine priority and blocking-choice handling.
Display rotation, arrows, chat, game information and session actions remain available.
Batch `live_game_menu` probes construct an offline live tab and exercise lifecycle refreshes;
historical replay tabs retain their existing smaller menu.

### `ruled_client_state.{h,cpp}` — `RuledClientState`

A `QObject` parented to `RuledGameController`; the client's whole picture of the ruled game.
Reach it with `game->getGameEventHandler()->ruled()`.

Holds: legal hand actions per `HandActionKind`, engine targeting tables (by hand slot/face and by
ability), the identity maps (§ ARCHITECTURE identity glossary), combat staging and confirmed
combat state, stack order and annotations, the single pending player choice, the caster-private
engine `PendingSpellCast`, and the opening (choose-first / mulligan / bottom / hand actions) state.
`ruled_opening_hand.cpp` stages Gemstone Caverns and its distinct exile choice from exact engine
offers. Physical clicks resolve through the hand-slot map; Begin/Finish submit those references,
and every new legal snapshot clears the local selection. The prompt owns no card legality.

Members are public on purpose — this is a shared view model, and an accessor pair per field would
be noise. The `[[nodiscard]]` query methods are the read API consumers should prefer.

### `ruled_card_popup.{h,cpp}` — face-down exile inspection

An authoritative ruled move into face-down exile clears the common card identity with an explicit
empty name. Hover and popup inspection use the recipient's private `FaceDownObjectMap` permission;
without a current grant the popup has no card name or provider. The popup adapter remembers the
identity it actually displayed so the matching dismissal still works if that permission changes.
Freeform cards and cards outside face-down exile retain their existing inspection behavior.

### `ruled_pending_cast.{h,cpp}` — `RuledPendingCast`

Owns the mutually exclusive local spell-cast / activated-ability transaction. A spell progresses
through `Announcing`, `BeginPending`, engine-authoritative `Paying`, `CommitPending`, or
`CancelPending`. Modes, targets, X, and additional-cost objects are staged before
`BeginSpellCast`; only the returned transaction id and locked cost may enter payment. A reconnect
rebuilds `Paying` from the caster-private `PendingSpellCast` and the exact reserved engine object.
The relay keeps a hand source physically in hand until commit; other cast sources can be parked on
the physical stack during payment.
The client never redetermines that cost. Hand selections are retained as stable `Server_Card.id`
values and converted back to the latest engine hand slot only when the announcement is submitted.
Starting one local transaction cancels the other. Freeform never enters this controller.

Cost reconciliation and prompt calculations live in `ruled_pending_costs.cpp`, as methods of
`RuledPendingCast`, and run in the headless client tests. Menu-only test targets can link the
original menu implementation without linking the client-state query implementation.
Reconciliation returns whether staging can survive the latest legal snapshot; it does not emit
UI signals or cancel the transaction itself. The payment bridge performs those effects in the
existing order after target reconciliation. Mana text is engine-authored presentation input, never
an independent source of casting legality.

`ruled_pending_targets.cpp` implements the same transaction owner's headless target-group and
selected-mode progression, damage allocation, and selection-display queries. State transitions
return their result to the UI bridge; they never send commands or emit UI notifications.

Two writer groups, and they must not be confused:

- **`RuledEventDispatcher` is the only writer of engine-authoritative fields.**
- The state's own `toggle*` / `clear*` / `submit*` methods mutate **local staging** in response to
  clicks, and are the only place that sends a command back (through the host).

### `ruled_payment.{h,cpp}` and `ruled_payment_ui.{h,cpp}`

`RuledPayment` holds exact mana and object-contribution staging, correlates private preview
replies, and allows one submission after an authoritative complete preview. Spell previews carry
`CommitSpellCast { transaction_id }`, so Rust validates the proposed payment against the locked
engine transaction without accepting a client-supplied total. `RuledPaymentUi` adapts that model
to spell casts, activated abilities, permanent actions, and parked resolution payments. It owns
object highlights, mana contribution clicks, engine cancellation, and nested suspension around
mana abilities. Previews never tap cards or spend mana. Historical Waterbend acceptance
[is recorded on issue #146](https://github.com/pizzaninja188/Cockatrice/issues/146#issuecomment-5458728218).

`ruled_payment_progression.cpp` implements the same `RuledPaymentUi` bridge's cast/activation
entry and face/cast-method selection, initialization, cost-group progression, cost-object clicks,
X/flexible-mana dialogs, cancellation, submission, and resolution-payment integration.
`RuledPlayerActions` retains thin entry points and
host signals; it does not own a second progression controller. The two cost-click hooks remain
on either side of the existing trigger/permanent-choice handlers to preserve their precedence.
`RuledTargetUi` owns target reconciliation, eligibility, card/player click dispatch, target-group
and mode prompt progression, and damage-allocation UI notifications. It hands completed staging
directly to `RuledPaymentUi`; `TabGame` selects prompt modes. The bridges keep `RuledPlayerActions`
as the QObject connection context and preserve existing translation contexts, acknowledgement
behavior, and nested-payment lifetimes.

### `ruled_replacement_picker.{h,cpp}`

Owns the public replacement/prevention image window. `ReplacementEffectOption` supplies each
opaque application ID, source display snapshot, and summary; `RuledClientState` holds the one
pending choice and produces the image rows. A click submits one application, then the engine
recalculates applicability. The picker refreshes in place and remains non-closeable until the
choice ends. Its zone marker and choice revision distinguish synthetic tile indices from every
physical card and from stale snapshots. Rejections unlock the current choice; late acknowledgements
cannot restore an obsolete one. Private discard replacement options keep their existing buttons.

### `ruled_event_dispatcher.{h,cpp}` — `RuledEventDispatcher`

`processPayload(bytes)` → parse → `resetPerBatchLegalActions()` → one private `apply*` method per
event kind → `finishBatch(ctx)` emits everything the batch accumulated, in the legacy order.

**A new engine event means a new method plus one `has_*()` line — never an inline block.**
`BatchContext` is the per-batch accumulator (timeline text, prompt-feed text, and the three
"dirty" flags) so a batch emits each signal once rather than per event.

### `ruled_client_host.h` — `RuledClientHost`

The pure-virtual seam the state and dispatcher use to reach the Qt UI: local seat id, turn/phase
writes, synthetic stack cards, the P/T fallback, command transport (with and without an ack), the
modal-choice fallback, and arrow resync. `RuledGameController` implements it.

> **Keep `ruled_client_state.cpp` and `ruled_event_dispatcher.cpp` free of `AbstractGame`,
> `PlayerLogic` and `CardItem`.** Anything new they need goes on this interface. That is the only
> reason `tests/ruled_client_tests/` can link and run headless with a `FakeHost`; adding a UI
> include there silently breaks the test target's link.
>
> (`ruled_client_state.h` does include `ruled_v1.pb.h`, for `PhaseId` / `HandActionKind`. Generated
> protobuf headers are fine — the rule is about the Qt game objects.)

### `ruled_actions.{h,cpp}` — `namespace RuledActions`

The click bridge to the UI; together with the adapters above it may depend on `AbstractGame` / `PlayerLogic` / `CardItem`,
because resolving a click is a UI concern. Free functions, so upstream call sites stay guards:

```cpp
if (RuledActions::tryHandleCombatClick(this)) return;
```

Also the home of:

- **`isRuledGame(game)`** (+ `isRuledGameForPlayer` / `isRuledGameForCard`) — the **only** place
  that reads the `ruled_game` proto flag. Never re-inline
  `game->getGameMetaInfo()->proto().ruled_game()`.
- `resolveHandActionIndex(state, kind, card)` — the single click→engine-hand-slot entry point.
- `findBattlefieldCardItemByEngineOid` / `findStackCardItemBy…` / `classifySpellTargetItem` +
  `resolveSpellTargetItem`. **A targeting arrow's endpoint kind is classified once and latched**
  (`RuledClientState::latchTargetKind`, write-once), because CR 608.2b makes a target that changed
  zones a *different* object. Resolve only within the latched kind — re-classifying each sync lets
  a dead permanent's oid re-resolve through the graveyard map, and the arrow follows the card to
  the pile instead of disappearing.
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
| `legalActionsChanged()` | `RuledTargetUi` | Reconcile staged spell/ability targets against the latest settled authoritative target sets. |
| `blockerRejected()` | `GamePromptWidget` | Sticky error label; emitted *before* `combatStateChanged` so the refresh does not overwrite it. |
| `combatDamageUiChanged()` | `TabGame` | Assign-combat-damage prompt (attacker, assigned/power, legality). |
| `spellTargetSelectionChanged()` | `CardItem`, `PlayerTarget` | Repaint target highlights. |
| `spellDamageAllocationUiChanged()` | `CardItem`, `PlayerTarget`, `TabGame` | Repaint per-target damage allocation. |
| `battlefieldMapUpdated()` | `CardItem`, `TabGame` | Repaint after the identity maps change. |
| `stackHasItemsChanged(bool)` | `TabGame`, `GamePromptWidget` | Stack window visibility; pass-priority button text. |
| `stackOrderChanged(QList<quint32>)` | `TabGame` | Re-sort the stack window (LIFO; `Event_MoveCard` may arrive before `stack_pushed`). |
| `graveyardTargetsNeeded(QList<int>)` | `TabGame` | Open/close graveyard views. Four sources union in `emitGraveyardTargetsNeeded()`: pending cast and trigger targets, object candidates in the active local copy-retarget offer, and latched graveyard targets of anything still on the stack (so a view stays open until that stack object resolves). Copy-target player candidates stay in their separate ID domain. Names players, not a bool: a spell may target any graveyard (Reanimate). |
| `firstStrikeStepPendingChanged(bool)` | `GamePromptWidget` | "First Strike Damage" vs "Combat Damage" button label. |
| `firstStrikeDamageStepActiveChanged(bool)` | `GamePromptWidget` | Same, once inside the substep (CR 510.4). |
| `undoableManaAbilitiesChanged(int)` | `TabGame` | The undo-land-tap affordance (CR 605 float courtesy). |
| `cleanupDiscardUiChanged(int,int)` | `TabGame` | CR 514.1 discard prompt (required / selected). |
| `openingUiChanged()` | `TabGame` | Choose-first / mulligan prompt mode. |
| `openingBottomUiChanged(int,int)` | `TabGame` | London-mulligan bottoming prompt. |
| `resolutionHandPickUiChanged(int,int)` | `TabGame` | Tier-3 pick prompt; `required == -1` means cleared. |
| `librarySearchPickStarted(QStringList,QVector<int>)` | `TabGame` | Auto-open the deck zone view with the candidates. |
| `revealedPickChanged(bool,QStringList,QVector<int>,int,int)` | `TabGame` | Open/close a chooser-private revealed-cards popup. |
| `RuledRevealState::changed()` | `RuledRevealWindows` | Reconcile all public reveal windows; active choices share the same surface and completed snapshots remain locally dismissible. |
| `triggerNeedsTarget(QString)` | *(none today)* | Emitted on `TriggerNeedsTarget`; the prompt text currently reaches the panel through `enginePromptFeed` instead. Wire it, or drop it, when the trigger-target UI next changes. |

Incoming direction — `GamePromptWidget` signals connect straight to `RuledClientState` slots
(`confirmAttackers`, `skipAttackers`, `confirmBlockers`, `skipBlockers`,
`confirmCombatDamageForCurrentAttacker`, `openingPickFirstSeat`, `openingMulliganKeep`,
`openingMulliganRedraw`, `openingBottomCancel`, `openingBottomDone`, `submitResolutionHandPick`).
The `connect` lines live in `tab_game.cpp` and are the accepted residual fork delta there.

---

## Invariants

1. **One pending choice, separate public reveal.** The engine parks at most one decision and blocks, so
   `RuledClientState::pendingChoice` is a single `std::optional`. A choice is answerable only when
   its nonzero `resolution_prompt_id` matches the final active prompt snapshot for the local decider;
   submissions carry that captured ID so a delayed dialog cannot answer a replacement prompt.
   `setPendingChoice()` tears down whatever it replaces (including the revealed-cards popup), and
   `clearPendingChoiceOfKind()` retires the choice answered by a follow-up engine event. Ordinary
   object-choice submissions use `sendResolutionChoice()`; dedicated payment, replacement, staged
   cost, fallback-dialog, and special-cast paths construct their own `SubmitResolutionChoice`.
   Every route must submit the exact nonzero prompt ID accepted by the current active snapshot;
   modal and cast paths revalidate the captured ID after their interaction. `TriggerTarget` is
   answered with `ChooseTriggerTarget` instead.
   A public reveal is mirrored separately on every participant, including observers that do not
   own the choice; authoritative batches replace or clear that exact snapshot, while local combat
   previews leave it untouched.
2. **Trigger choice bookkeeping is not stack identity.** `lastTriggerSourceOid` and
   `lastTriggerAbilityIndex` describe the current trigger choice. Each ability's `StackPushed`
   publishes its controller and optional exact battlefield source. The client tracks the source
   occurrence, clears stale generations, and never borrows missing metadata from a previous trigger.
   The current shared-stack host does not render a separate controller label or source arrow;
   targeting arrows begin at the synthetic stack card. Copies take the copy path even with no card id.
   A reconnect stack snapshot replaces tracked synthetic presentation, replays ordered synthetic
   ability and copy entries, and restores a pending resolution choice afterward. Existing physical
   bindings are preserved; a fresh session's physical spell stack bootstrap remains a limitation.
3. **Session teardown is asymmetric.** `RuledSessionResetScope::All` on game stop;
   `KeepCurrentBatch` on game start, because the new session's first batch is broadcast *before*
   the `Event_GameStateChanged` that flips `game_started` — clearing it would strand the opening.
   See the enum's comment before changing either path.
4. **Legal actions are per-batch.** `resetPerBatchLegalActions()` wipes them before every payload,
   so nothing can leak across games. `applyNoLegalActions()` deliberately does *not* clear the
   must-attack / must-block requirement sets or the selectable attacker / blocker sets (a
   Servatrice-synthesized preview echo carries no `legal_by_player` entry for us). Combat clicks
   are gated by those selectable sets, keeping continuous restrictions engine-authoritative.
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
in a started ruled game. GUI acceptance remains separate from automated tests; the launcher
removes repeated pre-game setup before checking the real interaction paths.

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
client is never trusted; both gates must be enabled before dev commands are accepted.

The command path stays fork-owned:

- **`RuledDevCommandParser`** is pure `QString` → `ruled::v1::RuledCommand`, with no widget and no
  game-object dependency, so it links into the headless `ruled_client_test` target. This is where
  the text stops being text — everything past it is a typed oneof, which is what keeps the wire
  contract self-documenting and the replay log readable (the fork's no-scripting-DSL rule).
- **`RuledDevConsoleWidget`** emits submissions and sends nothing. It owns command history,
  file preview and busy controls in the existing Messages dock under the prompt panel.
- **`RuledDevConsoleRunner`** parses through the existing parser, prevalidates every preset line
  and resolves seat ordinals before sending anything, then waits for each engine acknowledgment.
- **`ruled_dev_console_game`** attaches the runner to `RuledActions::sendRuledCommandExpectingAck`
  and cancels on session reset, game closure or connection changes. `TabGame` keeps one install hook.

**Load preset…** opens a UTF-8 text file (at most 64 KiB). Its filename names the preset; the preview
shows the loaded contents, and **Run (adds state)** is a separate action. Use one existing command
per line; blank lines and full-line `#` comments are ignored. Windows CRLF and a UTF-8 BOM are
accepted. There are no variables, includes, loops, delays or new engine commands. A preset has at
most 128 commands; `help` is interactive-only. A syntax error or missing seat sends nothing.

These are **prepared test states**, not proof that legal gameplay can reach the state. A successful
parse is not engine acceptance. Runtime rejection stops the remaining commands; accepted commands
remain, with no rollback. **Cancel remaining** stops the queue and waits for any command already
sent before unlocking. A lost response keeps that command pending until a response or lifecycle
reset; cancellation cannot undo it. Disconnect/session replacement invalidates late callbacks and
never automatically resumes. Input and Run stay locked while busy. After completion, rejection or
cancellation, a fresh Run starts from the beginning and can add duplicate cards and mana; use a
fresh game for a clean rerun. The original engine dev gate and capture/logging transport apply to
every command. See [capture inspection](../../../../docs/RULED-DIAGNOSTICS.md).

Checked-in [setup presets and manual checks](../../../../docs/dev-presets/README.md) cover Aura
activation restrictions and four-seat Decimate targeting. Load their `.txt` files through the same
file picker; copy/edit a file to keep another named setup without changing the client.

Three grammar rules worth knowing before extending it:

- **`put` and `move` are separate verbs on purpose.** `put` always conjures a new card; `move`
  relocates one the seat already owns and is the way to reach graveyard, exile, library or command.
  They were one verb that guessed (move if a copy exists, else conjure), which made "give me a
  second Serra Angel" impossible to express and silently pulled the existing one off the
  battlefield instead. If you add a third placement verb, keep the intent explicit.
- **`ready` is stripped only as a trailing token, and only when something precedes it**, so a card
  actually named "ready" still parses. None is today; the grammar should not depend on the pool.
- **A leading number is ambiguous for `mana`** — `mana 12` is twelve generic, `mana 2 UU` is the
  second seat. It is read as a seat only when it is a valid ordinal *with* symbols after it, so
  `mana 3 RR` falls back to mana rather than erroring on a seat that cannot exist. `put` has no
  such ambiguity (no zone word is numeric). Seats are 1-based ordinals, never raw player ids.
- `cmd` and `command` both name `DEV_ZONE_COMMAND`, so stage an existing object with
  `move cmd Card Name`; there is no Stack destination for dev movement.
- **`choice public-zones`** opens a parameterless, read-only `ZoneSearch` mapping fixture. At the
  current priority window it snapshots one distinct, face-up live card from each of Battlefield,
  Graveyard, Exile, Stack and Command. The Stack candidate must be a real physical spell cast; the
  engine derives every ObjectId, generation, name and source zone, and the authenticated command
  actor answers. It neither moves a candidate nor resolves/retire the Stack item. Stage the five
  cards first, including actually casting the card intended for the Stack row. This fixture checks
  multi-zone presentation and answer mapping; it is separate from Midnight Tilling's Mill behavior.

Adding a primitive is a proto arm, a `parse` case, and an engine handler — no new UI.

Public reveals use the [architecture contract](../../../../docs/ARCHITECTURE.md#public-reveal-contract).
Private look/search choices never publish public reveal metadata.

Optional choices use engine-authored bounds or decline flags. `Decline` submits an empty
resolution answer or removes the active optional cast-cost group's staged payments;
`Cancel` abandons an unsubmitted cast/activation, and `Back` returns from cost-object selection.
Mode/target-linked required costs cannot be declined. `ruled_resolution_choice_dialog.cpp`
owns the fallback picker and follows the same explicit-decline rule for a zero minimum.

Copy retargeting presents one target occurrence at a time, including its ordinal in the prompt.
Object and player candidates stay in their separate ID fields through click handling and command
submission. The optional empty action is labeled `Keep original target`; it preserves the original
generation-bound target receipt, including when that permanent has since left and returned.

### Opening actions

`LegalActions.opening` supplies recipient-specific starting-seat eligibility, keep/redraw
permissions, mulligan count, and remaining bottoming quota. The dispatcher never parses
opening labels or counts button clicks. Seat buttons use the published player IDs; bottoming
continues to use `HAND_ACTION_OPENING_BOTTOM` and the existing acknowledged command sequence.
Waiting seats receive no actionable permissions. The relay retains opening snapshots for
reconnect and clears them when opening ends; session reset clears the client mirror.

## Acceptance carried forward from retired reports

These are retained limits and repeatable checks, not new acceptance claims or a request to run
every historical check on each task. Issue delivery comments preserve dated results. Only the
applicable changed interaction needs new acceptance; use the game guidance for execution.

- Convoke (#145), Earthbend (#150), counter/Wither work (#157), and token copies (#46) have
  agent-operated GUI results on their closed issues; these do not establish human acceptance.
- Waterbend [#146](https://github.com/pizzaninja188/Cockatrice/issues/146#issuecomment-5458728218)
  did not complete desktop combat-timing or live zone-change stale-selection checks. A pre-existing
  freeform hidden-stack observation was outside that delivery; a clean general freeform-stack pass
  was not claimed. Token-copy #46 did not exercise separate freeform or missing-image fallback.
- Historical card deferrals to #155/#159 must not be treated as current blockers: both issues are
  now closed. Check current code and tracker for those cards rather than copying old report status.
- Blight's full six-card desktop checklist remains recommended beyond its focused performed checks.
- Colorstorm Stallion/double-faced token copies and registered-seat reconnect were not manually
  accepted in #237. Public reveal acceptance was explicitly deferred. Architecture fixtures did not
  establish visible acceptance of draw-replacement/private-discard consumers.

### Public reveals — deferred

Start two clients with `scripts/launch-ruled-game.ps1 -Dev`.

1. Behold a hand card with Elven Passage. Both clients should see the same named card; close the
   window on one client and confirm it remains on the other.
2. Put Spyglass Siren onto the battlefield, resolve its trigger, and use the Map on a creature.
   Use a Forest-only deck for this case so the top card is a land. Both clients should
   see the land window after it moves to hand, with no extra decision or counter.
3. Repeat in a game with a Storm Crow-only deck for a nonland top card. There should be one
   public window per client. Only its controller can choose graveyard or top; after confirming,
   both windows remain read-only and closeable.
4. Leave that completed window open during a second public choice. Clicking an old image must
   not select a candidate in the new choice. Reveal the same card again and check that it is a
   distinct occurrence; ordinary priority updates must not duplicate either window.
5. Resolve Mystical Tutor and Commune with Nature. Only the actually revealed card becomes
   public; the rest of the library/look remains private. Check source labels and retained images
   after movement/shuffling. Check Gifts Ungiven's split from both seats.
6. Leave a cast-cost/Ninjutsu reveal active while resolving another reveal; neither window should
   overwrite the other. Rejoin during the active reveal, then let its source leave the stack and
   confirm that the restored window becomes a dismissible snapshot.

### Colorstorm Stallion and double-faced copies — not performed

In two ruled dev clients, reach a main phase and use the active player's dev console:

```text
put bf Colorstorm Stallion ready
put hand Tidings
mana UUUUU
```

1. Cast Tidings and resolve only Opus. Both boards should show the original at 4/4 and one
   distinct 3/3 token while Tidings remains on the stack. Inspect haste, ward, and Opus on the
   copy. Resolve Tidings. A later five-mana spell should trigger both Stallions independently.
2. Put Reckless Waif on the battlefield and Cackling Counterpart in hand; add `UUU`, cast
   Counterpart targeting Waif, and resolve. Remove the original with the dev command or a
   spell. After a turn with no spells, resolve the token's upkeep trigger. Both clients should
   show Merciless Predator at 3/2 with its back-face ability and no duplicate token.
3. Reconnect a registered seat. Verify the transformed token's face, base/effective P/T,
   abilities, and count match the other client. Bounce the token with Unsummon; it should
   disappear on both boards and never remain in hand.

### Blight — remaining recommended matrix

Start two dev clients with seed 153 and arrange the named cards/mana.

- With a ready Gristle Glutton, a tapped/newly controlled Grizzly Bears, and Tatterkite, activate Glutton. The prompt must say “Blight 1”; Bears is selectable, Tatterkite is not. Cancel once: neither seat should see a committed tap or counter. Repeat and confirm: both seats see the source tap and Bears become 1/1. Only the controller chooses a discard; one card is drawn after an actual discard.
- Cast Cinder Strike, first skipping Blight, then paying it. Confirm 2 versus 4 damage. The Blight creature picker must not become a target-selection prompt or offer hand cards. Wild Unraveling must require exactly one of Blight 2 or the additional {1}.
- Resolve Dream Seizer with the 1/1 Bears available. Choose Blight, select Bears, and leave the opponent's discard prompt open. Both clients must still show the same Bears on the battlefield. Only the opponent sees discard candidates. Complete discard: that exact Bears moves to its owner's graveyard on both clients.
- Resolve both Blighted Blackthorn's entry and attack triggers. Paying Blight 2 draws one card and loses one life; declining does neither. A tapped attacker can be chosen to receive the counters.
- Resolve Chaos Spewer. Choosing Pay opens the staged {2} picker; Undo keeps that payment offer available, and final Decline enters Blight directly. Blight requires exactly one eligible creature when available. If the source has left and no eligible creature remains, final Decline completes without a creature prompt or counters.
- Verify that create/reset always selects ruled play and no local-game action is available. Shared card movement and activation presentation remain covered through ruled games.

### Draw replacement and simultaneous discard — not established by architecture fixtures

For affected consumers, check threshold/one-versus-all library choices; zero/partial/full
discard with count-only announcements and private identities; Abundance's public revealed prefix
with private bottom ordering; Tomorrow's private selection; reconnect at pending stages;
source/drawer/controller concession; exact physical movement and effect tail once. These steps
are retained from the architecture delivery; later card-level evidence must be checked separately.


### Upstream integration — user acceptance pending

On 2026-11-04 the user confirmed priority passing after casting, nonempty live-stack close
protection, and debug-replay stack visibility through backward/forward seeks. Remaining payment,
identity, privacy, resolution-choice and broader playback checks below are pending.

The ruled stack is an engine-owned view: while it has published objects, its close button and
View-menu dismissal are disabled. Replay tabs create the same stack action and automatically
restore physical spells and synthetic ability tiles when seeking. Mandatory choice windows keep
their close protection; resolution dialogs also reject Escape/window-close dismissal. Their UI
owners can still close them when the engine completes or replaces the choice. Completed reveals
and optional exile-cast permission views retain local dismissal.

The live-stack close protection and debug-replay seek checks are user-confirmed. For the remaining
resolution-choice check, Escape must keep an unanswered choice open, while a legal choice (or
explicit Decline when offered) advances normally. That hands-on check remains pending; automated
offscreen tests cover dismissal and a replay containing both a spell and a triggered ability.

Use matching protocol-10014 fork clients and Servatrice. From the integration checkout, run
`scripts/launch-ruled-game.ps1 -Dev` in the interactive user session. Complete the opening prompts.
The integration build uses the short `X:` drive alias; keep that alias while using its build cache.

1. Conjure `Lightning Bolt` in the first seat's hand and `Grizzly Bears` on the other battlefield;
   add `R` to the first seat. Use the card's context-menu Play action, select the opposing Bears,
   pay and submit. Both clients must show one physical spell on the stack, with an arrow to the
   selected permanent; resolving it must remove that exact creature and move the spell once.
2. Repeat with multiple spells or abilities pending. The stack window must retain every item in
   engine order, with arrows anchored to the visible stack copies. Payment counters, restricted
   mana rows and cancel/retry prompts must update on the same primary action controller.
3. Reconnect a registered seat during a pending choice or payment. Its hand, public zones,
   command-zone view, priority and current prompt must match the other seat's permitted view;
   private choices and face-down identities must remain recipient-only. Repeat with a spectator
   whose omniscient permission is disabled.
4. Open an exported ruled report, seek backward and forward repeatedly, and confirm that prompts,
   stack items, player graphics and zone views rebuild without duplicates or stale windows.

These are requested user checks, not evidence of completed GUI acceptance. Automated offscreen
playback, model, relay and network tests establish their own narrower contracts.
