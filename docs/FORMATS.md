# Ruled formats and deck admission

Create Game requires Standard or Commander. Standard uses two seats and 20 starting life.
Commander supports two to four seats and 40 starting life. These are the fork's supported
game configurations; the format is immutable after creation and appears in game information.

Tricerules validates construction. Servatrice authenticates requests, normalizes native decks
and ordered one-copy sideboard moves, and relays private reports. Cockatrice presents results;
its display database never grants legality. Validation covers mainboard, sideboard and the
dedicated commander declarations, independently of the optional banner image.

Selection, readiness, sideboard changes and final start all enforce admission. A rejected
replacement preserves the previously accepted deck or sideboard plan. Format legality and
executable engine support are separate results, and both must pass. Unknown identities,
unrecognized construction choices, unavailable data and infrastructure failures cannot pass.
Card-specific errors go only to the submitting player, including when decklists are closed.
Ready decks in open lobbies are checked every minute so catalogue changes can clear readiness.

Both deck editors expose **Validate deck…**, also available in the Deck menu. Connect to a
matching server, choose the validation format, and press Validate. The dialog uses the same
server validator as game admission. Editing the deck or changing format invalidates its result;
a response for an older deck cannot be presented as current. Invalid drafts can still be saved.
Plain-text export includes Commander and Deck sections. Import also accepts `CM:` declarations,
keeps those cards outside the mainboard, and preserves separate native commander metadata.

## Catalogue source and updates

The sidecar downloads Scryfall's `oracle_cards` JSONL bulk source from the HTTPS location in
the [bulk-data API](https://api.scryfall.com/bulk-data). It reads format legalities, Oracle IDs,
all-face color identity, front characteristics and construction exceptions. It derives typed
partner and copy-limit metadata during ingestion. This catalogue is separate from the authored
RON definitions: format updates do not add executable card behavior or edit authoring datasets.

An update runs on startup and every six hours. A complete candidate is parsed and hashed before
atomic cache promotion. Failed downloads or malformed candidates retain the last good cache.
Reports identify the source date and SHA-256; a last successful check older than 48 hours adds
a visible notice. With no usable cache, legality is Unknown until an update succeeds.

The default Windows cache is `%LOCALAPPDATA%/Cockatrice/tricerules/format-data`.
`TRICERULES_FORMAT_DATA_DIR` overrides it. The development launcher uses `build/format-data`.
On its first run the launcher warms that cache before automatically selecting decks.
Run `tricerules-server --refresh-formats` for an immediate administrator refresh; running
sidecars pick up the promoted manifest on subsequent validation queries. Setting
`TRICERULES_FORMAT_UPDATES=0` disables automatic network updates and keeps cached validation.
New bans, unbans and rotations take effect after Scryfall publishes an updated bulk snapshot
and the next successful refresh, without rebuilding.

Accepted games record the chosen format and catalogue provenance. Catalogue updates do not
change active engine state. Diagnostic replay reconstructs accepted settings from its recorded
startup, independent of current bans. The local capture launcher explicitly enables recorded
admission on its isolated servers and retains the verified resume plan's format and roster.
Network clients cannot request that mode. Historical 40-card test fixtures separately require
explicit server-side test and dev gates; selecting a real format always enforces construction.

## Commander behavior and scope

The current [Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf)
(effective September 25, 2026) govern construction and gameplay: CR 903.3–903.5, 702.124,
903.6–903.10 and 704.6c. Standard construction follows CR 100.2a and the 15-card sideboard limit in CR 100.4a.
The implementation checks deck size, copy limits and exceptions, bans, commander eligibility,
permitted pairs, color identity and basic land types. Unresolved eligibility characteristics or specialized pregame identity choices
that cannot yet be verified return Unknown. Commander Draft and companion declarations are
outside this release; an ordinary Commander sideboard is refused.

Commanders start in the actual command zone. Casting uses the existing staged payment flow,
with a separate committed-cast count for each physical commander and its additional generic
tax. Cancellation restores the same physical card and position without incrementing that count.
Combat damage is tracked independently for each commander and victim across zone/control
changes; 21 damage from one commander causes loss during state-based actions. Existing return
to command-zone choices continue to use engine-owned decisions.

`RuledAuxiliaryArea` arranges independent graphics sections, compared with the existing sidebar
and CardZone views. Its first section contains actual commander CardItems, at the local
battlefield's bottom right and each opponent's top right. A reserved column prevents permanent
overlap; empty sections collapse. Kami of the Crescent Moon and Atraxa, Praetors' Voice use the
card section. A future non-card section can present, for example, Teferi, Hero of Dominaria's
emblem or Chandra, Torch of Defiance's emblem without pretending those objects are cards or
battlefield permanents. Moving existing emblems/static-effect displays is deferred.

## User GUI acceptance — pending

Automated tests cover construction, source integrity, format-derived setup, private admission,
sideboarding, commander payments/identity, combat loss and floating-area layout. They do not
establish interactive GUI acceptance. The user elected to test after implementation.

1. Run `scripts/launch-ruled-game.ps1 -Dev` in the interactive user session. The default Standard
   samples are 60 basic lands. Verify 20 life, the format label and successful readiness.
2. In Create Game, switch between formats. Standard fixes two seats/20 life; Commander permits
   two to four seats/40 life and defaults to four. Reopen with remembered settings.
3. In each editor, add/move one copy into Commander, undo/redo, save and reopen. The banner
   remains independent. Export/re-import plain text and check the Commander section. Validate
   a valid sample, an illegal copy count and an unsupported card.
   Edit while a response is pending and verify that the old result is marked out of date.
4. Select an illegal deck with closed decklists. Only its owner receives card details; the
   previous accepted deck remains loaded. Repeat with an invalid sideboard move, Ready and
   Force Start. No invalid configuration may start.
5. Run `scripts/launch-ruled-game.ps1 -Format Commander -Players 2 -Dev` (stop the previous run
   first). The sample declares Kami with 99 Islands. Verify 40 life and one real commander in
   each floating area. Repeat with three/four seats, narrow windows and reconnect/spectator views.
6. Cast, cancel, recast and return a commander to the command zone. Verify the exact card moves
   once, payment tax increases only after committed casts, and opponents cannot cast it. Check
   commander combat damage/loss, and that noncombat damage does not count toward 21.
