# Cockatrice game-client agent guidance

The repository-root `AGENTS.md` still applies. This file owns ruled game UI integration, client state, prompts, upstream hooks in `player_actions`, and manual two-client verification.

## Ruled client ownership

All substantial ruled client logic belongs under `cockatrice/src/game/ruled/`; upstream files keep short guards or member indirection. Read `ruled/README.md` for the complete responsibility and signal map.

- **`RuledClientState`** mirrors the engine view: identity maps, legal actions, combat staging, stack tracking, pending choices, and ruled signals.
- **`RuledEventDispatcher`** applies one `RuledEventBatch`. Add one private method per event kind plus one `has_*()` dispatch line; do not add large inline event blocks.
- **`RuledActions`** owns click interpretation and `CardItem` lookup. `RuledActions::isRuledGame(game)` is the only place that reads the ruled-game flag.
- **`RuledClientHost`** is the UI seam implemented by `GameEventHandler`. Keep `ruled_client_state.cpp` and `ruled_event_dispatcher.cpp` free of `AbstractGame`, `Player`, and `CardItem`; expose required UI operations through the host interface so headless client tests remain linkable.

Engine legal actions, targets, prompts, and object identities are authoritative. The client may display Oracle data but must not infer ruled legality or reconstruct engine IDs from names.

## Prompt UI

`game_prompt_widget` is fork-owned and stays in the Messages dock above the log, ruled and non-replay only.

- One exclusive `PromptMode` and one `RuledPromptState` carry the active prompt. Add a new mechanic as an enum case and handle it in both visibility and label switches; do not add parallel booleans.
- `TabGame::refreshRuledPromptState()` owns mode selection. `TabGame` otherwise handles placement and signal connections.
- Targeting is derived from `TargetingSources`; combat, priority, and sticky blocker errors remain orthogonal inputs.
- Reuse existing commands and signals for UI text and actions before adding protobuf.
- A valid one-of-one cast-cost group advances as soon as the option and any required object selection are complete. Do not add a redundant Confirm Costs step; retain explicit confirmation for multi-object cohorts.
- Widget visibility tests use `isHidden()` because offscreen tests do not show the parent widget.

## Client verification

Read `../../../docs/AGENT-VERIFICATION.md`. Client-only iteration builds the Cockatrice and touched test targets, then runs `ruled_client_test`, `game_prompt_widget_test`, and any specifically affected client test. Protobuf or relay changes require the full cross-component gate and `ruled_e2e_smoke_test`.

Qt module additions for tests belong in `cmake/FindQtRuntime.cmake` through `_TEST_NEEDED`, not in a per-test `CMakeLists.txt` workaround.

For touched interaction paths, verify the actual context-menu action, target-arrow anchor, picker annotations, popup count, and payment progression as applicable. Trace the engine offer through relay identity and the real click/render path; a passing view-model or relay test does not prove that interaction. Add focused coverage at the touched UI seam and specify remaining hands-on acceptance.

## Manual verification on Windows

Automated tests stop at the view model. For interaction, networking, hidden-information, or cross-zone identity behavior, launch a real ruled game from the repository root:

```powershell
./scripts/launch-ruled-game.ps1
./scripts/launch-ruled-game.ps1 -Dev
./scripts/launch-ruled-game.ps1 -Stop
```

The launcher starts the sidecar, Servatrice, and two autopiloted clients. Run `-Stop` before rebuilding so live binaries do not block the linker.

The dev console creates focused states without deck editing or turn setup:

```text
put hand Serra Angel
mana 4WW
put bf Grizzly Bears ready
put 2 bf Hill Giant
move gy Serra Angel
help
```

`put` always conjures and is limited to hand or battlefield. `move` relocates an existing object and is the path to graveyard, exile, or library. Dev commands are accepted only when both the session and sidecar gates are enabled; they remain logged commands. `put bf` fires ETB/static registration but no cast trigger, and `put gy` deliberately does not fire dies triggers.

Final summaries must distinguish manual steps actually performed from recommended steps. For privacy or identity changes, verify both seats' visible state, physical zone movement, and cross-zone identity rather than relying only on headless tests.

Record user-performed acceptance as user-confirmed. When the user defers testing, keep it deferred while completing applicable automated gates and already-authorized delivery.
