# Ruled bug reports, reconstruction, and live resume

Ruled games automatically record structured evidence on the client and Servatrice. The engine
remains the only rules-state writer. Captures are local files; there is no upload, issue creation,
remote capture download, or video recording.

## Report a problem

1. In Cockatrice, choose **Help → Report Bug…**. If several ruled games are open, select the game.
2. Describe what you expected, what happened, and any useful notes.
3. Review the game screenshot. It was taken before the report dialog opened; you can remove it.
4. Export a new ZIP, review it, and send it to your maintainer using your normal sharing channel.

The ZIP contains your client's received game data, local UI state, input/command trace, and optional
screenshot. This can include your private cards and game chat. Text-entry keystrokes are not logged;
submitted game messages can appear in the game command/event stream. Passwords and login commands
are outside the per-game recorder. Typed notes and screenshots are user-supplied evidence.

The report ID and latest server capture ID connect the report to the maintainer's server files.
The client sends an authenticated report marker, which does not execute an engine command. A
disconnection can prevent that marker reaching Servatrice; the previously received capture ID can
still locate the server evidence. Neither identifier grants access to server files.

Reports can export a stopped/incomplete capture's surviving prefix. Read `complete`, `status`,
`error`, and reader warnings before drawing conclusions. The screenshot and report notes can be
newer than the last recorded state. A game with capture disabled or no writable capture directory
has no earlier evidence to export.

## Files and privacy

Every capture has a unique directory containing:

| File | Meaning |
| --- | --- |
| `README.md` | Portable instructions included with the evidence |
| `manifest.json` | Format/build/data identity, privacy, capture/report IDs, completeness, retention |
| `timeline.jsonl` | Ordered UTF-8 records, one complete JSON object per line |
| `raw/<sequence>.pb` | `ruled.diagnostics.Record` wrapping the exact protobuf message |
| `state/<name>-<sequence>.json` | Periodic full state at a recorded boundary |
| `state/<name>-latest.json` | Latest convenience snapshot; use the timeline reader after a gap |
| `report.md`, `report.json` | Human and machine readable report fields, when exported by the client |
| `screenshots/game.png` | Optional pre-dialog game screenshot |

Client captures have `source: "client"`, `privacy: "recipient_only"`. They cannot reconstruct omitted
or redacted information. Server captures have `source: "server"`, `privacy: "server_only"` and include
all decks, hidden zones, deterministic startup information, and authoritative engine state. Keep
server exports within the maintainer/debugging group authorized to see that information.

The [format schema](ruled-diagnostics-format.schema.json) describes the envelope. Message `data` is
decoded from the protobuf descriptor: proto2 extensions and known nested ruled payloads are expanded,
enums have symbolic names, and absent fields are `null`. Empty collections mean empty, not omitted.
Unknown bytes remain base64. All 64-bit integers and timeline sequences use decimal strings; do not
round-trip them through JavaScript `Number`. Rust composite map keys remain structured entries.

Names supplement explicit identities. Engine ObjectId, card definition ID, known Oracle/display name,
physical Server_Card ID, owner/controller/recipient ID, engine hand slot, face index, and zone-change
generation remain distinct fields. Server relay state includes physical bindings, library identity,
hand order, stack mapping, catalog, and pending visual updates. Client state includes the recipient's
legal offers, identity maps, choices, reveal state, staged payments/transactions, prompt labels and
button state, physical card positions, and selection. Unknown private names stay unknown.

Example timeline shape (abbreviated data):

```json
{"format_version":1,"sequence":"12","kind":"client_result","utc":"2026-09-06T12:00:00.000Z","elapsed_ms":"400","correlation_id":"request-uuid","data":{"response_code":"RespOk","command_index":"3"}}
```

Common kinds are `client_request`, `client_response`, `client_command_blocked`, `ui_input`,
`ui_input_result`, `received_game_events`, `server_context`, `engine_request`, `engine_response`,
`engine_transport_error`, `relay_projection`, `recipient_event`, `state_snapshot`, `state_delta`,
`report_marker`, `capture_gap`, and `capture_closed`. A request is not proof of acceptance: correlate
its response. A local click with no observed command is explicitly distinguished from a rejected
command. Input timing is evidence, not an executable UI macro.

Snapshots are named `engine`, `relay`, and `client`. Between full snapshots, state deltas contain
JSON-pointer paths and `add`, `remove`, or `replace`, with `before` and/or `after` values. The reader
checks delta preconditions and starts from the latest full snapshot at or before the requested row.

## Inspect and export

Build the current tools with `./scripts/build-ninja.ps1`. The following commands accept capture
directories, `manifest.json`, or the ZIP format emitted by these tools:

```powershell
./scripts/inspect-ruled-capture.ps1 -Capture report.zip -Validate
./scripts/inspect-ruled-capture.ps1 -Capture report.zip -Kind client_command_blocked
./scripts/inspect-ruled-capture.ps1 -Capture report.zip -Object 42 -Seat 0 -Markdown
./scripts/inspect-ruled-capture.ps1 -Capture report.zip -Command cast_spell
./scripts/inspect-ruled-capture.ps1 -Capture report.zip -Report <report-uuid>
./scripts/inspect-ruled-capture.ps1 -Capture report.zip -State client -To 120 -Output build/client-at-120.json
```

`From`/`To` are timeline sequences, not engine command counts. Filters search decoded evidence;
`-AllowProtocolMismatch` explicitly permits comparing readable data with a changed protocol without
requiring exact raw/readable equality. It does not make an incompatible engine replay authoritative.

On the server machine, a maintainer with filesystem access can export by capture or report UUID:

```powershell
./scripts/export-ruled-capture.ps1 -CaptureRoot D:\captures\server -Id <uuid> -Output full-capture.zip
# Or select an exact server directory:
./scripts/export-ruled-capture.ps1 -Capture D:\captures\server\<capture-uuid> -Output full-capture.zip
```

Exports preserve existing destinations by refusing to overwrite them. ZIP extraction validates CRCs,
paths, duplicates, bounds, and matching headers before writing into a new empty directory. Version 1
uses portable uncompressed ZIP members; externally recompressed ZIPs are not accepted by the built-in
reader. Extract those with your normal archive tool and pass the resulting capture directory.

## Reconstruct an engine boundary

Reconstruction requires the maintainer server capture. Client reports and older Cockatrice replays
do not contain enough information for exact engine reconstruction.

```powershell
./scripts/replay-ruled-capture.ps1 -Capture full-capture.zip -StopAfter 24 -Output build/after-24
./scripts/replay-ruled-capture.ps1 -Capture full-capture.zip -StopBefore 24 -Output build/before-24
./scripts/replay-ruled-capture.ps1 -Capture full-capture.zip -RetrySequence 81 -Output build/retry-81
```

The count is the number of **accepted engine PlayerCommand requests**, including canonical commands
that may themselves perform automatic priority passes. Zero means immediately after SessionStart;
`StopBefore 1` is the same boundary. Rejected requests and preview queries do not increase this count.
`RetrySequence` selects the timeline sequence of an engine request and retries it after reconstructing
its accepted predecessors, including when its original response was rejected or absent.

The tool uses the same `EngineSession` constructor, deck resolver, dev gate, command handler, and
publication caches as the live sidecar. It compares each replayed response and complete engine state,
reports the first differing boundary/JSON field, and writes `summary.json`, `engine-state.json`, and
`differences.json`. Diagnostic JSON is never loaded as executable engine state.

The default requires matching engine source/dependency fingerprint and card registry content hash.
Use the original build for exact reproduction. `-AllowBuildMismatch` explicitly enables comparison
against a changed build/data set and labels differences; it is not a claim of exact reproduction.
Exit status is 0 for a matching prefix, 2 for differences, and 1 for invalid/incompatible input.

## View the recorded client

Choose **Help → Open Bug Report…** and select a ZIP or manifest. A server capture asks which recorded
recipient to view. The board uses the actual GameReplay handlers, GameEventHandler, and ruled
dispatcher with that recipient's recorded events. It has no network connection or playable controls.

The green timeline bar and playback controls open in a detached window. The event list stays beside
the evidence tabs, which show recorded local UI state, current-build playback state, field differences, report text,
and screenshot. Prompt and screenshot content scroll within their panels. Seeking preserves your
window and dock arrangement. Select a row to seek; rewind clears the old state and
replays the prefix. Recorded selections, payment staging, window geometry, and click timing remain
recorded evidence: the board does not simulate those local interactions. A server capture alone has
no recorded client-widget state, so that comparison is marked unavailable. Disabled playback controls
and different local geometry naturally differ from the original client state.

For automated current-client playback/export:

```powershell
./build/windows-ninja-all/cockatrice/cockatrice.exe --open-ruled-report report.zip --report-seat 0 --export-playback build/playback.json
```

Batch export uses a temporary client profile and exercises forward and backward seeking. For headless
Windows execution, set `QT_QPA_PLATFORM=offscreen` and `QT_QPA_PLATFORM_PLUGIN_PATH` to the development
Qt kit's `plugins/platforms` directory. The normal deployed Windows client carries the Windows plugin.

## Resume locally with real clients

```powershell
./scripts/launch-ruled-game.ps1 -Capture full-capture.zip -StopAfter 24
```

The launcher builds, validates/reconstructs the accepted prefix, creates a unique
`build/ruled-resume/<run-id>` directory, and starts fresh loopback-only sidecar/Servatrice processes
on separate available ports. It starts one visible client for each recorded player, in recorded seat
order, and loads display decks through normal pregame actions. It restores each client's toolbar
stops before they publish their policy. The engine receives the original seed, player IDs, ordered
decks, effective dev-command gate, and accepted commands. The relay rebuilds physical card IDs using
its normal projection; engine identities/generations and the recorded player IDs remain authoritative.

Every reconstructed step is checked by engine-state SHA-256 before play continues. The child capture
records `parent_capture_id`, `resume_stop_after`, and comparison mode. Incompatible or invalid plans
abort startup. A capture gap limits resumption to its valid prefix. After startup, the clients are
ordinary interactive clients: answer the pending choice or perform the next legal action.

`-AllowBuildMismatch` permits an explicitly labeled local comparison run. The launcher cannot combine
`-Capture` with `-Seed`, `-Dev`, `-DeckA`, `-DeckB`, `-Freeform`, or `-NoServers`; those would change or
reuse the captured session's inputs. It restores all environment variables it changes and never
reuses or stops unrelated server processes. If an executable is held by another running instance,
close that specific instance before rebuilding.

The launcher prints the exact teardown command:

```powershell
./scripts/launch-ruled-game.ps1 -Stop -RunDirectory '<printed run directory>'
```

Teardown verifies each recorded PID, executable path, and process start time before stopping it.
Evidence is retained. The launcher supports 2–99 seats, matching the client pregame count range.

## Storage and failure behavior

Set `COCKATRICE_RULED_CAPTURE_DIR` to choose the capture root. Client/server subdirectories are
separate. Otherwise the root is the process's Qt local application data directory under
`ruled-captures`. `COCKATRICE_RULED_CAPTURE=0` disables capture for that process.

Captures retain seven days by default; a report marker pins its capture for 30 days. Cleanup runs
when a new capture is opened, under a pruning lock. Active sessions and pinned reports are preserved.
The default root cleanup quota is 2 GiB; server operators can set `COCKATRICE_RULED_CAPTURE_QUOTA_MB`.
Active/pinned evidence can exceed that root quota. A single client capture stops at 256 MiB and a
server capture at 512 MiB, with small manifest/gap bookkeeping overhead. Per-response diagnostic
snapshots are limited to 8 MiB and leave headroom within the sidecar frame budget. Capture failures
mark evidence incomplete instead of rejecting gameplay.

Raw records use atomic file replacement before their JSONL row is appended and flushed. Full
snapshots and manifests are atomic files. A crash can leave an orphan raw/snapshot file or a partial
last line: readers retain only complete, contiguous timeline records, exclude a truncated final row,
and stop at the first explicit gap. An unpaired engine request is unfinished, not accepted. No
complete-session claim is inferred from a destructor or from having some readable files.

Captures are diagnostic input, not trusted instructions. Tool readers bound sizes, reject unsupported
versions and path traversal, and do not execute strings or deserialize a captured engine object.
The live resume plan is a maintainer-owned local artifact, never a client network command.

## Interaction checklist and acceptance

1. **Rules authority:** no card rules or legal actions added. Diagnostic vocabulary supports multiple
   mechanics, including interrupted resolution choices and staged/nested mana payments. It extends
   raw ruled transport/replay with correlated state evidence; existing public events cannot contain
   full hidden engine state and existing legacy replays cannot supply deterministic startup inputs.
2. **Identity/state:** tricerules is the writer; relay maps physical identities and filters recipients;
   Qt records/displays its own state. Resume reexecutes logged commands, including dev commands under
   their original effective gate. No state injection bypasses zone changes or generation checks.
3. **Timing/interactions:** snapshots occur at command/event boundaries; pending choices, triggers,
   replacements, characteristics, attachments, combat, and payments are serialized as evidence and
   reconstructed through existing engine execution. New rule ordering behavior: N/A.
4. **Players/failures:** capture and resume iterate player sets and preserve recorded IDs. Readers
   reject malformed versions, raw/readable mismatch, bad paths, and divergent strict replay. Rejected
   and unfinished requests remain distinct from accepted commands.
5. **Visibility:** full engine/relay data stays server-only; recipients receive correlation metadata
   and their normal filtered events. Client reports contain no server-state download. Omissions and
   unknown identities remain explicit.
6. **Propagation:** Rust shared session/snapshots → server-only IPC fields → Servatrice journal and
   normal projection → recipient metadata/events → Qt journal, report, and read-only viewer. Live
   resume uses that same path. Freeform and normal replay behavior remain gated separately.
7. **Verification:** focused red/green coverage for decoding, snapshots/deltas, quotas, archive/report
   export, identity, reconstruction, and toolbar restoration; real server reconstruction/resume and
   offscreen recipient playback/ZIP/CLI checks; full affected-side gate is
   `./scripts/verify.ps1 -Side Both`. No card-data regeneration is required.

Hands-on acceptance remains separate from automation. Recommended two-client check: cause a ruled
pending choice or payment, export Report Bug with and without its screenshot, inspect both exports,
open the report and seek around the failure, obtain the matching maintainer capture, resume just
before the relevant accepted command, then answer the choice through the live UI. Verify both
players' hands remain private, prompt/toolbar state is correct, and ordinary freeform still works.

**MTG applicability:** the change observes and reexecutes existing opening choices, priority,
resolution, payment, and zone/identity behavior. It introduces no new MTG rule semantics or intentional
mechanics deferrals. Compliance rests on the same authoritative engine path and existing legality
checks, with diagnostic/replay regressions covering preservation rather than a second rules model.
