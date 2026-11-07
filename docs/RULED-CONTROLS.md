# Ruled game controls

The controls below the current prompt share their actions with configurable keyboard shortcuts.
Change bindings in shortcut settings; top-row and numpad digits work by default. Shortcuts do not
fire while typing in chat, search, or numeric fields, and holding a key does not repeat it.
Spectators and replay viewers have no gameplay controls.

| Key | Action |
| --- | --- |
| 1 | Confirm the current enabled primary action, or pass priority once. Missing selections are never supplied automatically. |
| 2 | Pass until a response or an unconfigured trigger, a required choice, or the end of this turn. |
| 3 / 4 | Answer the current engine-offered Yes / No choice. No is not a general Cancel action. |
| 5 | Clear this game's trigger/answer rules, temporary yields and one-time stop; disable automatic passing. Keep the persistent phase-stop layout. |
| 6 | Pass through the rest of this turn, including new stack items. Required choices still stop you. |
| 8 | Toggle no-action auto-pass. The engine retains priority whenever availability is uncertain, and respects phase stops. |
| 0 | Yield toward this turn's end step, stopping for a response or required choice. If already there, stop there. |

The status line shows acknowledged state. “Updating automation” means a request is awaiting its
reply. Clear All and Full Control remain available while another gameplay command is in flight;
they cannot undo a resolution already accepted by the engine.

**Full Control / Hold Priority** stops automatic passing and answers without deleting saved rules.
Manage can pause, resume, inspect or remove rules. A required choice cancels temporary yielding;
answering it leaves automation paused until you explicitly pass or resume it. Disconnect also
cancels temporary yielding and pauses retained preferences. Reconnecting does not resume them.

## Stack menu

Right-click a spell or ability in the stack window:

- **Yield to this item once** passes only when that exact item is on top.
- **Yield to current stack — stop on new items** interrupts when anything new enters the stack.
- **Yield until stack is empty — including new items** includes new responses and triggers.
- For a triggered ability, **This source** remembers that source incarnation and ability.
  It expires when the source changes zones; blinking and reanimating create a new incarnation.
- **All matching sources** explicitly broadens the rule to the same engine-defined ability and
  captured controller. Different abilities with similar wording do not match.
- Eligible optional triggers additionally offer **Always Yes**, **Always No**, and **Ask each time**.
  This preference is separate from yielding priority and covers only that trigger's outer optional
  decision. It does not select targets, make payments, or answer another choice inside its effect.

A trigger rule acts only when its matching trigger is on top. It does not pass over unrelated
objects above it. A resolving optional trigger retains its menu while its Yes/No prompt is open.
When the source incarnation has already left, only the broader matching-sources option is available.
Stack yields stop at stack-empty before proceeding to another phase.

## Phase bar

The existing top/bottom boxes retain persistent opponent-turn/own-turn stops. Right-click a phase
for **Stop here once** or **Yield until here**, with an explicit own-turn/opponent-turn scope.
An exclamation mark identifies a one-time stop; an arrow identifies a yield destination. The damage
phase offers separate first-strike and regular combat-damage choices. Untap has no priority stop.

## Pending user GUI acceptance

Automated widget, relay and engine tests do not establish hands-on GUI acceptance. From the repository
root, launch `./scripts/launch-ruled-game.ps1 -Dev` for the following checks, then use `-Stop` before
rebuilding. Dev fixture commands are per seat; the control semantics are the same in ordinary games.

1. While typing a message, enter `12345680`: it should remain text. Focus the battlefield and check
   the enabled controls using both top-row and numpad keys. Rebind one key and verify its new label.
2. Use `put bf Garruk's Packleader`, then `put bf Hill Giant`. Right-click the resulting trigger.
   Test source-only yield and its separate Always Yes/No options; confirm the optional prompt can
   still be configured while resolving. A required target or payment must remain interactive.
3. Put two Elvish Visionaries onto the battlefield. In both seats, yield to the stack. Clear All
   between resolutions: remaining items must stop. Compare current-stack and entire-stack yields
   by adding a response from the other seat. Check both seats' stack tiles and resolved-card zones.
4. Move Packleader to the graveyard with `move gy Garruk's Packleader`; its source rule must disappear.
   A new Packleader must require a new source rule. Explicit broader rules remain listed in Manage.
5. Test a one-time end-step stop and Full Control after casting your own spell. On a server that retains
   disconnected seats, reconnect a yielding seat: preferences must display paused and require an explicit resume.

These controls implement ordinary priority and optional-choice decisions under CR 117.2e,
117.3b–d, 117.4–5, 603.3b and 603.5 ([November 6, 2026 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020261106.txt)).
They do not change card definitions, payment legality, state-based actions, or trigger ordering.
