# Issue Tracker

> **Status (2026-07-23):** active tracker, moved from repo root to `docs/`; `scripts/auto-fix-issues.sh` paths updated to match.

This file is **your input** to the automated fixer. You own it — edit it (ideally
on your Windows machine) and push. The automation reads it but never writes it;
it records progress in `AUTOMATION_STATUS.md` instead.

## How to use
- Add issues under **Open**, each with a unique short ID (`#1`, `#2`, …). Don't
  reuse IDs.
- Give each a `Priority:` (High / Medium / Low) — the automation works High first.
- Use labels in brackets: `[bug]`, `[feature]`, `[chore]`, `[docs]`.
- Delete completed issues from this file rather than marking them checked. For work
  committed directly to `master`, remove the issue in the implementation commit; for a
  `fix/issue-N` branch, remove it immediately after merge. Before reporting completion,
  reconcile dependency wording in the remaining issues. Historical status stays in
  `AUTOMATION_STATUS.md`.
- Workflow: you add issues here → the box (cron) fixes them on `fix/issue-N`
  branches and pushes them → you pull, UI-test, and merge to `master`. Status and
  per-branch manual UI test steps live in `AUTOMATION_STATUS.md` / the branch
  commit message.

---

## Open

- [ ] #130 [feature] Author the next 20 fully supported calibration creatures
  - Details: Add the first 20 `simple_triggered_creatures` rows from the 2026-08-18 calibration in selection-hash order: Watcher of the Wayside, Sanguine Syphoner, Burglar Rat, Flesh Burrower, Prideful Parent, Apothecary Stomper, Elfsworn Giant, Helpful Hunter, Kin-Tree Nurturer, Dusyut Earthcarver, Sandskitter Outrider, Beast-Kin Ranger, Dwynen's Elite, Humbling Elder, Reputable Merchant, Delta Bloodflies, Iceridge Serpent, Felidar Savior, Infestation Sage, and Summit Intimidator. Use only current RON/token vocabulary, add focused scenario coverage for distinct trigger/target/choice shapes, regenerate `tricerules/CARDS.md`, and run the Rust/card-data gates. This is Rust/card-data only; protobuf, relay, Qt, and manual two-client testing are N/A.
  - Priority: High

- [ ] #96 [feature] Surveil and ordered top-library partition choices
  - Details: Add one private, resumable library-partition primitive for surveil (CR 701.25) and bounded variants that put chosen cards into the graveyard while preserving the ordered top remainder. Evidence: Wary Thespian, Wary Watchdog, Appendage Amalgam, Fear of Surveillance, Cruel Truths, and Gutless Plunderer. The engine publishes the looked-at cards and legal destinations only to the choosing player, logs ordering choices, fires surveil only after the complete action, and resumes later effects in order; Servatrice must redact the cohort from other seats and the ruled client must reuse the image picker. Include two-client hidden-information acceptance.
  - Priority: High

- [ ] #97 [feature] Predicate-driven battlefield-entry replacements
  - Details: Generalize the CR 614 battlefield-entry pipeline beyond unconditional self-entry state. Support self entry tapped when a public player-set predicate holds (Razortrap Gorge and the six sibling lands) and battlefield-source replacement effects that add counters to matching entrants (Dragonstorm Globe; also Hardened Scales-style reuse). Conditions and affected cohorts must use current derived state, enter through the shared CR 616 choice coordinator, and apply before ETB triggers without emitting later tap/counter actions. Engine events remain authoritative; add replacement-ordering and multiplayer-relative scenario coverage. No client change unless a replacement choice is exposed.
  - Priority: High

- [ ] #98 [feature] Face-down permanents and manifest dread
  - Details: Implement face-down battlefield object state, private face identity, manifest/turn-face-up special actions (CR 701.40), and the manifest-dread two-card choice (CR 701.62). Evidence: Bashful Beastie, Innocuous Rat, Manifest Dread, Twist Reality, Turn Inside Out, and Unable to Scream. Preserve the physical card and engine `ObjectId` across face changes, publish only engine-authored public 2/2 characteristics, reveal candidates only to the controller, validate creature mana costs when turning face up, and keep choices replay-deterministic. This requires protobuf, redaction, physical-card mapping, ruled-client actions/display, headless/E2E coverage, and real two-client hidden-information acceptance.
  - Priority: High

- [ ] #99 [feature] Room permanents, locked doors, and unlock events
  - Details: Add shared-type-line split permanent support and the locked/unlocked designations and special action from CR 709.5. Evidence: Ticket Booth // Tunnel of Hate, Derelict Attic // Widow's Walk, Glassworks // Shattered Yard, Rampaging Soulrager, and the eerie cards Erratic Apparition and Cult Healer. The engine owns which doors exist and are unlocked, legal unlock actions, per-door characteristics, copied values, and `fully unlocked` events; Servatrice binds one physical card; the client renders authoritative door state and submits published actions without parsing Oracle text. Cover cast-face entry, noncast entry, unlock timing/costs, copies, zone resets, triggers, and two-client visible acceptance.
  - Priority: Medium

- [ ] #100 [feature] Omen card faces and shuffle-on-resolution lifecycle
  - Details: Add CR 720 Omen cards as one physical card with a normal permanent face and alternate instant/sorcery spell characteristics, distinct from Adventure and ordinary split layouts. Evidence: Sagu Wildling // Roost Seek, Stormshriek Feral // Flush Out, Riling Dawnbreaker // Signaling Roar, and Dirgur Island Dragon // Skimming Strike. Publish both legal cast faces, use the chosen face on the stack, and replace the Omen spell's graveyard destination with a deterministic shuffle into its owner's library on every successful resolution. Preserve card catalog/face display identity through protobuf, relay, client selection, stack display, and physical library reconciliation; add Rust, client, relay, E2E, and two-client acceptance.
  - Priority: Medium

- [ ] #101 [feature] Activated abilities outside the battlefield
  - Details: Generalize activated-ability legality to an authored source zone, preserving zone-change generation and publishing stable engine-owned actions for non-battlefield cards. First consumers: typecycling from hand on Shepherding Spirits, Slavering Branchsnapper, and Daggermaw Megalodon (CR 702.29), plus renew from graveyard on Adorned Crocodile, Sagu Pummeler, and Champion of Dusan. Pay discard/exile-self costs atomically, search by land subtype for typecycling, remove stale actions after any zone change, and never expose concealed-zone actions to other seats. Extend structured legal actions, relay redaction, and ruled-client click/prompt flows; require two-client hidden-zone acceptance.
  - Priority: High

- [ ] #102 [feature] Per-object activation frequency limits
  - Details: Add reusable activation limits such as `once each turn`, keyed to full object identity and reset at the correct turn boundary. Evidence: Temur Devotee, Sultai Devotee, Mardu Devotee, Jeskai Devotee, and Abzan Devotee; reuse also covers once-per-combat or once-per-game restrictions when their reset scopes are parameterized. Legality must disappear immediately after the accepted activation, survive control changes only as the rules require, reject stale commands, and remain deterministic. This is engine/card-data only unless the existing legal-action refresh proves insufficient.
  - Priority: Medium

- [ ] #103 [feature] Ward costs and counter-unless-paid triggers
  - Details: Implement Ward (CR 702.21) as a triggered ability generated when a protected permanent becomes the target of an opponent-controlled spell or ability. Support mana and discard costs using the existing resolution-cost choice channel; evidence: Cackling Prowler, Trapped in the Screen, Spectral Snatcher, and Dirgur Island Dragon. The engine must bind the exact triggering stack object, counter it only if still present and unpaid, handle multiple Ward instances/APNAP order, and publish private discard candidates only to the payer. Reuse the current prompt infrastructure and add Rust, relay/client, E2E, and two-client payment/decline acceptance.
  - Priority: Medium

- [ ] #104 [feature] Optional cast-time payments with linked resolution state
  - Details: Extend `LegalCostChoices` so a spell can offer optional additional payments/actions and carry typed receipts onto its stack item under CR 601.2 and 607. Evidence: kicker on Grow from the Ashes and Gnarlid Colony, and behold (CR 701.4) on Caustic Exhale, Osseous Exhale, Molten Exhale, and Dispelling Exhale. Support mana, choosing a qualifying public permanent, or revealing a qualifying hand card; reveal only what Oracle requires, reject stale candidates, and let effects/targets/entry replacements query the recorded choice without client inference. Add protocol/relay/client cost staging, redaction tests, E2E, and two-client acceptance.
  - Priority: Medium

- [ ] #105 [feature] Harmonize alternative graveyard casting
  - Details: Implement harmonize (CR 702.180) using generic alternative-zone casting and cost-reduction substrate, with Unending Whisper and Mammoth Bellow as the two calibration cards. Publish the graveyard cast action only to the owner, optionally choose and tap one untapped controlled creature during cost determination, reduce only generic mana by its current power, and replace every leave-stack destination with exile after the harmonize cost was paid. Preserve graveyard physical identity, stale-choice rejection, replay determinism, and rules-authoritative payment; add protocol, relay/client, E2E, and two-client hidden-zone acceptance.
  - Priority: Low

- [ ] #106 [feature] Mobilize tapped-and-attacking token cohorts
  - Details: Implement mobilize (CR 702.181) for Reigning Victor, Nightblade Brigade, Dragonback Lancer, and Shock Brigade. On the source's attack event, create the exact Warrior cohort tapped and attacking, let the controller choose each token's defending player/planeswalker/battle from engine-published candidates, do not fire declared-attacker triggers for those tokens, and create one delayed next-end-step sacrifice tied to their identities. Extend token events/combat maps and client choices without changing freeform; add multiplayer-generic Rust coverage, relay/client/E2E coverage, and real two-client combat acceptance. This depends on #72 for nonplayer attack recipients but must work for player defenders first.
  - Priority: Medium

- [ ] #107 [feature] General graveyard target actions and result cohorts
  - Details: Extend the zone-aware graveyard vocabulary beyond return-to-hand/battlefield. Support targeted exile (Ambush Wolf, Arashin Sunshield), movement to an owner's library top/bottom (Malevolent Chandelier, Jade-Cast Sentinel, Monastery Messenger), grouped targets constrained to one graveyard (Soul-Shackled Zombie), and typed result cohorts for immediately following conditions. Preserve owner, generation, destination ordering, and public graveyard visibility; physical moves and object maps must stay synchronized. Add Rust scenarios plus relay/E2E coverage for battlefield-bound abilities that move physical cards.
  - Priority: Medium

- [ ] #108 [feature] Graveyard aggregate conditions for threshold and delirium
  - Details: Extend `GameCondition` with controller-relative graveyard aggregates: total card count for threshold (Crypt Feaster) and distinct card-type count for delirium (Spineseeker Centipede, Hand That Feeds, Impossible Inferno). Use the existing layout-aware outside-stack face selection, current Oracle card types from the rules registry, and pure reevaluation suitable for triggers, static abilities, costs, and conditional effects. Hidden card identities remain private while the aggregate result is public. Rust/card-data only; no protocol or manual UI gate.
  - Priority: Medium

- [ ] #109 [feature] Generation-aware linked exile until a source leaves
  - Details: Implement CR 607 linked temporary exile for Banishing Light, Stormplain Detainment, and Trapped in the Screen. Record the exact exiled object/new-zone generation, return only cards exiled by that specific source, handle source removal before/after the ETB trigger resolves, multiple sources, tokens, owners, and leaves/re-enters as a new object. The engine owns the link; Servatrice mirrors battlefield↔exile physical movement and redacts nothing beyond normal zone rules. Add Rust, relay, E2E, and two-client physical-identity acceptance.
  - Priority: Medium

- [ ] #110 [feature] Rich library search filters and placement choices
  - Details: Generalize private library selection to support exact names (Tempest Hawk), card power predicates (Living Phone), disjunctive types and named cross-zone search (Say Its Name), conditional destinations (Embermouth Sentinel), and an object's owner choosing top versus bottom (Uncharted Voyage, Riverwalk Technique). Keep candidate publication engine-authored, preserve ordered top/bottom semantics, distinguish searches from looks, and resume effects only after logged choices complete. Reuse the image picker and fail-closed redaction; require two-client hidden-library acceptance and physical deck-order checks.
  - Priority: Low

- [ ] #111 [feature] Turn-history ordinals and attack facts
  - Details: Extend `TurnHistory` rather than adding card-local state. Record per-player spell and card-draw ordinals plus whether a player attacked this turn, enabling Erudite Wizard, Poised Practitioner, Jeskai Devotee, Highspire Bell-Ringer, Focus the Mind, and Gutless Plunderer. Triggers must fire at the event edge exactly once, cost conditions read committed history during CR 601.2f, copied spells do not count as casts, and cleanup rolls all facts deterministically. Rust/card-data only unless card-draw events need a public annotation.
  - Priority: Medium

- [ ] #112 [feature] Reusable spell-cost reductions over targets and battlefield cohorts
  - Details: Widen total-cost calculation beyond face-local fixed conditions. Support target-dependent reductions (Luminous Rebuke, Seized from Slumber), battlefield-source reductions for matching spells (Mocking Sprite, Highspire Bell-Ringer), and counted affinity-style reductions (Salt Road Packbeast), all under CR 601.2f. Publish typed reduction applications, lock targets and costs in the correct order, floor generic mana at zero, and keep the client staging only engine-authored payable totals. Add Rust, protocol/client payment-contract, E2E, and two-client acceptance where the quoted cost changes visibly.
  - Priority: Medium

- [ ] #113 [feature] Characteristic setting and ability removal layers
  - Details: Fill the existing CR 613 pipeline slots needed by Witness Protection and Unable to Scream: layer-3 text/ability removal as applicable, layer-4 type replacement/addition, layer-5 color setting, layer-6 ability removal, layer-7b base P/T setting, and name changes in the copiable/text characteristics model. Preserve timestamps, copy effects, attachment lifecycle, and continuous reevaluation; implement the CR 613.8 dependencies these cards actually expose through the roadmap's existing insertion point rather than bypassing them. Never mutate base card data. Publish resulting engine characteristics generically to clients and add layer-ordering, dependency, copy, and attachment regressions.
  - Priority: Low

- [ ] #114 [feature] Disjunctive target and card filters
  - Details: Add an explicit composable any-of filter node while retaining current leaf predicates and validation. Evidence: Make Your Move (artifact OR enchantment OR creature with power 4+) and Broken Wings (artifact OR enchantment OR creature with flying), with Say Its Name and Monastery Messenger exercising graveyard card-type disjunction/negation. Candidate publication and resolution revalidation must use the same engine predicate; reject empty, redundant, or contradictory branches. Rust/card-data only because clients already consume published candidates.
  - Priority: Medium

- [ ] #115 [feature] Attachment event observers and conditioned attached modifiers
  - Details: Extend attachment-aware vocabulary for Goldvein Pick (attached creature deals combat damage to a player), Cracked Skull (attached creature is dealt damage), and Quick-Draw Katana (attached modifier only during the Equipment controller's turn). Capture the event-time attached object with generation-aware identity and LKI, attribute triggers to the attachment controller, and continuously reevaluate modifier conditions without snapshotting the recipient. Rust/card-data only unless existing attachment annotations prove insufficient.
  - Priority: Low

- [ ] #116 [feature] Condition-gated effect branches and creature scopes
  - Details: Reuse `GameCondition` across two missing consumers: continuously conditioned cohort modifiers (Inspiring Paladin) and automatic resolution branches selected by live state (Trade Route Envoy; also Embermouth Sentinel once #110 lands). Do not expose player-selectable modes when Oracle specifies a condition. Static scopes reevaluate in their proper layer; resolution branches snapshot only at their instruction; reject power-dependent layer conditions until CR 613.8 support is actually required. Rust/card-data only.
  - Priority: Low

- [ ] #117 [feature] Fight as reciprocal creature damage
  - Details: Add the CR 701.14 fight action for Bushwhack and Prey Upon, distinct from one-way power-based damage. Both creatures deal noncombat damage equal to current power simultaneously; if either target is illegal at resolution, neither fights; prevention, deathtouch, lifelink, and post-event SBAs use the ordinary damage path. Reuse two distinct engine-published target groups. Rust/card-data only.
  - Priority: Medium

- [ ] #118 [feature] Damage prevention scoped to one chosen creature
  - Details: Generalize prevention applications so Fleeting Flight and Sheltering Light-style effects can prevent all combat damage that would be dealt to one generation-aware chosen creature for the turn, without setting the global Fog flag. Compose with source/recipient filters, multiple prevention effects, CR 616 ordering, leaving/re-entering, and damage that is not combat damage. Rust/card-data only.
  - Priority: Low

- [ ] #119 [feature] Self-scoped continuous combat prohibitions
  - Details: Add a source-scoped static combat restriction for Vampire Soulcaller and Goblin Brigand-style cards that continuously prohibit the source from blocking or attacking as authored. Route it through shared `can_attack`/`can_block`, derived characteristics, copy effects, and control changes; do not add face booleans for every future restriction. Rust/card-data only.
  - Priority: Low

- [ ] #120 [feature] Source-relative activated costs and zone effects
  - Details: Complete activated-ability source semantics needed by Hungry Ghoul and Unburied Earthcarver (`sacrifice another creature`, excluding the source) and Wingspan Stride (return this Aura to its owner's hand). Costs must validate all components before payment, bind `Source` without targeting, and preserve CR 400.7 identity when the source changes zone. Parameterize source exclusion/selection rather than card-specific variants. Rust/card-data only.
  - Priority: Medium

- [ ] #121 [feature] Untargeted player-set draw and discard effects
  - Details: Extend recipient-aware zone effects for Friendly Teddy (`each player draws`) and Fanatic of the Harrowing (`each player discards`). Use `RelativePlayerSet`/`PlayerRecipient` rather than synthesizing targets; collect simultaneous choices in APNAP order without leaking hands, then apply the complete action before following conditions. This requires engine resolution choices, per-player redaction, client prompts, E2E, and two-client hidden-hand acceptance.
  - Priority: Low

- [ ] #122 [feature] Typed result predicates for paid and moved cards
  - Details: Carry exact engine-owned result cohorts between immediately adjacent instructions so later effects can ask about the cards actually discarded, exiled, or otherwise paid. Evidence: Grab the Prize checks whether its additional-cost card was nonland, Soul-Shackled Zombie checks whether a creature card was exiled this way, and Fanatic of the Harrowing checks whether its controller discarded. Preserve hidden identities, expose only the resulting public branch, and avoid querying a zone after intervening moves. Reuse typed effect/payment results; add hidden-information and stale-object scenarios.
  - Priority: Low

- [ ] #123 [feature] Exile-and-play permissions with explicit expiry
  - Details: Implement impulse-play effects for Clockwork Percussionist and Impossible Inferno. Exile the exact top card, grant its owner/controller engine-authored permission to play land or cast the appropriate face through the stated turn boundary, enforce normal timing/costs, and expire the permission deterministically even if control changes. Servatrice must preserve physical exile identity; clients consume published legal actions without learning concealed cards early. Add Rust, relay/client, E2E, and two-client acceptance.
  - Priority: Medium

- [ ] #124 [feature] Extensible permanent counter kinds and stun replacement
  - Details: Generalize counters beyond +1/+1 and -1/-1 so Acrobatic Cheerleader can place a flying counter and Fear of Immobility can place a stun counter. Keyword counters feed layer 6 while present; stun counters create the single CR 122.1d untap replacement that removes one counter instead. Keep named counter storage/annotations generic, handle copy and zone changes correctly, and use the CR 616 replacement-choice path where effects overlap. Add Rust scenarios and client annotation coverage; manual two-client testing is recommended for visible counter state.
  - Priority: Low

- [ ] #125 [feature] Exile-instead-of-dying replacement from marked damage
  - Details: Add a duration-bound replacement application for Scorching Dragonfire and Lava Coil: if the damaged creature or planeswalker would die that turn, exile it instead. Bind the affected object's generation, apply at the actual battlefield-to-graveyard event (including later lethal damage/SBAs), compose with regeneration and other replacements under CR 616, and expire at cleanup. Rust/card-data only until #72 adds planeswalker objects.
  - Priority: Low

- [ ] #126 [feature] Controller-chosen exile from an opponent's hand
  - Details: Extend the private opponent-hand choice path used by Coercion/Thoughtseize so Aggressive Negotiations and similar effects can exile the selected card rather than discard it, then resume later targeted effects. Only the effect controller receives eligible nonland card identities; the hand owner and other seats receive redacted counts/public move results. Preserve physical `Server_Card.id`, reject stale selections, and add Rust, relay/client, E2E, and real two-client hidden-hand acceptance.
  - Priority: Low

- [ ] #127 [feature] Characteristic filters on observed trigger objects
  - Details: Widen permanent-entry event filters beyond card type/controller to current derived characteristics such as power. Evidence: Vicious Clown watches another controlled creature with power 2 or less, while Mentor of the Meek supplies a second power-filtered ETB use with a different effect. Evaluate the entrant after entry replacements and continuous effects at the trigger event, preserve source exclusion and token handling, and avoid retroactive triggers after later characteristic changes. Rust/card-data only.
  - Priority: Low

- [ ] #128 [feature] Beginning-of-combat and second-main-phase trigger events
  - Details: Publish generic phase/step event boundaries for Riling Dawnbreaker's beginning-of-combat trigger and Acrobatic Cheerleader's survival trigger at the beginning of the controller's second main phase. Support intervening-if source-tapped checks and an object-scoped once-only trigger guard without assuming two players. Collect/APNAP-order triggers before priority, use the existing typed `PhaseId`, and add turn-flow scenarios. Rust/card-data only; no UI change because phases are already published.
  - Priority: Low

- [ ] #129 [feature] Restricted mana for ruled special actions
  - Details: Extend mana spending-purpose identity beyond casting spells and activating abilities so Creeping Peeper-style mana can be spent on engine-published Room unlock and turn-face-up special actions but not unrelated costs. Keep restrictions on individual mana contributions, quote payable pools engine-side, and make the client stage only authoritative payment options. Evidence also includes mana restricted to foretell/morph-style special actions. Depends on #98 and #99; requires Rust plus protocol/client payment tests and two-client acceptance once either consumer ships.
  - Priority: Low

- [ ] #72 [feature] Planeswalker and battle objects as rules targets
  - Details: Add the missing nonplayer object kinds and legal target vocabulary so damage and destroy effects can include planeswalkers and battles where Oracle permits. Calibration evidence: Chandra's Magmutt, Finishing Blow, Goblin Arsonist, Pitchburn Devils, Scorch Spitter, Shock, Sorcerer of the Fang, and Viashino Pyromancer. This requires engine-owned card types, battlefield identity, damage/defeat state-based actions, proto/relay/client representation, and end-to-end target selection; do not approximate these objects as players.
  - Priority: Low

- [ ] #46 [feature] Token copy effects — Populate and create-a-copy tokens
  - Details: Build on the shipped permanent copy-layer snapshot, effective-face accessors, and battlefield display identity from #45. Bridge the token lifecycle and `CreateTokens` support to that snapshot. Support both targeted “create a token that's a copy of target permanent” effects and untargeted Populate-style choices without conflating the latter with CR 115 targeting; reuse one snapshot/minting helper and keep player sets generic. A copied token must receive the chosen permanent's copiable values, including existing copy effects, while excluding counters, damage, attachments, and non-copy continuous effects; copying an inline token must not require a registry `CardId`. Reuse the shipped effective battlefield display identity through proto, relay, and ruled client paths. Scenario coverage: copy a registry-backed permanent; copy an inline token; copy an already-copied permanent; prove counters and temporary pumps are excluded; reject illegal/stale targets or choices; verify token ownership, controller, zone changes, and cease-to-exist behavior. Add conformance and end-to-end display coverage. Double-faced tokens and copy-with-modifications remain deferred.
  - Priority: Low

---

## Backlog (not yet prioritized)

- Own version of MTGO's FACE card rendering
- Targeting visuals overhaul
