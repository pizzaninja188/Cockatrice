# Colorstorm Stallion and double-faced token copies

Implementation of [issue #237](https://github.com/pizzaninja188/Cockatrice/issues/237),
including Transform and ModalDfc token copies. Colorstorm Stallion uses shared effects:
ward {1}, haste, and one Opus trigger that pumps its source and copies it when at least
five mana was actually spent on the triggering instant or sorcery.

## Rules interaction checklist

1. **Authority.** Oracle and rulings were fetched for Colorstorm Stallion, Reckless Waif,
   and Cackling Counterpart. The engine alone resolves legality, actual payment totals,
   copy values, token entry, and transformation. `TokenCopySource` distinguishes a chosen
   permanent (Cackling Counterpart) from the untargeted source (Stallion); Populate shares
   the same construction pipeline. Meld, merging, and new copy-with-modification mechanics
   are outside this change.
2. **Identity.** Copy snapshots own current copiable values, intrinsic faces, active face,
   and presentation provenance. Departing sources retain snapshots keyed by object ID and
   zone-change generation, including simultaneous departures and token cessation. A blinked
   object cannot substitute for the original source. Intrinsic token faces remain separate
   from later copy effects; single-faced Clone does not become double-faced. Transform keeps
   object identity, counters, and damage, refreshes static effects, and emits no entry event.
3. **Timing.** Opus resolves before its spell and survives that spell being countered.
   The five-mana threshold is an effect condition based on committed payment facts, including
   reductions and alternative costs. Copied values exclude the pump and counters. Existing
   entry replacements, suspended resolution, APNAP ordering, and SBAs are reused. Both faces
   freeze before an entry choice. Existing face-change generations reject stale transforms.
4. **Players and failure paths.** Token ownership follows the creating ability's controller,
   including three-seat games and a differently owned source. Source copying adds no target
   prompt or ward event. Chosen sources retain normal targeting and generation validation.
   Registry validation rejects source copies in spells and nonbattlefield activated abilities.
   Transformation cannot turn a single-faced token or expose a nonpermanent destination face.
5. **Visibility.** Retained copy snapshots stay internal. The optional battlefield
   `token_identity` contains only current public face values; face-down copies are anonymous.
   Full battlefield snapshots replace metadata; unchanged-battlefield batches retain it.
   No hidden-zone choice or private field is introduced.
6. **Propagation.** Rust publishes current token metadata through `TokenIdentity` in the
   battlefield snapshot. Servatrice refreshes name, base P/T, colors, keywords, and ability
   text on the existing physical token and sends a full-state display update when needed.
   Qt's existing `CardItem::processCardInfo` and `RuledTokenDisplay` consume that update.
   No additional Qt hook or command is needed. Physical IDs and token status are preserved;
   all new relay behavior stays within ruled synchronization. Freeform is unaffected.
7. **Verification.** Focused regressions cover payment boundaries, removed/blinked/copied
   sources, ceased tokens, replay, ownership, copied abilities, double-faced entry/Populate,
   transformation, modal faces, copy overlays, and entry suspension. Relay tests cover
   reconnect snapshot contents and same-name metadata replacement. A real Servatrice and
   sidecar with two protocol clients covers Stallion creation, Waif transformation, metadata,
   physical identity, and removal. The guest E2E harness cannot preserve a disconnected seat;
   reconnect is checked at the relay snapshot boundary and remains a manual acceptance step.
   Final gates use `scripts/verify.ps1 -Side Both -CardData` after pinned-input refresh.

Combat selection and unrelated hidden-zone choices are N/A. Existing haste and ward receive
card regressions. No dataset download, commit, push, or tracker mutation is part of this work.

## Manual acceptance (not performed)

Native desktop control is unavailable in this session. In two ruled dev clients, reach a
main phase and use the active player's dev console:

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

**MTG applicability:** source independence (113.7a), new-object identity (400.7), last-known
information (608.2h), copiable values (707.2), double-faced token copies (707.8a/707.10g),
transformation (701.27), mana value (202.3), and token ownership/lifecycle (111), checked against
the [official rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt).
Stallion behavior follows its fetched
[Oracle rulings](https://api.scryfall.com/cards/f5b54d46-2caf-4d1b-8be1-dbd9e9dce058/rulings).
