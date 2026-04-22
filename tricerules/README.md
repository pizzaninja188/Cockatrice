# tricerules — MTGO-style rules engine (sidecar)

Rust workspace that implements an authoritative game engine and exposes it over TCP to Servatrice.

## Build

```bash
cd tricerules
cargo build --release
```

The `tricerules-server` binary listens on `127.0.0.1:17381` by default (override with `TRICERULES_PORT`).

## Run with Servatrice

1. Start `tricerules-server` before or after Servatrice.
2. Set `TRICERULES_HOST` / `TRICERULES_PORT` if non-default.
3. Create a **Ruled game** from the Cockatrice “Create game” dialog (checkbox).
4. When the match starts, Servatrice opens a session to the sidecar and forwards `Command_RuledPayload` intents.

## CMake

Configure with `-DWITH_RULES_ENGINE=ON` to add a `tricerules` CMake target that runs `cargo build --release` (requires `cargo` on `PATH`).

## Protocol

- Framing: 4-byte big-endian length + protobuf message.
- Payload messages are defined in `libcockatrice_protocol/libcockatrice/protocol/pb/ruled_v1.proto` (`package ruled.v1`).
- Cockatrice wraps `ruled.v1.RuledCommand` / `RuledEventBatch` bytes in `Command_RuledPayload` / `Event_RuledPayload` extensions (`game_commands.proto` / `game_event.proto`).
