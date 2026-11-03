# Reusable dev-console setup presets

These files prepare test states with existing dev commands. They do not prove legal gameplay
reachability, answer opening choices, cast spells or resolve choices. No MTG rules surface area
is changed by the preset loader; the existing engine/card definitions decide all gameplay.

From the repository root, launch a fresh local game:

```powershell
./scripts/launch-ruled-game.ps1 -Dev
# For the multiplayer preset instead:
./scripts/launch-ruled-game.ps1 -Players 4 -Dev
```

Finish the starting-player/opening-hand choices. Reach your main phase with priority and an empty
stack, with no staged cast, payment or pending choice. Keep that phase stopped while setting up.
In the Messages dock, choose **Load preset…**, select one of the files below, review its preview,
then choose **Run (adds state)**. Loading alone sends nothing. For multiplayer, run from seat 1.

| Preset | Prepared state after every command is accepted | Manual check |
| --- | --- | --- |
| [aura-activation.txt](aura-activation.txt) | Local ready Drudge Skeletons, Stuck in Summoner's Sanctum in hand, and `2UBB` mana added (3 commands) | Activate Skeletons' regeneration for `B` before attaching the Aura. Cast the Aura for `2U` targeting Skeletons, pass priority to resolve it and its entry trigger, then check that Skeletons' regeneration action is disabled or absent even with `B` left. |
| [multiplayer-targeting.txt](multiplayer-targeting.txt) | Local Decimate in hand and `2RG` mana added; seat 2 Sol Ring, seat 3 Grizzly Bears, seat 4 Propaganda and Forest (6 commands) | Cast Decimate. Select Sol Ring, Grizzly Bears, Propaganda and Forest in their respective target groups. Verify targets/arrows across all three other seats; resolve through all seats and verify the four cards move to their owners' graveyards. |

Run sends one command at a time and advances only after engine acceptance. A syntax error anywhere
in a file sends **zero commands**. In a two-player game, `multiplayer-targeting.txt` fails its seat-3
preflight and sends **zero commands**, including no Decimate. A runtime rejection stops the tail;
earlier accepted commands remain. For a small rejection check in a fresh dev game, load a scratch
file containing the following three lines: the first adds `W`, the second is rejected because
`put` cannot conjure into a graveyard, and the final `R` must never be added.

```text
mana W
put gy Serra Angel
mana R
```

While a command is pending, input/Run are locked. **Cancel remaining** stops unsent lines and waits
for the command already sent; it does not undo that command. Disconnecting, closing the game or
starting a new session stops the run without automatic resumption. A fresh Run starts the file
again, adding copies/mana. Use a fresh game for each clean check.

Accepted commands use the existing capture/engine-request path. Inspect their responses with the
[diagnostics tools](../RULED-DIAGNOSTICS.md); a sent request alone is not acceptance. Use the existing
capture/replay/resume tools when a prepared state must be reconstructed, rather than treating a
preset as a saved engine snapshot. The [console reference](../../cockatrice/src/game/ruled/README.md#dev-loop-console)
documents the file bounds and grammar.

GUI acceptance for these checks remains pending until a user performs and confirms them. Automated
runner/widget tests establish sequencing and failure handling, not those hands-on card interactions.
Stop the launched processes when finished:

```powershell
./scripts/launch-ruled-game.ps1 -Stop
```
