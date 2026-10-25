# Composable card execution and authoring contract

Status (2026-10-24): the `DrawDiscard` compatibility family and flat `ChooseResolutionBranch`
grammar/runtime are retired. After the Trade Route Envoy port, flat-variant validation/traversal,
selection/dispatch, and expansion paths were removed. The shared internal
`EffectOutcome::RestartResolutionBranch` remains for MayBehold's no-candidate path; typed Program
`Choose` and shared cost, requirement, and branch-selection helpers remain. The broader composable-card execution
migration remains incomplete: receipt composition and library search/selection transactions remain open.
The target design also established cross-component resolution-prompt identity, exact compiled owner
identities, and immutable ability binding. Supported finite profiles, accepted syntax, and
restrictions are maintained in the
[active authoring reference](../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md#finite-authored-programs).
The original migration census was 41 existing card
identities, 42 `DrawDiscard` bodies and three additional modal siblings: 45 compiled owners, not 45
card admissions. All 45 owners now use authored programs, and the migration added no card
admissions. The `DrawDiscard` variant, source lowerer, and draw-after continuation are removed; registry
admission rejects old `DrawDiscard` RON and requires every sibling to be authored when a modal
container uses programs. Typed discard outcomes and replacement/entry/payment continuations retain
the owning frame. Authored-owner conversions include Calamitous Tide's face, Agna Qel'a's
activation, Uthros Scanship's ETB, all three Three Steps Ahead modes, and the exact Recruit
generator pair for Long Lake Nuisance and Patient Instructor. Broader operations and contexts remain
migration targets.
Resolution prompt IDs are scoped to the deciding player for both legacy and Program choices; answer
validation checks the actor and that actor's prompt identity. This does not widen accepted syntax.

Card-specific update (2026-10-21): The Final Days and Sold Out now use typed spell-face Programs.
Sold Out uses the narrow Subject-bound singleton-Creature Exile and target-damage-history profile;
its [review map](../tricerules/tricerules-cards/authoring/review-maps/sold_out.json) records the
source-line mapping and semantic evidence for that owner. Trade Route Envoy's enter trigger now uses
a mandatory controller `Choose` with a live countered-creature draw branch and a +1/+1-counter
`Otherwise`; its [review map](../tricerules/tricerules-cards/authoring/review-maps/trade_route_envoy.json)
records the full printed line and semantic fixtures. The narrow compiler admission leaves generic
Program `If` unchanged; the Envoy definition port reused existing production core operations and
added no protocol/client changes. The subsequent flat-branch retirement removed engine model/runtime paths without changing
the protobuf, relay, or client contract. Trade Route Envoy
was outside the original `DrawDiscard` migration census. Following this port, the flat
`ChooseResolutionBranch` schema/runtime was retired; typed Program `Choose` and shared branch/cost
helpers remain. This does not complete the broader migration: receipt composition and library
search/selection transactions remain open. The bounded 2026-10-21
optional-trigger continuation correction retains a begun choice after its original controller
leaves, lets a surviving player select who will apply or decline it, and preserves the original
stack controller, source, target IDs, and target generations (CR 400.7, 608.2m, 800.4a, 800.4g–h).
Source-owner-only departure leaves the live controller's choice in place. A chosen zone effect
resumes only for the captured object incarnation; a missing or later incarnation is a no-op. Existing
typed Program `Choose` departure behavior is retained through the shared delegation helpers.

Card-specific update (2026-10-22): Conductive Machete, Cursed Windbreaker, and Killer's Mask now
author their exact ETB `ManifestDread` result as an optional `ManifestedPermanent` (`0..1`) and
bind that result to `AttachEquipment`'s creature input. The engine publishes the exact entered
object only after entry commits; an empty library or skipped entry publishes no object, and a stale
Equipment source makes the attach operation a no-op. Existing modifiers, equip costs, and
presentation remain under exact owner assertions. This admits no general attachment resolver or
new effect-list forms. The runtime has a skipped-entry path, but these three owners cannot currently
produce an actual skipped-entry scenario, so that specific scenario evidence remains deferred.
No protobuf, relay, or client contract changed.

Equipment attachment Programs preserve exact produced and selected objects. Kyoshi Battle Fan binds
the complete replacement-expanded `CreatedTokenBatch` to `AttachEquipment`: one token attaches
automatically, while multiple tokens require the Equipment controller to choose one. Vow to Erebor
binds its creature target through the Dwarf check, then offers the spell controller an optional legal
Equipment choice and attaches only that selection. Stale choices do not advance resolution, and
pending choices follow the existing departure-delegation rules. The actual-card scenarios in
`equipment_receipt_program.rs` and `issue_188_chosen_equipment_attachment.rs` cover these receipt and
controller paths. Ordinary targeted attachment remains available; this adds no card admission or
protobuf, relay, or client contract.

Tishana's Tidebinder uses a `CounterTargetAbility` Program with an optional typed
`CounteredAbilitySource` receipt (`0..1`). `IfResult(CounteredAbilitySourceMatches)` requires the
captured source ObjectId and zone-change generation to identify the same battlefield incarnation,
and checks its current Artifact, Creature, or Planeswalker types before the typed
`RemoveAllAbilities(PreviousEffectObject, WhileSourceOnBattlefield)` input applies. The duration is
linked to Tidebinder remaining on the battlefield. The flat-list implicit `PreviousEffectObject`
removal route is retired; ordinary flat-list removal, including The Wondrous Wasp's chosen-creature
effect, remains supported. The actual-card scenarios in `issue_207_stack_abilities.rs` cover
Teferi's registered `activated_01`, a completed no-target result, source-incarnation checks, and two
Tidebinder triggers targeting the same activation. This uses Oracle's current-source-type ruling and
CR 608.2b-c, 611.2b, and 400.7. Prepared-state layer-4 fixtures cover eligible-to-ineligible and
ineligible-to-eligible source-type changes after target selection and before counter resolution,
without changing the ObjectId or zone generation. A separate test replays the accepted serialized Tidebinder command tail
from two identical prepared fixture states and compares every event batch and final diagnostic
snapshot; it is not a full legal-session replay claim. Source-less ability results and
ability-counter-prevention cases remain deferred. No protobuf, relay, or client surface changed, so
native UI acceptance is N/A.

The bounded 2026-10-14 Divert Disaster increment adds an optional `CounterPaymentOutcome` output
for fixed-cost `CounterTargetSpell` leaves and a typed `IfResult` condition. Its total outcomes are
`Paid`, `Declined`, `NotOffered` and `ForcedUnpaid`. `Paid` follows an accepted, committed payment;
`Declined` follows the payer's accepted decline. `ForcedUnpaid` records an already-offered payment
whose payer leaves before answering. After departure detaches any departed outer source and removes
the payer's objects, the engine consumes the standalone `ManaPayment` choice and resumes the same
parked cursor with `ForcedUnpaid`; it does not delegate or synthesize a decline. `NotOffered` means
the counter instruction began without its captured stack target still present as that exact object,
including when an earlier instruction removed it, or its target group was skipped while another
legal group kept the owner resolving. Invalid or unpayable Pay submissions remain pending and
publish no result. Divert's Lander effect consumes only `Paid`, so a departed payer does not create
one. The `CounterUnlessPaid` branch
receipt condition and now-unused `PreviousResultReceipt` wrapper are retired; the shared counter-
placement/sacrifice receipt matcher and other resolution receipts remain.
The bounded 2026-10-15 Choose increment compiles nested controller choices with homogeneous
all-free or fixed Mana/Waterbend costs, the accepted bounded requirements, typed branch yields,
and nested body/Otherwise blocks. Waterbending Lesson pays Waterbend `{2}` or discards one card;
Chaos Spewer may pay `{2}` or blights 2. Payment, Blight creature selection and counter
replacement choices suspend the same program cursor, which resumes its following instruction
once. The exact admitted syntax and fail-closed limits are in the
[active authoring reference](../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md#finite-authored-programs).
At the 2026-10-15 checkpoint, only those two definitions had been ported; other supported costs and
owners still used legacy flat branch resolution. That remaining flat form was retired after the
subsequent Trade Route Envoy port on 2026-10-21.
The following five 2026-10-15 Choose ports reuse that runner without runtime changes: Ant-Man's
Army's mandatory Food-or-Treasure choice, Consecrated Sphinx's optional free Draw 2, Mentor of the
Meek's optional `{1}`-for-Draw 1, Mind's Eye's optional `{1}`-for-Draw 1, and only Solemn
Simulacrum's `triggered_02` death-draw owner. At that checkpoint, Solemn's `triggered_01` ETB
basic-land search remained on its existing legacy branch; the tranche added no card admissions.
The 2026-10-15 Wash Out port uses an all-free color `Choose` and the resultless
`ReturnAllToOwnersHand` program leaf. Mass and single-card owner-hand returns share the Commander
replacement handler, which parks the exact resolving operation through APNAP ordering and
destination choices. Library-return cohort consolidation remains deferred; its existing path is
still separate, and this port adds no card admission.
The 2026-10-16 Battlefield-search increment adds one closed resultless `SearchLibrary` form:
controller's fixed Library, one Basic Land, bounds 0..1, Battlefield tapped, shuffle, no reveal or
result. Lumbering Worldwagon's ETB and actual-attack triggers and Solemn Simulacrum's ETB use it;
Worldwagon's Crew and Solemn's death-draw program remain unchanged. The existing Library-to-
Battlefield entry batch preserves replacement choices and resumes the parked cursor after the
entry pipeline. A zero-card result still shuffles and fires the search event (CR 701.23b, 701.24e);
entry choices and a begun ability remain governed by CR 614.12, 616.1, 113.7a and 608.2m. This
increment adds no card admissions or protocol/client changes.
The bounded 2026-10-17 Embermouth Sentinel increment exercises a resultless conditional destination
on its `triggered_01`: an optional controller `Choose` offers one free branch whose Library
`BasicLand` search reveals the selected card, shuffles, and puts it on top unless the controller has
a Dragon, when it enters tapped. This is the sole currently authored card use of the accepted
conditional-search shape; compilation validates the closed operation and Dragon-count condition
independent of owner identity. The Battlefield aggregate has no Creature filter; a noncreature
Kindred Dragon qualifies under CR 109.2 and 205.3m. The condition is evaluated after the accepted
selection and fixes the destination before the existing entry-replacement pipeline, which resumes
normally (CR 701.23a, 614.12, 616.1, 608.2m). This uses the existing search and entry transactions;
it adds no core, protocol, client, or other card admission.
The bounded 2026-10-18 `SacrificePermanent` increment adds a fixed one-permanent cost only to
`Controller`-chosen ability Programs. It covers four existing owners: Servant of the Stinger's
combat-damage trigger and West Wind Avatar's enter, attack, and Disappear triggers. Every terminal
sacrifice-filter leaf must select a `Creature` or `AnyPermanent` controlled by `You`; typed Programs
reject `excluded_objects` on `any_of` branches while preserving flat exclusions within the
`source_only: false` bound. The parked occurrence revalidates current candidate identity, generation,
filter, and control before committing the shared cost transaction; optional decline or an
offer with no selectable branch follows `Otherwise`, and the paid body and parent cursor resume
once. Other legacy sacrifice callers remain on their existing path. This adds no card admission or
protocol or client change.
The bounded 2026-10-19 `TapPermanents` increment admits a fixed one-permanent cost only on a
`Controller`-chosen ability Program branch. Command Bridge taps one permanent you control and keeps
its existing sacrifice `Otherwise`; Gravelgill Scoundrel taps one creature you control while
excluding the exact source incarnation, then makes that attacking incarnation unblockable. The
flat filter must select `Creature` or `AnyPermanent` controlled by `You`; `any_of` and filter-level
`excluded_objects` are not admitted. The parked resolution keeps its original payer and
generation-bound candidates, then revalidates identity, filter, and control through the shared
one-permanent `TapPermanents`/`SacrificePermanent` cost resolver before committing payment. Only an
untapped permanent can be tapped (CR 701.26a); these choices are not targets. A zone-changed card is
a new object (CR 400.7, 403.4), so a returned Gravelgill can pay the old trigger's cost without the
old effect restricting that new incarnation. This adds no card admission or protocol or client
change.
The bounded 2026-10-19 `LookChooseToHand` increment adds a resultless leaf only to spell Programs
with no filter, no reveal, chosen bottom ordering, and a fixed positive exact selection. Sleight of
Hand uses the two-card, choose-one form; Flow State uses `ProgramIf` to choose its two-card or
one-card form after checking the controller's graveyard for an instant and a sorcery when that
instruction resolves. The branch remains fixed while the private library choice suspends. Moving
cards to hand is not drawing (CR 121.5); the owner orders cards placed together on the library bottom
(CR 401.4). Empty libraries continue without a choice, and a zero- or one-card remainder skips the
order prompt. The existing private `LIBRARY_LOOK` flow and physical candidate binding are reused;
filtered, revealed, optional, random-bottom, and ability-owned forms remain on the legacy effect-list
path. This adds no card admission or protocol or client change (CR 608.2c, 608.2h).
The bounded 2026-10-19 Scalestorm Summoner port admits one finite `ProgramIf` condition:
`MaximumPower` among the ability controller's creatures, with a positive `min`, no `max`, default
remaining filter fields, and no observed-object exclusion. It migrates only `triggered_01` from the
flat `ChooseResolutionBranch` to the existing `If` and `CreateTokens` program operations. The attack
trigger remains unconditional; the pinned Oracle record `55ede1d4-593d-433c-943a-4db12eeb5fa8`
(rechecked 2026-10-19) and WotC ruling (2024-04-12) confirm that the condition is checked only as the
ability resolves, so power added in response qualifies and a qualifying creature removed before
resolution does not (CR 603.4, 608.2c, 608.2h). This reuses the current condition evaluator and leaves
the registered Dinosaur unchanged. Its complete definition and updated review map have independent semantic approval. It adds no card admission or runtime, protocol, or client change.
The 2026-10-19 Replicating Ring and Lavabrink Floodgates migrations use one closed, ability-only
source-condition family: validated `SourceCounterCount` with ordinary bounds and live
`ObjectMatches(Source, AnyPermanent)`, recursively composed with nonempty `AllOf`/`AnyOf` for
Program `If` tests and `Choose` requirements. The leaves keep separate semantics: source-counter
lookup preserves existing source/LKI behavior, while `ObjectMatches` checks exact live-source
membership. This source family remains unsupported in spell Programs and conditional-search
predicates; event-bound leaves and cross-object matching remain closed. Ability Programs admit fixed-positive `PutCounters` and
`RemoveCounters` on `Source` as ordinary effects, plus the existing resultless `RemoveAllCounters`
shape. `EffectsApplicable` currently supports only its existing complete fixed-positive
`PlusOnePlusOne` source-counter branch, Currency Converter's linked-Exile branch, and the exact
Mimic Vat imprint receipt flow below.

There is one additional closed `EffectsApplicable` profile for Currency Converter's first discard
trigger owner: an ability-only optional Controller choice with one free branch, one resultless
`Exile(TriggerObject, Some(linked_id))` operation and an empty `Otherwise` block. The compiler admits
that leaf only inside this exact branch shape and records `TriggerLinkedExileObject`; registry
admission requires a captured event object from the trigger (or an explicit captured-reference
contract). At resolution, the existing object-generation and zone checks decide whether the linked
Exile branch is applicable. This profile does not admit another subject or destination, unlinked
Exile, extra operation, result binding/yield, cost, extra branch, mandatory choice, or spell Program.
`draw_then_discard` and the later `return_linked_card` consumer remain separate owners. Model and
scenario coverage is recorded in the Currency Converter review map.

The 2026-10-21 Mimic Vat increment moves only the `imprint` trigger into an ability-owned Program.
Its second closed `EffectsApplicable` profile is one optional Controller choice with a single free
branch: exile the exact captured `TriggerObject` under the `imprinted_card` link, then immediately
feed its typed 0..1 receipt to `ReturnOtherLinkedExiledCards` for the same ability and link. The
receipt names the newly linked event incarnation, so the return operation skips only that member
and returns every prior linked card to its owner. A missing receipt does not suppress the paired
return instruction, and neither the receipt nor its consumer is available outside this exact owner,
branch and adjacency. Its empty `Otherwise` keeps decline from replacing the current imprint.

The optional-choice departure policy is compiled with Mimic Vat's exact receipt profile. If its
Controller has left before the offer or while the prompt is open, the optional action cost remains
unpaid and the empty `Otherwise` completes without a replacement chooser (CR 800.4f). Currency
Converter's one-leaf optional Exile has no `If you do` follow-up, so it retains the ordinary non-cost
delegation flow. Other Program choices also keep the existing CR 800.4g behavior. Receipt binding,
stale-event rejection, prior-link preservation, source/event-owner departure, and deterministic
accepted-command replay are covered by the Mimic Vat review map.

The 2026-10-22 Mimic Vat increment moves only its activated `create_copy` body into a Program.
The exact `Ability(mimic_vat,mimic_vat,create_copy)` owner binds the one-copy `LinkedExile` result
to both haste and the next-end-step delayed Exile consumer. An empty linked Exile creates neither a
copy nor a delayed observer. The queued activation retains the source incarnation captured when
activated, then reads that incarnation's current link at resolution; it cannot follow the physical
Vat into a later zone-change generation. This owner-specific
admission does not open general Program use of `LinkedExile`, and flat-list `PreviousEffectObjects`
consumers for Haste or delayed Exile are retired while unrelated flat copy creation remains
supported. Automated scenarios cover an empty Vat, source departure through the delayed Exile, and
leave-and-return with a changed link (CR 400.7, 603.7, 607.2a). This adds no card admission or
protocol, relay, or client change.

The 2026-10-22 Koya increment moves only the `triggered_01` parent body into a Program. Its exact
owner binds the optional `0..1` creature target to `Exile(Chosen)` and immediately passes the
operation's zero-or-one exact post-Exile object to `CreateDelayedTrigger(PreviousEffectObject)` for
the existing `delayed_01` child. A completed no-target Exile carries an explicit empty result and
stages no observer; a trigger whose target is illegal before resolution never reaches that producer.
The delayed child keeps the captured object generation and controller rather than searching Exile,
including when a declared commander owner chooses Command after observer creation (CR 400.7j,
113.7a, 608.2h, 704.6d, 903.9a). The retired flat Koya Exile-to-delayed-consumer signature now
requires a typed Program; Teferi's independent flat single-object choose-and-return sequence remains
supported. Compiler profile, source-departure, fizzled-target, optional-empty,
and commander evidence is recorded in the [Koya review map](../tricerules/tricerules-cards/authoring/review-maps/koya,_death_from_above.json).
This adds no card admission or protocol, relay, or client change.

Ring's upkeep Program places one Night counter, checks the threshold after replacement choices
resume the same cursor, removes only Night at eight, and creates eight named Replicated Ring tokens.
Floodgates uses an optional `AffectedPlayer` choice and requires the trigger's captured actor before
instruction zero. If that actor has left before resolution or while a choice is parked, the existing
delegation flow publishes a fresh prompt in the selected player's prompt-ID space; delegation does
not change the ability controller or a compiled cost payer (CR 503.1a, 800.4g). Its threshold binds
the existing typed `CardResultCohort(Sacrifice, Controller, 0..1)` from `Sacrifice(Source)` and passes
that explicit result immediately to `CreateReflexiveTrigger(Sacrificed(Source))`. The ordinary child
uses fixed-positive `DamageAll` for all players with the exact default Creature filter; Floodgates
uses six, and this does not admit direct Program `DamageAll`. The committed sacrifice must be the
exact source incarnation; a replacement destination still represents a successful sacrifice.
Replacement choices finish before the threshold tail, so two Doubling Seasons turn zero Doom into
four, then the child resolves once for six damage to each creature. These ordered checks follow CR
603.12a, 608.2c/h, 701.21a, 122.2, 400.7, 120.4b, and 616.1e-f. This adds no card admissions or
protocol, relay, or client changes.
The bounded 2026-10-20 Standstill migration moves only its exact spell-cast trigger owner into an
ability Program. It binds the direct `Sacrifice(Source)` result, then uses
`IfResult(CardResultCount(result: "sacrificed", min: Some(1), max: None))` to gate Draw 3 for
`EachOtherPlayerThanAffectedPlayer`; the cast trigger supplies the event-time caster and registry
requires that input before instruction zero. The Program predicate counts the named in-scope
cohort with ordinary optional bounds and no ambient source, action, or player selector. Its optional
`card_type` predicate uses recorded `matched_card_types` rather than looking up a card after it moves.
Source-sacrifice receipts capture effective characteristics, including supertypes such as Basic,
before any selected permanent moves and attach those facts to the existing committed movement
receipt; the receipt retains its original action, ObjectId, generation, instructed player, and
provenance.
Standstill's own count remains unfiltered. The flat `ChooseResolutionBranch` `CardResultCount`
requirement was retired on 2026-10-20 after its card owners moved to typed Programs. Named Program
`CardResultCount` and the `CountExpression` `CardsMatchingResult` family remain live; this records
only the status of that branch requirement. Bound `Sacrifice(Source)` and `Mill` are typed producer
examples for named Program conditions, not a claim that other cards are complete.
Automated Standstill scenarios cover the missing-caster preflight, first successful sacrifice among
two cast triggers, and an Abundance replacement park/resume/replay. Its complete definition and updated review map have independent semantic approval. This adds no card admission or protocol, relay, or client change.

The bounded 2026-10-20 Grab the Prize migration adds `ImportCastPayment` only for a nonmodal
spell face with exactly the mandatory single-card `DiscardCard` additional cost and no cast-cost
groups or alternate-method modifiers. The no-input instruction binds one `CastPaymentCohort` for
that exact discard; runtime preflight validates the one canonical discard `ActionFacts` entry before
instruction zero and stores the immutable receipt in the existing typed card-cohort slot. It does
not inspect the original payer's current liveness or cards, so a copy retains the payment facts
after that player leaves. Named `CardResultCount` may filter the imported cohort with its captured
card-type snapshot. The same result predicate also uses the action-time effective-type facts now
recorded by source sacrifice, including the Basic supertype for `BasicLand` filters.
`DamagePlayer(positive fixed amount, EachOpponent)` is admitted only in spell Programs and uses
the ordinary noncombat damage pipeline without adding a target role. Grab draws two, then deals 2
to each opponent only when the paid card snapshot is nonland. Six Grab the Prize scenarios cover
land/nonland payments, mandatory-cost rejection, Abundance resume/replay, competing multiplayer
prevention, zone changes/copies, and copy resolution after the original payer concedes. This adds
no card admission or protocol, relay, or client change.

The bounded 2026-10-20 Shredder's Technique migration binds the result of one spell-owned
`Destroy(Chosen(filter))` instruction to its exact `Subject` target group, which must be `1..1`.
The compiler records a `TargetCardResultCohort` with the `Destroy` action, `0..1` cardinality,
and target-group origin; the existing `ProgramValue::CardResultCohort` carries its successful
`CardResultEntry` into the named `CardResultCount`. Its matched card types come from effective
characteristics at the Destroy evaluation boundary, while owner, ObjectId, post-move generation,
and committed-action provenance stay on the existing entry. Shredder loses 2 life as the spell
controller only after a successful Destroy of an Enchantment; exile replacement still counts,
while indestructible, regeneration, and an illegal target produce no result. Scenarios cover
Enchantment gained, lost, or copied before resolution, a different target owner and controller,
replacement, stale identity, and a parked Aura-return choice that resumes through the tail once.
This follows CR 608.2b, 608.2h-i, 613.1, 613.1d, 701.8a-c, 701.19a-b, 702.12b, 614.1a, 616.1,
and 400.7. Mortify has the same chosen Creature-or-Enchantment Destroy shape but remains on its
existing path. This adds no card admission or protocol, relay, or client change.

The bounded 2026-10-20 Gerrard's Verdict migration uses a spell-owned targeted hand-discard result.
The `ChooseHandCards(Discard)` instruction binds `AffectedPlayer` to its exact singleton player
target group and records the existing typed card cohort; its compiled target-origin descriptor
records `Discard`, `0..2` cardinality, and that group. Named `CardResultCount` checks the captured
card-type facts: two visible Lands gain 6 life, one gains 3, and no Lands gain none. An unrevealed
Library of Leng discard has undefined characteristics and does not count as a Land (CR 701.9c).
Scenarios cover one/two/no Land results, empty/one-card hands, mixed and all-hidden Library of Leng
destinations with replay, target and caster departure at both hand-choice and Library replacement
stages, stale and malformed atomic answers, and a synthetic saved-observer refresh for the actual
Gerrard Program. That Rust fixture does not inspect client-delivered privacy payloads. The separate
two-client `GerrardsVerdictUsesRecipientFilteredLengEvidence` E2E test checks recipient-filtered
hand candidates, concealed physical events and library views, public graveyard identity, the owner
HAND-to-library physical-id aliases and selected top order, and post-draw owner hand snapshots while
the observer's hand snapshots stay blank. A target who leaves after the choice begins contributes
an empty result without
delegation; a begun spell continues after its caster leaves, while that player receives no life
(CR 608.2m, 800.4a). This reuses the target-bound cohort/count contract also used by Shredder's
Technique with a different action and target group. Its [review map](../tricerules/tricerules-cards/authoring/review-maps/gerrards_verdict.json)
indexes the Oracle paragraph and the actual-card scenarios. This adds no card admission or
protocol, relay, or client change.

The bounded Soul-Shackled Zombie migration uses an ability-owned graveyard Exile result. Its
single `Target` binding selects up to two cards from one graveyard and produces an Exile
`TargetCardResultCohort` whose bounds come from the validated target group. The named creature
count reads successful Exile receipts; it causes each opponent to lose 2 life and the controller
to gain 2 life once. Scenarios distinguish zero targets, two noncreatures, partial and all-illegal
targets, a same-ObjectId/new-generation round trip, source departure, atomic malformed selections,
and two creatures in a three-player cast with 22/18/18 life and identical accepted-command replay.
The synchronous producer uses the existing cohort capture and Program instruction boundary;
Shredder's saved-result observer fixture supplies shared interruption evidence, not a natural
Zombie/Aura interaction. The [review map](../tricerules/tricerules-cards/authoring/review-maps/soul-shackled_zombie.json)
indexes the whole Oracle paragraph and its actual-card evidence. This follows CR 113.7a, 115,
603, 608.2b-c/h-i, 701.13a, 400.7 and 119.3. This adds no card admission or protocol, relay, or
client production change.

The bounded 2026-10-20 Currency Converter increment moves only the `return_linked_card` activated
ability in the [handwritten definition](../tricerules/tricerules-cards/data/currency_converter.ron)
to a mandatory ability-owned `ChooseLinkedExiledCard` Program with destination `OwnerGraveyard`.
Its [review map](../tricerules/tricerules-cards/authoring/review-maps/currency_converter.json) covers
all three Oracle lines and the actual-card scenarios. Its `LinkedExileCardResultCohort` slot is keyed
by the exact producer ability and link and has cardinality 0..1. Selection records the chosen card's
pre-move `ActionFacts`, including its type snapshot, before the engine attempts the move to its
owner's graveyard. The later `CardResultCount` instructions test the selected card's type even when a
replacement changes that attempted destination; they then create a Treasure for a Land or a Rogue
for a Nonland. The runtime admits only the matching source incarnation, producer/link pair, and
in-flight instruction.
The public Program offer is face-up only at creation, refresh and answer, and registry validation
rejects a matching Hideaway producer so its private look permission cannot cross to a delegate.
When the original controller or source owner leaves while this exact mandatory choice is active or
saved below observer work, the same Program cursor and original effect-controller attribution are
retained and the linked candidates are reconciled. Under CR 800.4a the departed controller's
noncard stack object ceases to exist; because the ability had already begun resolving, CR 608.2m
requires it to finish. The engine keeps its Program continuation as execution state, not as a new
stack object. If the original chooser has left, CR 800.4g lets the ability controller choose a
replacement chooser. If that controller has also left, the runtime applies CR 800.4h to let the next
live player make that selector choice. This two-step application is an explicit rules interpretation,
not a worked example in the rules text. A departed controller receives no token (CR 800.4b). A
source leave does not rebind the begun occurrence to a later source incarnation; only a choice with
no current candidates completes with an empty result.
This is a narrow owner-graveyard profile, not a new rule for other begun abilities. Nautiloid Ship's
optional `ChooseLinkedExiledCard` return-to-controller-battlefield path remains unchanged.
An ability Program may use `PlayLinkedExiledCard` as a resultless leaf only with `Play`, no filter,
and no source return to its owner's hand if cast. It may be followed by later Program instructions;
linked-play completion resumes the saved Program cursor so that the tail resolves once. Mosswort
Bridge `activated_02` uses this generic profile inside its optional Controller `Choose`, whose
branch checks Controller-creature total power of at least 10 with no maximum. Program `Cast`,
filtered Play, and source-return forms remain excluded. Four-player scenarios cover its departed
controller, hidden linked card, linked-land play, and uncommitted linked-cast paths; a focused core
regression covers a live controller retaining the tail after the source owner and candidate leave.
This existing-card Program owner migration uses existing linked-card and resolution-choice events,
with no new wire, relay or client contract. Its mechanics use CR 608.2m for a begun ability's
continued resolution and CR 800.4a and 800.4g for player departure and delegated choices; it adds no
card admission.

The bounded 2026-10-19 Ward migration moves the exact 17 existing triggered-ability owners to a
resultless `Perform(CounterTriggeringStackObjectUnlessPays(...))` Program leaf: 14 use fixed
nonempty Mana costs without X and three use `DiscardCard(filter: None)`. The generator and
hand-authored RON use the same leaf, while exact `ability_id` ownership and unrelated face clauses
remain intact. Registry admission retires the legacy Ward leaf recursively through legacy triggered
effects, conditional/choice trees, modal modes, and static or dynamic grants; it cannot bypass the
Program's `TriggerAffectedPlayer` and retained `TriggerTargetingStackObject` inputs. The captured
`StackObjectRef` keeps its `ObjectId` and optional zone-change generation, preserving the generation
when present; nonphysical abilities and copies may legitimately have no generation. A missing outer
reference is malformed and distinct from a valid captured reference whose object is no longer live.
Both required inputs are checked before instruction zero. Payment suspends and resumes the same Program owner, frames,
in-flight Ward operation, input context, previous result, and cursor. Observer restoration rebuilds
current mana/discard offers with a fresh prompt ID and current physical-card generations. A begun
ability continues after its source or controller leaves (CR 608.2m); a payer who leaves has not paid
(CR 800.4f), so the captured target receives the unpaid counter attempt without delegating the
choice. When the captured spell or ability is already absent, the payment option still applies under
CR 118.12a and the counter attempt is impossible under CR 609.3; applying that rule to an absent
stack object is an inference rather than a direct ruling. This ports existing owners only and adds
no card admissions or protobuf, relay, or client changes.
The bounded 2026-10-19 Gearbane Orangutan port authors both siblings of its triggered modal
ability as resultless Programs. Mode 1 binds the exact `Destroy(Chosen(AnyPermanent, Artifact))`
subject to its own zero-or-one target group; mode 2 keeps a mandatory controller `Choose` with a
`SacrificePermanent(AnyPermanent, You, Artifact)` cost and one `PutCounters(PlusOnePlusOne, 2,
Source)` body. The compiler admits no other Program Destroy filter or subject. The shared resolution
pipeline applies its stack-wide target-legality check before dispatch (CR 608.2b), then the common
Destroy operation follows CR 701.8a-c. The payer controls the sacrificed permanent (CR 701.21a),
and the tail checks whether payment began, independent of the resulting event (CR 118.12); the
+1/+1 counters follow CR 122.1a. The source-counter tail addresses the original battlefield
incarnation; a card that leaves and returns is a new object and gets no old counters (CR 400.7).
The existing Destroy and sacrifice operations are reused. The shared Program cost offer now honors
`Choose.optional`: Gearbane's mandatory payment requires one current artifact and rejects Decline
or an empty answer atomically. No protobuf, relay, or client change is needed. Abrade and Ancient
Grudge remain existing Destroy operation users. Reuse the shared destruction proofs in
`destruction_tests.rs`:
`issue_196_destroy_receipts_and_public_moves_use_the_actual_destination_and_owner` and
`issue_196_survivors_keep_identity_and_only_regeneration_consumes_a_shield`. The generic paid
branch proofs in `resolution/program.rs` include
`program_choose_sacrifice_permanent_cost_is_offered_to_controller` and
`program_choose_sacrifice_payment_rejects_wrong_and_duplicate_objects_atomically`. The synthetic
`synthetic_program_choose_required_permanent_payment_restores_after_observer_return` fixture saves
the mandatory cost offer across an observer-return restoration with its candidates unchanged; it
checks the fresh prompt ID, mandatory min/no-decline offer, atomic rejection of Decline/empty
submissions, and that payment plus both program tails run once. It is an internal continuation
fixture, not a naturally reachable Gearbane interaction: Myr Retriever's observer trigger queues
after the destroy Program completes. The separate
`program_choose_sacrifice_payment_refreshes_returned_candidate_generation` fixture checks that a
changed permanent generation refreshes the candidates and prompt. The existing
`synthetic_legendary_sacrifice_program_finishes_search_and_tail_before_903_9a` remains the
post-Program Commander SBA ordering reference: its Commander choice follows Program completion.
Gearbane's scenarios cover its exact printed modes.

The bounded 2026-10-19 Lord Skitter's Butcher migration authors all three siblings of its
triggered modal ability as Programs: the Rat token creation, an optional controller choice to
sacrifice another creature and then Scry 2 and draw one card, and the controller-scoped Menace
grant. The creature sacrifice keeps its flat `excluded_objects: [Source]` filter so the entering
Butcher cannot pay; the Scry and Draw stay in the paid branch and `Otherwise` stays empty. The
resultless `GrantKeywordsAll` leaf reuses the ordinary validated `CreatureScopeFilter` and existing
one-shot snapshot resolver in spell and ability Programs. `TargetedPlayer` scopes remain excluded
because this operation has no Program target-role binding. The Menace mode snapshots creatures the
controller controls when it resolves, including Butcher itself, and lasts through the turn; later
entrants and opposing creatures are outside that cohort (CR 700.2b, 608.2c, 701.21a, 701.22a,
121.1, 611.2c, 613.1f, 702.111). This ports one existing generated identity and preserves its
stable face, ability and mode IDs; it adds no card admission or runtime, protocol, relay, or client
operation.

The bounded 2026-10-14 Mill increment adds a typed `CardResultCohort` result from `Mill` and an
earlier-result input to `ChooseCard`. The cohort records committed public output incarnations and
the chooser revalidates their live identity and current characteristics before offering them.
Midnight Tilling and Lluwen, Imperfect Naturalist's ETB use this form. Say Its Name and Fell
Gravship use `WholeCurrentGraveyard` in authored programs because their choice includes any
currently matching graveyard card, including one present before the mill. The old
`ChooseGraveyardCard`/`from_result` schema, adjacency validator, dispatch and parked continuation
are retired; no admitted definition or runtime caller uses that compatibility family. Lluwen's
activated ability that discards a land and creates Worm tokens remains unchanged. `CardResultFilter`
remains available to typed result/count expressions and other operations. The developer
`choice public-zones` fixture exercises the cross-zone candidate projection without moving a card
or changing priority.

The 2026-10-24 counter-controller migration ports An Offer You Can't Refuse, Swan Song, and Arcane
Denial to finite spell Programs. A nonpayment `CounterTargetSpell` may bind one optional
`TargetedSpellController` result only in a spell Program, and only its explicit `StackSpell` target
group supplies the captured controller. A still-targeted but uncounterable spell publishes that
controller; a skipped counter instruction publishes typed `None`, so its downstream token or
subjectless delayed-trigger consumer is a no-op rather than falling back to the caster. This remains
distinct from a whole resolving spell that fizzles under CR 608.2b. `ProgramInputPlayer` is restricted
to `CreateTokens` recipients and subjectless `CreateDelayedTrigger` affected players in spell
Programs. Fixed-payment counters keep their `CounterPaymentOutcome` contract. Arcane Denial's
`delayed_01` child remains an authored Program with `TriggerAffectedPlayer`; `delayed_02` remains a
separate caster-controlled delayed draw. Child owner identity, per-copy binding, exact target-group
scope, and delayed-trigger metadata validation remain enforced. The three former ambient
`PreviousTargetedSpellController` readers and list-adjacency admission paths are retired. This ports
no new card identity and adds no protobuf, relay, or client contract.

The bounded 2026-10-20 Combustible Gearhulk increment admits one narrow composition: an optional,
cost-free `Choose` by `TargetedPlayer(OpponentPlayer)` is paired with exactly one target-bound
`DamagePlayer` on the same singleton, controller-chosen opponent group. The compiler resolves that
group through nested blocks and the owning ability or mode's local target context. In the `Otherwise`
block, a controller `Mill` binds a named result and the immediately following `DamagePlayer` reads
that explicit input with `Count(PreviousMillManaValueSum)`. No wrapper, ambient previous-result read,
intervening instruction, other recipient, or paid targeted-chooser profile is admitted. If the
target leaves while a choice is pending, delegation changes only who answers; it does not replace
the captured damage target. The begun trigger retains source LKI if Gearhulk leaves (CR 113.7a,
608.2b, 608.2h, 608.2m, 800.4a, 800.4g). The six actual-card scenarios cover exact Mill receipts,
draw failure with a short library, Narset's controller-scoped draw replacement, target departure,
delegation with deterministic replay, and ordered damage prevention. Model compiler tests cover the
recursive and owner/mode-local target binding, the immediate named Mill input, and rejected paid,
wrapped, unbound, and mismatched-group forms. The pinned Oracle identity is
[Combustible Gearhulk](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=676036&printed=false);
its 2016-09-20 Wizards rulings confirm no actions during its resolution, a legal draw-three choice
with fewer than three library cards, after which the controller loses for the failed draw, and X
equal to zero in a milled card's mana cost. These tests do not establish a generic Mill
zone-replacement contract or a Rest in Peace interaction. The relevant current rules include CR
104.3a, 104.3c, 121.2, 202.3e, 603.3d, 608.2c, 701.17a-b/d, and 800.4g.

The initial design was reviewed against `41e4ccf463930d8384ac00eb60ac21f931646b38` (2026-10-12).
The former deck campaign remains outside scope. Cards require complete-card evidence and the
applicable verification and visible acceptance gates independently of this design.

## Outcome and boundaries

A new combination of supported operations must be expressible and validated as typed card data,
including choices that suspend resolution. Adding such a card must not require another engine
continuation, card-named dispatcher outcome, or hand-maintained adjacency exception. New rules
mechanics still require engine work and independent semantic review.

Keep the Rust single writer, immutable registry, explicit layers and replacement transactions,
generation-bound identity, deterministic logged decisions, and fail-closed privacy/admission.
Retain typed RON as the authoring format. Implement a finite instruction tree with exhaustive Rust
operations; no arbitrary scripts, author-defined functions, jumps, dynamic recursion, or general-purpose
VM. Finite nested blocks are allowed; author-directed loops are not.
The legacy custom Rust escape hatch is retired: card-model registry admission rejects every parsed
`custom_effect` key. New behavior belongs in typed programs and shared engine primitives.

The design replaces composition contracts, not every rules subsystem. Combat, turn structure,
continuous effects, costs, triggers, state-based actions and replacement ordering retain their
semantic ownership. An instruction sequence never substitutes for a simultaneous event batch.

## Existing seams and concrete changes

| Legacy contract at the design baseline | Target contract |
|---|---|
| [`SpellEffectKind::validate_list`](../tricerules/tricerules-card-model/src/primitives/effects.rs) manually pairs preceding producers and consumers | One model-side program compiler checks operation signatures, typed bindings, scope, cardinality and context |
| `Conditional` admits selected single instructions and rejects nesting | Finite nested conditional blocks share the runner; conditions are evaluated on entry |
| [`run_effect_list_with_previous`](../tricerules/tricerules-core/src/engine/resolution/mod.rs) rebuilds flattened branches and advances a flat index | An immutable compiled program plus explicit block/cursor frames owns sequencing |
| [`EffectResult` and `ParkedStackResolution`](../tricerules/tricerules-core/src/state.rs) carry one implicit previous result | Private typed immutable result slots, an in-flight operation and a saved frame stack |
| [`DrawCompletion`](../tricerules/tricerules-core/src/engine/draw.rs) includes `BeginDiscard` and `BrainstormPutBack` sequencing | Draw completion resumes the owning operation/frame; follow-up card actions are instructions |
| [`RecipeEmission`](../tricerules/tricerules-cards/src/bin/gen_cards/recipes.rs) and exact-text matchers assemble separate special combinations | Reviewed parameterized templates emit the same typed programs as handwritten RON |
| [`SubmitResolutionChoice`](../libcockatrice_protocol/libcockatrice/protocol/pb/ruled_v1.proto) answers whichever prompt is current | Every answer names the exact active `resolution_prompt_id` |

This is semantic consolidation. Moving existing branches into smaller files is insufficient.
The previously shipped shared execution work is a foundation to reuse, not proof this contract
already exists; see live issues [#381](https://github.com/pizzaninja188/Cockatrice/issues/381),
[#380](https://github.com/pizzaninja188/Cockatrice/issues/380) and
[#405](https://github.com/pizzaninja188/Cockatrice/issues/405) for related remaining acceptance,
dynamic-count and outcome-receipt work. Their live scope must be rechecked before implementation.

## Authored program and compiled representation

Use one `CardProgram` shape for spell bodies, modes, activated and triggered ability bodies, and
branch bodies. Each instruction has a stable ID, one finite kind, and optional named output bindings.
An operation's existing effect parameters remain typed. `Perform`, `If` and the existing paid/optional
branch-selection semantics are structural program nodes, rather than special cases in effect lists.

`EffectOp` denotes the operation portion of today's `SpellEffectKind`, not a second parallel enum
containing copies of every existing operation. During migration, lower old effect-list syntax into
the same compiled program. Retire old aliases and composition variants with their migrated families.

The following is the broader target type boundary, including future shapes. Use the active
authoring reference for accepted Rust/RON rather than copying this sketch:

```rust
// card-model: deserialized syntax; output types are inferred from operation signatures.
struct CardProgram { body: AuthoredBlock }
struct AuthoredBlock { steps: Vec<Instruction>, yields: Vec<NamedResultRef> }
struct Instruction { id: InstructionId, bind: Vec<ResultId>, kind: InstructionKind }
enum InstructionKind {
    Perform(EffectOp),
    If { condition: GameCondition, yes: AuthoredBlock, no: AuthoredBlock },
    Choose { choice: ChoiceId, branches: Vec<AuthoredBranch>, otherwise: AuthoredBlock },
}
// card-model: immutable, validated representation, supplied through CardRegistry.
struct CompiledProgram { identity: ProgramIdentity, blocks: Vec<CompiledBlock> }
struct CompiledBlock { steps: Vec<CompiledInstruction>, yields: Vec<TypedSlotRef> }
struct CompiledInstruction { inputs: InputBindings, outputs: OutputSignature, kind: CompiledKind }
// core: private execution state; program identity resolves against the immutable registry.
struct ResolutionFrame {
    program: ProgramIdentity, stack_context: ResolvingContext,
    cursors: Vec<BlockCursor>, slots: Vec<ResultSlot>, in_flight: Option<OperationState>,
}
enum ResultSlot { Unwritten, Ready(TypedValue) }
```

`Choose` retains the existing typed chooser, payment, optionality, branch IDs and decision semantics
from `ResolutionBranchDef`; the abbreviated skeleton does not replace them with an arbitrary menu.
An operation signature owns named output fields and types; `bind` assigns local names to those fields,
not author-selected type annotations. Typed values include generation-bound selections, action/card
cohorts partitioned by affected player, produced-object cohorts with provenance, captured card counts,
and choice/payment outcomes. These remain distinct variants rather than interchangeable object lists.
Compiled input/slot references preserve those types and target-role mappings without runtime string
lookup. `Unwritten` is an execution state and cannot be consumed as an empty semantic result.

Representative consumers ground the vocabulary: Brainstorm/Faithless Looting need a program spanning
draw and hand actions; Gorging Vulture/Gerrard's Verdict distinguish action receipts and filtered card
counts; Myr Battlesphere/Eerie Ultimatum use resolution-time selections with later actions;
Divert Disaster binds the fixed-cost counter-payment result and gates its Lander on `Paid`; these
are API examples, not new admissions.

The card-model crate owns compilation and validation. It produces immutable program/block IDs,
instruction addresses, typed result-slot indexes, target-role mappings and required context.
The registry supplies these programs to core without introducing a production dependency on the
embedded-card crate. Runtime frames reference a program identity and fingerprint, never a closure
or serialized pointer. Replay continues to require the matching engine/card-data environment.

Author IDs are scoped to the owning face/ability/program. Result names are mechanical references;
display text and Oracle presentation mappings cannot define data flow. Stable instruction/target-role
IDs replace positional cross-references in migrated definitions; legacy `effect_indices` lower to
those addresses. A compiler walk computes target requirements even in nested blocks. Resolution-time
conditions do not remove target requirements at announcement or change all-targets-illegal handling.

### Typed bindings and evaluation time

- A binding has one producer and an operation-declared type. References must resolve to an earlier
  dominating producer. No forward references, mutable variables, global result dictionary, or
  cross-ability references are admitted.
- Blocks have lexical scope. Child blocks may read ancestor bindings; child bindings do not escape
  implicitly. A structural instruction may bind its selected block's explicit `yields` through a typed
  join. Every reachable arm, including decline/otherwise, must yield the same field types; cardinality
  is the union of the arms' possible cardinalities. No implicit absent/zero value is supplied. An arm
  without exports has no outer result, and direct cross-arm reads always reject. During migration,
  flat-branch output-to-`PreviousEffect` flows had to lower through explicit joins or move into the
  corresponding Program branch; the flat branch form is now retired. Preserve Divert Disaster's
  semantic paid-result dependency through its typed slot; the
  `CounterUnlessPaid` receipt condition and unused `PreviousResultReceipt` wrapper are retired.
- A runtime-empty cohort is a valid value with count zero. It is distinct from an absent binding,
  declined optional action, failed payment, or an operation still waiting to commit. Those distinctions
  use typed status/receipts, not `None` interpreted differently by each consumer.
- Keep selected objects, committed movement/action receipts, produced objects and captured numbers
  distinct. A selection count is not proof that those objects were discarded, sacrificed or moved.
  Replacement-expanded token cohorts retain their provenance. Exact-one consumers reject many-object
  producers statically; runtime disappearance follows the operation's rules, not a loader error.
- Live selectors and amounts evaluate when their instruction begins, unless their type explicitly
  reads a captured receipt or event/cast-time fact. Never hoist live conditions to program start.
  Dynamic counts reuse `Amount`/count expressions across hand, mill and library operations instead
  of introducing another numeric field type for each operation.
- Object selections retain `(ObjectId, zone_change_generation)` and domain/zone constraints. Movement
  receipts distinguish input incarnation from actual output incarnation. Follow-ups never guess a
  new incarnation by physical card ID/name; last-known information is a separate explicit capability.
- Values and provenance stay server-only. Candidate publication is a separate authorized projection.
  Persist frame/results in deterministic state/checkpoint machinery; never serialize private slots
  into a client event or public debug/log field.

Each operation declares its input/result signature, legal contexts, target roles, and whether it may
suspend. Domain validators still check rules-specific constraints. Share traversal and data-flow
validation; do not replace meaningful per-operation restrictions with blanket acceptance.

## Execution frame and operation completion

One `ResolutionFrame` owns the resolving stack context, program identity, nested block/cursor stack,
result slots, and at most one active operation transaction. It preserves controller, source
incarnation, cast/trigger facts, legal target roles and stack-copy/deferred-exit state. Non-stack
choices such as legend selection retain their own owner; they do not fabricate stack frames.

Conceptual operation outcomes are `Complete(typed_outputs)`, `AwaitChoice(operation_state, offer)`,
`AwaitRulesWork(operation_state, child_work)` and `GameEnded`. These are private engine terms.
Complete means the operation committed all work it owns. Awaiting a draw replacement, entry decision,
discard replacement or child cast is not instruction completion.

1. Resolve the current immutable instruction address. Evaluate its live inputs and capture the
   operation's required snapshot once. Enter the chosen conditional block once; never reevaluate
   its condition merely because a child instruction suspended.
2. Run or resume the operation through the existing audited rules transaction. The operation owns
   proposal, replacement ordering, committed events and its typed partial state.
3. While it waits, retain the same instruction address and unpublished result slots. Parent
   sequencing cannot advance, repeat committed substeps or consume an incomplete receipt.
4. On completion, publish outputs to slots exactly once. Drain required immediate observer work at
   its actual event boundary, preserving a typed parent frame. Collect ordinary triggers without
   granting priority or running ordinary SBAs in the middle of resolution. Domain-specific immediate
   work and state-trigger observations retain their required boundaries.
5. Advance the owning cursor exactly once. Exiting a child block resumes its parent cursor. Finish
   the stack object only after all nested instructions and required immediate work complete.

Observation is finer-grained than instruction completion: each committed individual draw/event
records its triggers at that event boundary; a simultaneous cohort is observed after the whole
semantic batch commits. Required immediate replacement/move consequences finish before dependent
work proceeds. Record state-triggered abilities when their condition becomes true, including during
a still-running/parked operation, with their existing suppression rules. Ordinary recorded triggers
wait for their prescribed stacking opportunity; neither priority nor ordinary SBAs occur mid-resolution.

Draws stay individual draw transactions with their replacement/failed-draw handling. Simultaneous
discard, sacrifice, damage, token creation and battlefield entry keep their existing or corrected
batch semantics. A batch snapshots/selects as required, proposes replacements, commits the semantic
cohort and publishes receipts/triggers at the correct boundary. No generic `ForEach` node is admitted
to turn an inherently simultaneous operation into a loop.

| Existing transaction owner | Completion edge retained by the frame adapter |
|---|---|
| `PendingDrawTransaction` / `DrawCompletion` in [`draw.rs`](../tricerules/tricerules-core/src/engine/draw.rs) | Individual draws can already be committed while the operation waits; retain their progress, but publish the instruction output/advance its cursor only after all owned draw work completes |
| `PendingCounterPlacementBatch` / `CounterPlacementCompletion` in [`state.rs`](../tricerules/tricerules-core/src/state.rs) and [`replacement.rs`](../tricerules/tricerules-core/src/engine/replacement.rs) | Resolve proposed placement replacements and commit under the counter operation's batching contract before returning its placement receipts |
| `BattlefieldEntryEvent`, `EntryCounterReplacementResume::{Single,Batch}` and `BattlefieldEntryCompletion` in the same files | Entry plans and entry counters remain provisional; `validate_battlefield_entry_commit` prevents zone commit before planner completion; one completed adapter resumes the frame once |

Accepted earlier instructions are not undone because a later action is impossible. Perform as much
as the rules allow. Invalid player input must be rejected before mutating the pending frame, results,
events, RNG, command index or active offer. Reuse existing complete-command numeric checkpoints for
their governed errors. Internal compiler/runner invariant failures are not ordinary player illegality.
Generalize/reuse the existing complete-command checkpoint to restore all state, pending work, caches,
events and choice-ID allocators to the start of the failing command, then report an internal error
without accepting/logging that command. Keep earlier accepted commands intact; this is not rollback
of a whole multi-command resolution. No unlogged mutation may survive a failure. Do not automatically
retry an invariant error.

Copies get independent frames/results with the copy's captured choices and controller. Do not share
mutable slots between original and copy. Source departure does not end an independent ability; exact
incarnation checks and explicit LKI govern later source uses. Concession/game termination follows the
existing player-removal rules and disposes or resumes owned work once, including state-trigger guards,
temporary reveal permissions and deferred stack exit. No new two-player assumptions are permitted.

## Prompt identity and component contract

Mandatory logical `resolution_prompt_id` correlation connects `PendingResolution`,
`ResolutionChoiceRequired` and `SubmitResolutionChoice`. It distinguishes successive prompts even
when their candidates are identical. Candidate generation checks remain necessary and solve a
different identity problem.

- Allocate deterministic, nonzero IDs monotonically per deciding player within a game. The correlation
  key is `(game, deciding_player, resolution_prompt_id)`. A globally increasing visible number would leak other
  players' private prompt activity through gaps, even if the field were per-player. Store per-player
  allocator state in engine state/checkpoints. Never wrap/reuse an actor's ID. Reconnect and identical
  active-offer republication retain the logical ID.
- Every replacement/new prompt receives a fresh ID, even when actor, kind and candidates are equal.
  Any answer-semantic change (candidate identity/generation, bounds, allowed decisions, cost, source,
  chooser or ordering) replaces the offer. For `ManaPayment`, `payment_currently_legal` is readiness
  derived from the live mana pool, not answer identity: when only readiness changes for the same
  pending payment, republish under the same ID and refresh its enabled state. A changed cost, source,
  actor or logical payment occurrence receives a fresh ID. If another logical prompt supersedes a
  parent offer, retire the parent's old ID. Saved parent operations retain their state; restoring a
  parent offer revalidates it and allocates a fresh ID. Repainting/reconnecting to an unchanged active
  offer is not replacement.
- Check actor, active ID, payload shape, candidates/generations, bounds and decision legality before
  taking pending state or committing any mutation. A duplicate or late answer is an unchanged-state
  rejection, including unchanged choice-ID allocators. Rejected commands do not enter accepted replay
  logs or advance deterministic state.
- Mark the request ID per-player and restore it only for the deciding recipient. Internal counters,
  frame IDs and result slots are server-only. Commands echo the engine-issued ID, not a client counter.
- Update every Rust/C++ constructor, pending/reconnect snapshot, choice UI submission and replay path
  that carries this contract. Updated clients and server must deploy together; missing/zero IDs reject.
  Existing captures require their original compatible binaries/data; no speculative legacy adapter.

Every accepted gameplay batch, including initial state, reconnect and terminal state, carries an
authoritative `resolution_prompt_state` after automatic work settles. An inactive state retires any
staged resolution answer. An active state identifies the deciding player publicly and carries a
nonzero optional ID only for that player. Other recipients receive no ID field. A pure standalone
payment-preview response preserves the current state; a response with gameplay events or legal
actions is not exempt from the snapshot contract. Server-synthesized, non-authoritative combat
previews also preserve the current state when they omit a snapshot and contain only preview,
log or identity-map events with no legal-action map. They never replace an engine gameplay batch.

Only a `ResolutionChoiceRequired` matching the final active player and ID can install an answerable
offer. Informational or superseded choice events may still publish their authorized reveal effects,
but cannot stage answers. The same active ID without another matching choice event retains an existing
exact offer, including its pending submission callback. A new ID without its offer, a missing gameplay
snapshot, or a changed answer contract under the same ID fails closed. Relay reconnect caches follow
the same final-state rule and preserve only the
exact active offer. Generic per-player redaction must restore the ID only for its deciding recipient.

All resolution answer paths capture the offered ID before a menu, dialog or cast draft and revalidate
it before sending. Resolution-time land play also echoes that ID in `PlayLand`; ordinary land play
uses zero. A stale resolution answer cannot become an ordinary cast or land play. Child work retires
the parent offer; returning to the parent revalidates its candidates and issues a fresh ID without
repeating completed instructions or reevaluating an already-entered conditional block.
Retiring an offer expires its pending callbacks even when submitting the answer already emptied the
local choice holder. Leaving a game clears its prompt state; numeric IDs are scoped to that game.

Use an engine-owned `ResolutionPromptId(u64)` newtype. It is distinct from the existing model's authored
`ChoiceId`, which identifies a branch/choice definition and remains stable across executions. Neither
an authored choice ID nor an instruction ID is a valid prompt correlation token.

| Surface | Required change / boundary |
|---|---|
| card-model / cards | Program schema/compiler, operation signatures, generator emission and reviewed definitions |
| core | Runner, operation completion adapters, typed slots, active choice ID, replay/checkpoint and concession integration |
| protobuf / generated Rust and C++ | Request/answer ID fields with correct visibility; preserve reserved tags |
| Servatrice / network library | Relay opaque IDs, authorized redaction/republication, exact physical bindings; no rules evaluation |
| Cockatrice | Store the active offered ID and echo it from every relevant picker/button/payment/cast submission; reconnect replaces stale local selection |
| Freeform | N/A: all new behavior stays behind ruled gating |

The existing choice kinds, candidate presentation and physical-card projections remain the baseline.
New program/result types do not cross the wire. Any later operation needing a new UI interaction must
trace that separate contract through producers/consumers and obtain its own acceptance evidence.

## Authoring and generator contract

Handwritten RON and generation emit the same program. Templates bind typed amounts, filters,
recipients, durations and keywords, and compose reviewed program fragments with hygienic local IDs.
The generator does not interpret arbitrary Oracle prose or evaluate game state. It matches a reviewed
grammar, consumes the complete relevant clauses/faces, and rejects ambiguity, unsupported combinations,
unconsumed text, unavailable contexts or invalid output through the shared compiler.

Use shared templates for proven families (for example numeric pump plus keyword combinations), not
one Rust matcher per literal parameter combination. Preserve negative/exclusion cases and complete-card
admission. Keep source/rulings provenance, Oracle-line presentation and independent semantic assertions
attached to the same evidence record; structural compilation is not semantic approval. Existing local
datasets are sufficient for implementation preparation; external dataset refresh needs its own authority.
Use the existing review-map/fixture workflow for those links; this design adds no evidence tracker.

Custom Rust effects receive narrow operation/choice capabilities and declare the same typed inputs and
outputs. They may own a specialized algorithm state, but cannot bypass replacements, expose GameState,
construct client-legality rules or choose their own stack cleanup. Shared algorithms migrate into an
ordinary operation/program. Do not force genuinely distinct mechanics into loosely typed parameters.

## Migration sequence and acceptance

These are dependencies for the broader migration; the bounded pilot does not complete this list:

1. The model/compiler and temporary source-only lowering established explicit target roles and
   result provenance. For the retired DrawDiscard family, that lowering and provenance are now
   removed after owner and behavior assertions passed.
2. Introduce the frame runner and completion adapters under the current operations. Migrate a bounded
   draw/hand/library family through real cards; prove nested resume, exactly-once completion and unchanged
   simultaneous/replacement semantics. Keep one owner per resolving instruction during transition.
3. Implement prompt-ID propagation through all producers/consumers before promising stale-answer
   rejection. Run both Rust and C++ gates and visible interaction acceptance for the changed path.
4. Complete for the DrawDiscard family: its 45 owners use explicit programs, and its old sequencing
   variant, draw-after continuations, and source adjacency lowering are removed.
5. For this family, existing generator templates already emit authored programs. Source datasets
   were not refreshed as part of the retirement. Broader generator and corpus migrations remain
   separate work.

Deletion is mandatory when no admitted definition, generated template or runtime caller uses a
superseded path and its independent assertions pass through the shared implementation. Legacy
`custom_effect` data fails registry admission; temporary lowering has a family-specific retirement
condition, and there is no permanent second execution architecture.

Pilot retirement inventory: `custom::Brainstorm`, `ResolutionStep::Draw`, `PostDrawPhase`,
`DrawCompletion::BrainstormPutBack`, the custom draw-completion bridge and its library-placement
helper are removed. Brainstorm uses shared draw, private ordered hand selection and bound top
placement through the program runner. The `DrawDiscard` variant, source lowerer and `BeginDiscard`
continuation are now removed; all 45 owners from the original census use authored programs. The
`custom::GiftsUngiven` implementation and generated custom-effect registry are removed. Gifts now
uses a typed retained search, explicit reveal, target-bound partition, movement and shuffle; scenarios cover the
0–4 cohort matrix, target delegation, source/controller departure, commander replacement, and
precommit graveyard order. This composable execution migration does not change Gifts' whole-card
admission status; whole-card review remains deferred. Keep `custom_resolution/*` where it implements
shared mechanic transactions, copy legality or entry/APNAP work; its directory name does not make every
contained mechanism card-specific.
Wash Out uses the shared owner-hand return continuation for matching mass returns and individual
returns. Library-return cohort consolidation remains separate and deferred for a later migration.

Fanatic of the Harrowing's exact `triggered_01` owner now uses an authored ability Program. It
collects each live player's private `Exact(1)` discard choice in APNAP order, commits the complete
resulting cohort, then checks whether the trigger controller discarded at least one card before
drawing one. The narrow compiler addition admits ability-owned `Discard(EachPlayer, Exact(1))` with
an unbounded aggregate result and its controller-only, unfiltered `DiscardCount` projection with
`min: Some(1)` and `max: None`; existing `EachOpponent` and controller-produced result forms remain
intact.
This reuses the existing Program and player-set discard runtime, preserves CR 101.4, 109.5, 603.3a,
608.2c, 614.6 and 701.9a semantics, and adds no card admission or whole-card readiness claim. No
protobuf, relay or client surface changes.
Player departure now retires only that player's pending row and corresponding uncommitted selection;
live accepted rows retain APNAP order and generation bindings. Observer restoration republishes the
same player-set transaction with a fresh prompt ID and its original Program cursor. A begun
controller-owned ability continues through controller departure only for mandatory, unfiltered
player-set discard; spell, optional and alternative-cost departure behavior is unchanged.

The bounded 2026-10-19 Crypt Lurker increment moves its ETB owner to a resultless ability Program.
An optional `Controller`-chosen `Choose` offers one creature-sacrifice branch and one
`DiscardCard(filter: Some(Creature))` branch; either paid branch draws one card. This filtered
resolution cost is admitted only on an optional `Controller`-chosen ability branch. At the
2026-10-19 Crypt Lurker checkpoint, the flat `ChooseResolutionBranch` `DiscardCard` form was rejected
during card validation and had no runtime `HandCards` dispatch, while other flat branch costs were
still supported. The entire flat branch schema/runtime was retired on 2026-10-21 after the Trade
Route Envoy port. `ResolutionCost::DiscardCard` and the shared physical-candidate and cost
transactions remain in use for the separate Ward
`CounterTriggeringStackObjectUnlessPays` Program leaf with `filter: None`.

Crypt Lurker's pinned Oracle line authorizes one of the two actions and draws one card if either is
paid; the 2020-06-23 ruling confirms that both alternatives cannot be performed to draw extra
cards. Resolution-cost payment follows CR 118.12, discard CR 701.9, sacrifice CR 701.21a, and the
trigger continues independently of its source once on the stack (CR 113.7a). Integration scenarios
cover both paid alternatives, declines and empty offers, filtered and stale cards, Library of Leng
and Madness, source departure, and payer departure without delegation. A synthetic core fixture
directly restores a saved same-candidate discard-payment Program and verifies a fresh prompt with
the retained payer, cursor, and candidate generation before paying once; it is continuation
robustness evidence rather than a Crypt Lurker card scenario. No card admission or protocol, relay,
or client change is added.

A separate synthetic unit fixture moves the same discard candidate from hand to graveyard and back,
then invokes the departure refresh directly to verify generation rebinding; this is distinct from
the saved-observer fixture and Crypt Lurker's integration evidence.

Heated Argument's exact face owner now uses an authored `spell_program`: deal six damage to the
target creature, then offer the controller the optional one-card graveyard-exile cost, with the
paid branch dealing two damage to that target's controller. Its empty `Otherwise` preserves the
decline/no-candidate path. This is one owner migration and adds no card admission or whole-card
readiness claim. The scenario coverage exercises a real Ray of Command response, the payer's own
graveyard and wrong-actor rejection, payer concession while payment is parked, damage-replacement
suspension and resume, an empty min-zero payment, and a declared Commander exiled as the cost before
the rider, spell completion and CR 903.9a choice. Generic target-group LKI remains a separate
engine fixture because Heated Argument has no natural in-resolution zone-change seam. The change
does not alter protobuf or relay behavior; when a reachable non-overlapping placement fits the
available viewport, the library picker moves clear of the currently visible canonical stack so
Heated Argument's true arrow origin remains readable, and a constrained fallback keeps the picker
operable. The legacy flat `ChooseResolutionBranch` form of
this graveyard-exile cost is retired; the typed Program `Choose` form is its supported authored
shape.

Impossible Inferno's existing face owner now uses a spell Program: target-bound 6 damage to a
Creature, followed by a direct `If` for `Controller` `DistinctCardTypes` in the graveyard with
`min: Some(4)`, no maximum, and no filter. This is a narrow spell-only compiler profile; the
existing Controller `CardCount` profile remains valid. If the condition is true, its yes arm runs
one resultless `ExileTopWithPlayPermission(Controller, count: 1, no cast-cost conditional)`; the
no arm is empty. The existing operation handles the face-up Exile move and generation-bound play
permission through the controller's next turn, while the card still uses normal play costs and
timing. An empty library creates no permission. Damage resolves before the condition, with no
state-based actions between instructions (CR 608.2c, 608.2h, 704.4); an illegal sole target
prevents the spell from resolving (CR 608.2b). The scenarios cover both changes to the four-type
threshold between casting and resolution, damage in both outcomes, an illegal sole target, and an
empty library. The [review map](../tricerules/tricerules-cards/authoring/review-maps/impossible_inferno.json)
links the exact typed paths and actual-card evidence. This ports an already registered definition;
it changes no whole-card admission, runtime primitive, protobuf, Servatrice, or Qt contract.
Burning Curiosity and Clockwork Percussionist remain other consumers of the same permission
operation.

Pilot selection and implementation remain separate. Freeze representative existing and later-consumer
examples before coding. Brainstorm and Faithless Looting exercise sequential draw/hand actions; Gifts
Ungiven exercises interdependent private/public choices; Eerie Ultimatum exercises selection followed by
simultaneous entry. They are semantic references, not a claim that one new operation admits all four.
Flashback, search permissions, uniqueness constraints, and entry/replacement behavior remain separate
complete-card prerequisites.

Required evidence includes:

- Compiler rejection of forward/out-of-scope/wrong-type references, invalid cardinality/provenance,
  unsupported contexts, ambiguous targets, malformed nested trees and unsupported templates.
- Nested branch choice/replacement suspension resumes the exact instruction once; skipped branches
  neither prompt nor leak results; impossible later actions preserve earlier legal actions.
- Replacement-expanded and simultaneous operations preserve event count, ordering and committed
  receipts; selected versus actually moved/discarded/sacrificed cohorts remain distinguishable.
- Multi-player simultaneous actions collect their required choices in APNAP order before the cohort
  commits. Target legality is separate from resolution-time choice: an illegal Gifts Ungiven target
  produces no resolution prompt; searched candidates stay private until the explicit revealed cohort
  becomes public. Draw replacements finish per draw; Eerie's entering Auras cannot attach to another
  simultaneous entrant.
- Original/copy independence, source leave-and-return, disappearing candidates, player departure,
  terminal resolution, and deferred spell exit/state-trigger-guard cleanup.
- Duplicate/late/wrong-player choice rejection, reconnect with the same active ID, changed-offer
  invalidation, private candidate/ID redaction, physical identity/order and deterministic replay.
- Inject failures after a transaction commit and after slot/offer allocation: restore the entire
  failing command, including RNG, caches, pending frames, IDs and emitted events, with no replay-log
  advancement. Previously accepted resolution commands remain intact.
- Existing independent card scenarios plus one preselected later consumer that needs no engine edits
  within the newly supported family. Count full-card readiness separately from primitive coverage.

Use focused intended red/green tests, independent design/frozen-code review and the affected-side gates
in [AGENT-VERIFICATION.md](AGENT-VERIFICATION.md). The prompt-ID change requires Rust and C++, relay E2E,
and real two-client GUI checks; protocol E2E does not establish visible acceptance. Record any manual
deferral explicitly. Measure subsequent-card engine edits and phase-separated authoring effort; do not
set success by gross line deletion or extrapolate a pilot into a full-format schedule.

## MTG applicability and source boundary

This design governs resolution ordering, choices, targets, replacements, simultaneous actions, object
identity and private information. Oracle and current official Comprehensive Rules govern every migrated
operation/card. No rules simplifications or new card admissions are authorized by this design.

Sources checked on 2026-10-12, with Heated Argument added 2026-10-19 and Impossible Inferno added
2026-10-20: the [official rules page](https://magic.wizards.com/en/rules) links the
[Comprehensive Rules effective 2026-09-25](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf).
The independent Luna MAX rules reviewer fetched that PDF and the following Wizards card-text/ruling
references. They establish architectural constraints; they do not replace exact-identity preflight
and card-specific evidence when the pilot is selected. Impossible Inferno's exact Scryfall card and
rulings record is linked separately below.

| Source | Constraint on the contract |
|---|---|
| [Brainstorm: Final Fantasy release notes](https://magic.wizards.com/en/news/feature/final-fantasy-release-notes) | Three individual draws and their replacement sub-actions finish before the ordered put-back; no priority window between instructions |
| [Faithless Looting: Strixhaven release notes](https://magic.wizards.com/en/news/feature/strixhaven-school-of-mages-and-commander-2021-edition-release-notes-2021-04-16) | Complete the draw events, then the discard instruction, within one resolution; the flashback cast method remains a separate prerequisite |
| [Gifts Ungiven: Double Masters 2022 release notes](https://magic.wizards.com/en/news/feature/double-masters-2022-release-notes-2022-06-24) | Targeted opponent, up-to-four distinct names, public found cohort and opponent's subsequent two-card choice are distinct facts; finding one or two forces those cards into the chosen cohort |
| [Eerie Ultimatum: Tarkir Dragonstorm release notes](https://magic.wizards.com/en/news/feature/tarkir-dragonstorm-release-notes) | Choose the different-name cohort at resolution and return it simultaneously; an entering Aura cannot enchant another member of that same entering cohort |
| [Heated Argument: Secrets of Strixhaven release notes](https://magic.wizards.com/en/news/feature/secrets-of-strixhaven-release-notes) | Six target damage, an optional graveyard exile, and the conditional two damage follow the printed order; an illegal sole target prevents the whole spell from resolving |
| [Impossible Inferno: Scryfall Oracle record](https://scryfall.com/card/dsk/140/impossible-inferno) and [rulings](https://api.scryfall.com/cards/a35248f9-4a4e-4758-a4c1-0e0c83e3fd75/rulings) | Deal 6 damage first, then check Delirium during resolution; the exiled card still requires its normal costs and timing |

Relevant CR constraints from that version:

- **608.2a-b:** intervening-if and target-legality checks keep their distinct timing. A conditional
  instruction does not postpone or replace the initial all-targets-illegal check.
- **608.2c-f/h-i/p:** follow printed instruction order, make required resolution choices, evaluate
  information at the required time/LKI boundary, and finish resolution before ordinary priority.
  Partial impossibility is not rollback of earlier legal instructions.
- **121.2/121.2a, 614.11a, 616.1:** multiple draws happen individually; a draw replacement's required
  actions finish before the next draw, with applicable replacement choices/order preserved.
- **400.7:** a zone change normally creates a new object. A retained selection is not permission
  to act on a later incarnation; exceptions/actual movement receipts are explicit.
- **303.4f, 614.13a:** Aura attachment and choices within entry-modifying effects have specific
  entering-object restrictions; 614.13a is not a general model for every entry replacement condition.
  Use the Eerie ruling and 608.2f for its simultaneous cohort, rather than independent ordinary moves.
- **603.3, 101.4:** ordinary triggered abilities wait for the prescribed placement opportunity;
  APNAP choice ordering remains owned by the applicable operation/rules boundary.
- **603.8, 117.3b, 704.3:** observe state triggers during resolution when applicable; ordinary
  priority/SBA processing waits for the prescribed priority boundary after resolution.
- **608.2b-c/h/n, 704.3-4, 903.9a:** Heated Argument checks target legality before following its
  instructions, uses the target's current or last-known controller for its later rider, and puts
  the instant into its owner's graveyard as the final resolution step. A declared Commander exiled
  as the payment remains eligible for its separate CR 903.9a choice after the spell finishes; SBAs
  do not interrupt resolution.
- **115.1a, 601.2c, 608.2b-c/h, 207.2c, 704.4, 400.7, 406.3, 305.1-3:** Impossible Inferno's
  creature target is fixed when cast and must remain legal for the spell to resolve; its direct
  Delirium check sees the controller's graveyard when that instruction is reached after damage.
  SBAs wait until resolution ends. The exiled card is a new face-up object, and its play permission
  preserves ordinary cost, timing and land-play rules.
- **104.3a, 800.4g, 514.1-3:** concession/player departure and cleanup are independent rules surfaces
  that must survive parked resolution; cleanup/ordinary SBAs are not inserted between instructions.

The interaction checklist is covered by the ownership/identity, completion/timing, rejection/player,
privacy, component and verification sections above. The Impossible Inferno source, ruling, CR
application and actual-card scenarios are linked from its review map. Other cards still need their
own exact Oracle identities, applicable rulings, CR analysis and executable evidence. This migration
keeps ruled behavior engine-authoritative and leaves Freeform gating unchanged.
