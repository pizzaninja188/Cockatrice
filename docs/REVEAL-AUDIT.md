# Public reveal presentation

Every public reveal in ruled play has a card window as well as its game-log presentation.
An instantaneous reveal opens a dismissible snapshot. A public resolution choice uses that same
window as its picker, with only the deciding player able to select cards. Completing the choice
removes selection authority and makes the snapshot dismissible; it does not erase the cards.
Cost/activation reveals remain active for their existing engine-defined duration, then become
dismissible snapshots too. Closing a completed window is local and never delays the game.

## Producer audit

The audit covered engine reveal events, public choice metadata, active stack reveal snapshots,
explicit reveal log messages, custom effects, and authored reveal flags. Private look/search
choices and ordinary public-zone movement were checked separately: neither implicitly authorizes
a new public reveal.

| Producer | Previous gap | Shared path |
|---|---|---|
| Behold from hand (Elven Passage and the other Passages) | Ordinary reveal popup, separate from ruled choices | `reveal_cards` before resolution continues. Choosing a battlefield permanent is not a hidden-card reveal. |
| Explore (Map) | Lands only logged; nonlands requested a public window and a second controller picker | Land emits an instant snapshot before the move; nonland attaches the same snapshot type to its public choice. |
| Reveal top card, conditionally take it (Esper Origins) | The ordinary client suppressed the popup for a single library-top reveal | Shared instant snapshot, independent of the library pile and subsequent movement. |
| Search with `reveal: true` (Mystical Tutor, Bushwhack, landcycling and other authored searches) | Selected cards could be logged without a reveal event | Snapshot selected cards before movement/shuffling, grouped by source owner and zone. Declining/failing to find creates no empty popup. |
| Look, choose, reveal, bottom (Commune with Nature) | Chosen card was only logged | Only the selected card gets a public snapshot; the initial look and remaining-card ordering stay private. |
| Public hand selection (Coercion, Thoughtseize, Dai Li Indoctrination, Aggressive Negotiations, Temporal Intervention) | Choice-bound popup disappeared immediately; an ineligible hand could bypass the reveal | Shared public choice, or an instant hand snapshot when no selection is possible. Private-look variants stay private. |
| Gifts Ungiven | Its revealed split relied on `ChoiceKind::Revealed`, without an explicit public reveal snapshot | Custom interrupt explicitly declares public visibility. Both seats receive the revealed group; only the opponent chooses. The existing no-single-opponent fallback also emits an instant reveal. |
| Cast-cost reveals and Ninjutsu | Separate aggregated hand window, closed when the source left the stack | Shared active snapshots keyed by stack occurrence and cost group/activation. Intermediate reveals survive automatic batch settlement. |

No card data, search legality, targeting, costs, counter placement, or zone-change rules are changed.
The pre-existing Gifts Ungiven targeting simplification is not expanded by this presentation change.

## Shared contract and extension recipe

`CardsRevealed` is the single public payload: occurrence ID, source effect identity/description,
zone owner, source zone, and immutable card identities (ObjectId, zone-change generation,
definition ID and display name). It is used directly for an instant event, nested in
`ResolutionChoiceRequired.public_reveal`, or in `ActivePublicRevealSnapshot`.

1. An instant reveal calls `engine::reveals::reveal_cards` **before** moving/reordering its cards.
   The helper supports multiple source-owner/zone groups in deterministic encounter order.
2. A public choice over one source cohort uses `reveal_choice`; a custom resolution interrupt
   sets `public_reveal`. The engine's existing candidate list, eligibility and deciding player
   still govern selection. Private looks leave public metadata absent.
3. Active-duration reveal producers publish the same payload with a stable source-occurrence
   ID through the active snapshot. New durations belong in the engine, not in the widget.
4. Instant/choice occurrence IDs are assigned once at the accepted command boundary, after
   automatic settlement. Repeated publication retains the ID; revealing the same physical card
   again gets another ID. Intermediate active snapshots retain their occurrences when replaced.
5. Servatrice routes public snapshot data without trying to recover it from the card's later
   physical zone. Choice click IDs remain temporary and recipient-appropriate. The old ordinary
   `Event_RevealCards` projection is removed from ruled reveals, so it cannot create duplicates
   or invoke the library-top popup exception.
6. `RuledRevealState` owns occurrence history, active-choice association and local dismissal.
   `RuledRevealWindows` is the only public window owner. New reveal mechanics need no Qt popup
   branch. A window title identifies the revealing zone's owner, zone and source effect.

Completed cards use inert synthetic display IDs. Only the current public choice window is tagged
with its occurrence ID and may map a click to an engine candidate. A later choice cannot reuse
an older window's transient candidate indices. Historical snapshots never follow a card into
another hidden zone. The existing replay-seek suppression flag skips historical popup creation;
normal playback restores still-active reveals. Reconnect restores active public state, not a
newly reconstructed history of completed reveals.

## Rules interaction checklist

1. **Authority:** presentation follows the engine's explicit public/private declaration.
   Governing concepts are reveal duration and repeated reveals (CR 701.20a–d), private looking
   (701.20e), explore (701.44), search (701.23), and resolution choices (608.2d).
   Checked against the [official rules effective August 7, 2026](https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt).
   Retained completed windows are historical records, not continued rules permission to inspect
   a live card. New mechanic/card legality and Oracle changes: N/A.
2. **Ownership/identity:** tricerules is the sole state writer. Snapshot card IDs/generations are
   distinct from occurrence IDs and transient picker IDs. Movement/shuffle cannot substitute a
   different card image. Window dismissal changes only local presentation, not the command log.
   Attachments, counters, face transformation, and permissions are unchanged: N/A to this change.
3. **Timing:** existing park/resume and logged choice submission remain authoritative. No extra
   priority pass, acknowledgment or resolution pause is introduced. Preserve reveals through
   same-command movement and automatic settlement. Trigger ordering, replacement ordering,
   layers, dependencies and combat mechanics are unchanged: N/A.
4. **Players/failures:** owner and decider remain distinct, including Gifts and opponent-hand
   choices. Helpers group by actual owners without two-seat arithmetic. Empty groups produce no
   popup. Malformed public choice arrays and reused occurrence identities fail closed. Existing
   engine stale-generation/illegal-selection checks continue to apply.
5. **Visibility:** snapshot fields are public by explicit engine authorization; eligibility stays
   decider-only. Private look/search metadata never creates a public snapshot. Both seats receive
   the same revealed identities without receiving other library/hand cards. Completed windows
   cannot expose future hidden movements. Active state can be restored on reconnect.
6. **Propagation:** Rust producers and command finalization, shared protobuf, Servatrice routing,
   Qt dispatcher/state, the single window owner, and actual click routing are included. Freeform
   reveal processing is unchanged. Upstream hooks remain short; new UI logic is fork-owned.
7. **Verification:** red/green regressions cover the missing land reveal, mechanic-independent
   public picking, and active reveal loss during settlement. Scenarios cover explore, searches,
   private-look filtering, Gifts and cast/activation reveals. Client/relay tests cover retention,
   dismissal, repetition, public/private metadata and replay behavior. The two-seat E2E covers
   nonland and land explore, active reveals, and library reveal/movement. Final gate:
   `scripts/verify.ps1 -Side Both`. No RON/generated-data refresh is needed; no delivery mutation
   is implied. GUI acceptance is separate from these automated checks.

## Manual acceptance (deferred by the user; not performed by the agent)

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
