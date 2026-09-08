# Ruled E2E harness

This target runs real Servatrice and tricerules-server processes with two independent protobuf
clients. Keep new scenarios in this target so they retain the existing binary discovery,
deterministic seed, timeouts, capture artifacts, and failure transcripts.

## Ownership

- `ruled_e2e_session` owns process setup and teardown, server configuration, artifacts, and logs.
- `ruled_e2e_client` owns framing, session commands, acknowledgements, bounded pumping, and
  synchronous observation hooks. Command helpers send only when explicitly called.
- `ruled_e2e_observed_state` decodes one recipient's physical events, legal actions, choices,
  previews, and identity maps. Decoding never sends commands or evaluates scenario expectations.
- `ruled_e2e_opening_driver` supplies the shared opening policy and common batch assertions.
  Focused tests keep their opening hands; the seeded driver requests the existing single mulligan.
- `ruled_e2e_seeded_driver` owns the long seeded game's roles, progression, and milestones.
  Focused drivers live beside their tests in the casting/payment, zone/visibility, and
  combat/permanent translation units. Opening, diagnostics, and disconnect tests have their own unit.

## Adding a scenario

Use `RuledE2ESmokeTest` and an `OpeningDriver` for each seat. After opening, submit the offered
actions explicitly from the test. Use the decoded state and retained event vectors for assertions.
If the scenario needs intermediate milestones, derive a local driver and override only the
relevant hooks; keep its flags next to the test. Do not add card-specific branches to the shared
client or decoder. Both seats keep their own observations, even when the test compares them.

Hooks run synchronously, in received order:

1. A response is stored and a rejection logged before `onResponse` runs.
2. Each physical event is decoded before `onPhysicalEvent`; a ruled payload is then decoded.
3. An ordinary batch increments `stateVersion`, calls `onBatchBegin`, decodes each ruled event
   followed by `onRuledEvent`, then calls `onBatchEventsComplete`.
4. Recipient legal actions are updated last, followed by `onLegalActions` when present. Omitted
   legal actions retain the preceding observation; an explicit empty entry replaces it.
5. A payment preview updates only the preview observation and invokes `onPaymentPreview`.
   It does not advance the game version or run ordinary batch hooks. A scenario may submit its
   next command from that callback, as the seeded payment flow does.

Derived drivers should call the corresponding `OpeningDriver` hook when overriding its common
assertions. Battlefield omission retains the previous objects; an explicit replacement can clear
them. Never combine private data from the two clients into a recipient view.

## Verification

Run the E2E CTest with `RULED_E2E_REQUIRE=1` before and after an extraction; missing binaries must
fail the required run. Use the Windows quiet runner described in
[`docs/AGENT-VERIFICATION.md`](../../docs/AGENT-VERIFICATION.md).
Finish with `./scripts/verify.ps1 -Side Cpp`, which requires E2E prerequisites automatically.
The harness characterization tests supplement the real-server scenarios; they do not replace them.
