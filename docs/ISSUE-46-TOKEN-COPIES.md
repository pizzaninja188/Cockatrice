# Token copies and Populate

Bounded implementation of [issue #46](https://github.com/pizzaninja188/Cockatrice/issues/46): Cackling Counterpart, including Flashback, and Wake the Reflections. Existing targeting, copy-source selection, token display, and physical binding are reused. No new UI controls or wire fields are added.

## Rules interaction checklist

1. **Authority and scope.** The engine snapshots copiable values and creates tokens. `CreateTokenCopies` supports one filtered permanent source and a count (Cackling Counterpart, Quasiduplicate); `Populate` supports Wake the Reflections and Rootborn Defenses. Oracle text and rulings for the two authored cards were checked through Scryfall. Double-faced token construction and effects that copy with modifications remain deferred; both cards retain partial metadata. Attempting the deferred physical DFC case produces an explicit unsupported log instead of silently creating a single-faced approximation.
2. **State and identity.** `GameObject.token_origin` owns intrinsic token characteristics, separate from later copy-layer values and registry provenance. Token status survives leaving the battlefield until the token ceases to exist. A card copying a token remains a card. Engine object ID, definition ID, display name, face index, and physical server card ID remain distinct. Populate choices retain object generations and use the existing logged command; repeated seed and command sequences produce identical batches.
3. **Timing and interactions.** Targeted copies use normal cast legality and resolution revalidation. Populate chooses during resolution, is mandatory when candidates exist, and does not target. Creation reuses simultaneous token-entry processing, replacement choices, triggers, and SBAs. Parked entry choices preserve the remaining effect sequence. Copiable values exclude counters, damage, attachments, tapped status, and later pumps. Captured activated and triggered abilities resolve without requiring a registry-backed token or a surviving source. Face-down sources produce only public anonymous 2/2 values. Room snapshots retain both doors and independent lock state. No new combat, cost-payment, or delayed-trigger mechanic is introduced.
4. **Players and failure paths.** Candidate selection uses current controller, not owner, and does not assume two players. New tokens belong to the creator. Wrong-player, empty, duplicate, noncandidate, and stale-generation choices are rejected without consuming the prompt. No creature tokens means no choice and no token. A targeted source that becomes illegal creates no copy. Tokens that have left the battlefield cannot move again before the next SBA.
5. **Visibility.** Copy characteristics and battlefield identities are public. Face-down underlying names and registry IDs are excluded from token creation. Only the deciding player receives an interactive Populate choice; other recipients use the existing wait/redaction path. No hidden-zone picker or new private field is introduced.
6. **Propagation.** `TokenCreated` carries the final identity after entry replacements. Existing relay synchronization creates independent physical tokens even when a real card and several tokens share a name or definition ID. Existing Qt copy-source selection honors the engine's minimum and maximum. Protobuf edits are documentation only. Production relay/client paths and freeform behavior are unchanged; characterization tests cover relay resync/removal, mandatory selection, and both clients through a real server/sidecar session.
7. **Verification.** Red/green regressions cover registry-free snapshots, captured target groups and legality, intrinsic entry status, token lifecycle, and runtime Room snapshots. Scenarios additionally cover Populate validation, source ownership, shroud, copied ETBs, Flashback, replay, and an entry choice interrupting Populate's effect tail. Full gates follow `AGENT-VERIFICATION.md`; logs are under `build/verification-logs`. Direct desktop checks are recorded below separately from automated E2E.

## Two-client acceptance

Launch `./scripts/launch-ruled-game.ps1 -Dev -Seed 46`. Keep both seats visible and reach player 1's main phase. In player 1's dev console, run each line:

```text
put bf Serra Angel ready
put hand Giant Growth
put hand Cackling Counterpart
put hand Wake the Reflections
put hand Unsummon
mana 20UUUUUUWWGG
```

1. Cast Giant Growth on the Angel and pass priority on both seats. Both clients should show 7/7. In the same turn, cast Cackling Counterpart targeting it and resolve. A distinct 4/4 Angel token should appear on both boards; the original remains 7/7.
2. Cast Wake the Reflections. Player 1 must select the creature token using the existing copy-source interaction; declining is unavailable. Player 2 waits and cannot submit. Selecting it creates one more independent 4/4 token on both boards.
3. Cast Unsummon on one token. That exact token disappears on both seats and never becomes a hand card. The original Angel and the other token retain their identity and positions.
4. Cast Cackling Counterpart from the graveyard using Flashback. Target the remaining token and resolve. A new 4/4 token appears, and Cackling Counterpart ends in exile.
5. Inspect the token's name, artwork/fallback, power/toughness, flying, and vigilance on both seats. Confirm targeting and selection highlights distinguish all same-name physical objects. A freeform game should retain its usual manual interactions.

Stop the dev session with `./scripts/launch-ruled-game.ps1 -Stop` before rebuilding.

### Desktop result, August 27, 2026

Steps 1–4 passed through Computer Use on two real rebuilt Qt clients, with seed 46 and local Servatrice/sidecar. Both seats showed the original Angel at 7/7 and copied tokens at 4/4. The Populate prompt highlighted only the token, offered no decline action, and ignored a click on the original; the other seat displayed a wait. The selected token disappeared on both boards after Unsummon without entering the hand, while the original and populated token retained their positions. Flashback displayed the {5}{U}{U} payment and stack annotation, created another distinct 4/4, removed the spell from the graveyard, and put it in exile on both seats. Token artwork, names, labels, and printed flying/vigilance were visible. All four test processes were stopped afterward.

These were agent-operated desktop checks, not human acceptance. A separate freeform desktop session and fallback artwork with a missing local image were not exercised; no production freeform or client display code changed.

## Completion gates

All final commands exited 0:

- `cargo test --quiet`: full workspace, including 1,070 scenarios.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`.
- `scripts/gen-card-checklist.ps1 --check`: regenerated `tricerules/CARDS.md`.
- `scripts/build-ninja.ps1`: full Windows build.
- Full CTest with `RULED_E2E_REQUIRE=1`: all 18 targets passed, including the new real server/sidecar token-copy flow.
- Changed C++ ranges pass `clang-format --dry-run --Werror`; `git diff --check` passes.

Focused red/green and final gate logs are under `build/verification-logs` with the `Issue-46` label.

**MTG applicability:** token creation/ownership/lifecycle (CR 111), Populate (701.36), copy values and entry (707), face-down characteristics, target revalidation, replacement ordering, and Flashback. Rule references were checked against the [official Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt). Double-faced token copies and copy-with-modifications remain the explicit #46 deferrals.
