// Headless client-core tests for the ruled view model.
//
// The client mirror of tests/ruled_batch_tests/: synthetic `ruled::v1::RuledEventBatch`
// messages go into `RuledEventDispatcher`, and the assertions are on `RuledClientState` —
// identity maps, legal-action parsing per action kind, stack tracking, combat staging, and
// pending player choices. Nothing here renders: `RuledClientState` and `RuledEventDispatcher`
// reach the Qt game objects only through `RuledClientHost`, and `FakeHost` below stands in for
// `GameEventHandler`, recording the commands the view model would have sent.
//
// This is the layer that used to be untestable — the same logic lived inline in the
// `RULED_PAYLOAD` case of the upstream `GameEventHandler`, behind the whole client.

#include "game/ruled/ruled_client_host.h"
#include "game/ruled/ruled_client_state.h"
#include "game/ruled/ruled_event_dispatcher.h"

#include <QSignalSpy>
#include <QString>
#include <gtest/gtest.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

namespace
{

constexpr int kLocalPlayer = 0;
constexpr int kOpponent = 1;

/// Records everything the view model asks of the UI, and answers the few queries it makes.
class FakeHost : public RuledClientHost
{
public:
    int local = kLocalPlayer;
    int activePlayer = -1;
    int priorityPlayer = -1;
    int toolbarPhase = -1;
    int arrowSyncRequests = 0;

    struct SyntheticCard
    {
        quint32 oid;
        QString name;
        int controllerPlayerId;
        QString setName;
    };
    QVector<SyntheticCard> createdSyntheticCards;
    QVector<quint32> removedSyntheticCards;
    QVector<ruled::v1::RuledCommand> sentCommands;
    int dialogRequests = 0;
    QString lastDialogPrompt;

    /// Optional P/T fallback, keyed by engine oid, for the ZoneView-stripped path.
    QHash<quint32, QPair<int, int>> fallbackPt;
    QHash<quint32, QString> cardNames;
    QHash<quint32, QString> providerIds;

    [[nodiscard]] int localPlayerId() const override
    {
        return local;
    }
    [[nodiscard]] int currentActivePlayerId() const override
    {
        return activePlayer;
    }
    void setActivePlayerId(int playerId) override
    {
        activePlayer = playerId;
    }
    void setPriorityPlayerId(int playerId) override
    {
        priorityPlayer = playerId;
    }
    void setToolbarPhase(int phase) override
    {
        toolbarPhase = phase;
    }
    void createSyntheticStackCard(quint32 virtualOid,
                                  const QString &displayName,
                                  int controllerPlayerId,
                                  const QString &setName) override
    {
        createdSyntheticCards.append({virtualOid, displayName, controllerPlayerId, setName});
    }
    void removeSyntheticStackCard(quint32 virtualOid) override
    {
        removedSyntheticCards.append(virtualOid);
    }
    [[nodiscard]] QString stackCardProviderId(quint32 oid) const override
    {
        return providerIds.value(oid);
    }
    [[nodiscard]] bool fallbackCreaturePt(quint32 engineOid, int *power, int *toughness) const override
    {
        const auto it = fallbackPt.constFind(engineOid);
        if (it == fallbackPt.constEnd()) {
            return false;
        }
        *power = it->first;
        *toughness = it->second;
        return true;
    }
    [[nodiscard]] QString battlefieldCardName(quint32 engineOid) const override
    {
        return cardNames.value(engineOid);
    }
    void sendRuledCommand(const ruled::v1::RuledCommand &command) override
    {
        sentCommands.append(command);
    }
    void sendRuledCommandExpectingAck(const ruled::v1::RuledCommand &command,
                                      std::function<void(bool accepted)> onFinished) override
    {
        sentCommands.append(command);
        pendingAck = std::move(onFinished);
    }
    void requestResolutionChoiceDialog(const QString &prompt,
                                       const QVector<quint32> &,
                                       const QStringList &,
                                       int,
                                       int,
                                       bool,
                                       bool) override
    {
        ++dialogRequests;
        lastDialogPrompt = prompt;
    }
    void scheduleSpellTargetArrowSync() override
    {
        ++arrowSyncRequests;
    }

    /// Answer the most recent command that asked for an ack (confirmBlockers).
    void answerPendingAck(bool accepted)
    {
        ASSERT_TRUE(static_cast<bool>(pendingAck));
        auto cb = pendingAck;
        pendingAck = nullptr;
        cb(accepted);
    }

private:
    std::function<void(bool)> pendingAck;
};

class RuledClientTest : public ::testing::Test
{
protected:
    FakeHost host;
    RuledClientState *state = nullptr;
    RuledEventDispatcher *dispatcher = nullptr;

    void SetUp() override
    {
        state = new RuledClientState(&host);
        dispatcher = new RuledEventDispatcher(state, &host);
    }

    void TearDown() override
    {
        delete dispatcher;
        delete state;
    }

    void apply(const ruled::v1::RuledEventBatch &batch)
    {
        // Go through the serialized entry point so the per-batch reset runs exactly as it does
        // when an Event_RuledPayload arrives.
        std::string payload;
        ASSERT_TRUE(batch.SerializeToString(&payload));
        ASSERT_TRUE(dispatcher->processPayload(payload));
    }

    /// Batch carrying just a phase change, to drive the combat state machine.
    static ruled::v1::RuledEventBatch phaseBatch(ruled::v1::PhaseId phase, int activePlayerId)
    {
        ruled::v1::RuledEventBatch batch;
        auto *pc = batch.add_events()->mutable_phase_changed();
        pc->set_phase_id(phase);
        pc->set_active_player_id(activePlayerId);
        return batch;
    }

    static ruled::v1::BattlefieldObjectMap::Entry *
    addPermanent(ruled::v1::RuledEvent *event, int playerId, quint32 oid, int serverCardId)
    {
        auto *entry = event->mutable_battlefield_object_map()->add_entries();
        entry->set_player_id(playerId);
        entry->set_engine_object_id(oid);
        entry->set_server_card_id(serverCardId);
        entry->set_is_creature(true);
        return entry;
    }
};

// ---------------------------------------------------------------------------------------
// Identity maps
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, BattlefieldObjectMapBuildsIdentityMapsBothWays)
{
    ruled::v1::RuledEventBatch batch;
    auto *ev = batch.add_events();
    auto *e1 = addPermanent(ev, kLocalPlayer, 100, 7);
    e1->set_summoning_sick(true);
    e1->set_haste(true);
    auto *e2 = addPermanent(ev, kOpponent, 200, 9);
    e2->set_trample(true);
    e2->set_is_creature(false);
    apply(batch);

    EXPECT_EQ(state->engineOidForCardId(kLocalPlayer, 7), 100u);
    EXPECT_EQ(state->cardIdForEngineOid(100), 7);
    EXPECT_EQ(state->playerIdForEngineOid(100), kLocalPlayer);
    EXPECT_TRUE(state->isEngineOidSummoningSick(100));
    EXPECT_TRUE(state->isEngineOidHaste(100));
    EXPECT_TRUE(state->isEngineOidCreature(100));

    EXPECT_EQ(state->engineOidForCardId(kOpponent, 9), 200u);
    EXPECT_TRUE(state->isEngineOidTrample(200));
    EXPECT_FALSE(state->isEngineOidCreature(200));

    // Unknown ids answer with the documented sentinels rather than asserting.
    EXPECT_EQ(state->engineOidForCardId(kLocalPlayer, 999), 0u);
    EXPECT_EQ(state->cardIdForEngineOid(999), -1);
    EXPECT_EQ(state->playerIdForEngineOid(999), -1);
}

TEST_F(RuledClientTest, HandSlotAndGraveyardMapsAreQueryable)
{
    ruled::v1::RuledEventBatch batch;
    auto *hs = batch.add_events()->mutable_hand_slot_map();
    auto *he = hs->add_entries();
    he->set_player_id(kOpponent);
    he->set_server_card_id(42);
    he->set_hand_index(3);
    auto *gy = batch.add_events()->mutable_graveyard_object_map();
    auto *ge = gy->add_entries();
    ge->set_engine_object_id(500);
    ge->set_server_card_id(11);
    apply(batch);

    EXPECT_EQ(state->engineHandSlotForServerCard(kOpponent, 42), 3);
    EXPECT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 42), -1);
    EXPECT_EQ(state->graveyardEngineOidForServerCardId(11), 500u);
    EXPECT_EQ(state->graveyardEngineOidForServerCardId(12), 0u);
}

TEST_F(RuledClientTest, HandSlotMapIsRebuiltFromScratchEachBatch)
{
    ruled::v1::RuledEventBatch first;
    auto *he = first.add_events()->mutable_hand_slot_map()->add_entries();
    he->set_player_id(kLocalPlayer);
    he->set_server_card_id(5);
    he->set_hand_index(0);
    apply(first);
    ASSERT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 5), 0);

    // A later batch with no hand_slot_map must not leave the stale slot behind.
    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN1, kLocalPlayer));
    EXPECT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 5), -1);
}

// ---------------------------------------------------------------------------------------
// Legal-action parsing — one case per hand-action kind
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, ParsesLandPlayLabelsIncludingMdfcFaces)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_labels("Play land Forest (hand idx 2)");
    actions.add_labels("Play land Cragcrown Pathway (hand idx 4, face 0)");
    actions.add_labels("Play land Timbercrown Pathway (hand idx 4, face 1)");
    actions.add_labels("Cast Lightning Bolt (hand idx 1, target)");
    apply(batch);

    EXPECT_TRUE(state->isLandPlayLegalForHandIndex(2));
    EXPECT_TRUE(state->isLandPlayLegalForHandIndex(4));
    EXPECT_FALSE(state->isLandPlayLegalForHandIndex(3));
    EXPECT_EQ(state->landPlayHandIndicesForCardName("Forest"), QList<int>({2}));

    // CR 712: one hand slot, two playable faces, sorted by face index.
    const QVector<RuledLandFaceOption> faces = state->landPlayFaceOptionsForHandIndex(4);
    ASSERT_EQ(faces.size(), 2);
    EXPECT_EQ(faces[0].faceIndex, 0);
    EXPECT_EQ(faces[0].faceName, QStringLiteral("Cragcrown Pathway"));
    EXPECT_EQ(faces[1].faceIndex, 1);
    EXPECT_EQ(faces[1].faceName, QStringLiteral("Timbercrown Pathway"));
    // A single-face land still reports exactly one option, at face 0.
    ASSERT_EQ(state->landPlayFaceOptionsForHandIndex(2).size(), 1);
    EXPECT_EQ(state->landPlayFaceOptionsForHandIndex(2)[0].faceIndex, 0);
}

TEST_F(RuledClientTest, ParsesSpellCastLabelsAndTargetRequirement)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_labels("Cast Lightning Bolt (hand idx 1, target)");
    actions.add_labels("Cast Llanowar Elves (hand idx 3)");
    apply(batch);

    EXPECT_TRUE(state->isSpellCastLegalForHandIndex(1));
    EXPECT_TRUE(state->isSpellCastNeedsTargetForHandIndex(1));
    EXPECT_TRUE(state->isSpellCastLegalForHandIndex(3));
    EXPECT_FALSE(state->isSpellCastNeedsTargetForHandIndex(3));
    EXPECT_EQ(state->spellCastHandIndexForCard("Llanowar Elves", 99), 3);
    EXPECT_EQ(state->spellCastHandIndexForCard("Nonexistent", 0), -1);
}

TEST_F(RuledClientTest, ParsesCleanupDiscardLabelsAndRequiredCount)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    // CR 514.1: nine cards in hand means two must be discarded.
    for (int i = 0; i < 9; ++i) {
        actions.add_labels(("Discard Card" + std::to_string(i) + " (cleanup, hand idx " + std::to_string(i) + ")"));
    }
    apply(batch);

    EXPECT_TRUE(state->localPlayerMustCleanupDiscard());
    EXPECT_EQ(state->cleanupDiscardRequiredCount(), 2);
    EXPECT_EQ(state->cleanupDiscardSelectedCount(), 0);

    state->toggleCleanupDiscardHandIndex(0);
    state->toggleCleanupDiscardHandIndex(4);
    EXPECT_EQ(state->cleanupDiscardSelectedIndicesSorted(), QList<int>({0, 4}));
    // Selection is capped at the required count.
    state->toggleCleanupDiscardHandIndex(6);
    EXPECT_EQ(state->cleanupDiscardSelectedCount(), 2);
    // Toggling off frees a slot again.
    state->toggleCleanupDiscardHandIndex(0);
    state->toggleCleanupDiscardHandIndex(6);
    EXPECT_EQ(state->cleanupDiscardSelectedIndicesSorted(), QList<int>({4, 6}));
    // Illegal slots are never selectable.
    state->clearCleanupDiscardSelection();
    state->toggleCleanupDiscardHandIndex(50);
    EXPECT_EQ(state->cleanupDiscardSelectedCount(), 0);
}

TEST_F(RuledClientTest, ParsesOpeningLabelsIntoTheThreeOpeningModes)
{
    {
        ruled::v1::RuledEventBatch batch;
        auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
        actions.add_labels("You start (opening pick)");
        actions.add_labels("Opponent starts (opening pick)");
        apply(batch);
        EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::ChooseFirst);
    }
    {
        ruled::v1::RuledEventBatch batch;
        auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
        actions.add_labels("Keep opening hand (opening)");
        apply(batch);
        EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::MulliganChoice);
    }
    {
        ruled::v1::RuledEventBatch batch;
        auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
        actions.add_labels("Keep opening hand (opening)");
        actions.add_labels("Put Forest on bottom (opening, hand idx 0)");
        actions.add_labels("Put Mountain on bottom (opening, hand idx 5)");
        apply(batch);
        // The bottoming step wins over the mulligan prompt when both labels are present.
        EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::BottomLibrary);
        EXPECT_TRUE(state->isOpeningBottomLegalForHandIndex(0));
        EXPECT_TRUE(state->isOpeningBottomLegalForHandIndex(5));
        EXPECT_EQ(state->openingBottomLegalHandIndicesSorted(), QList<int>({0, 5}));
    }
}

TEST_F(RuledClientTest, ParsesTargetingTablesForHandSlotsAndAbilities)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    // Hand slot 1, face 0 — the composite key the engine emits.
    auto &slotTargets = (*actions.mutable_valid_targets_by_hand_slot())[(1u << 8) | 0u];
    slotTargets.add_valid_permanent_ids(100);
    slotTargets.add_valid_stack_ids(300);
    slotTargets.add_valid_graveyard_ids(500);
    slotTargets.set_can_target_opponent(true);
    slotTargets.set_is_damage_targets(true);
    slotTargets.set_max_targets(3);
    slotTargets.set_fixed_damage(4);
    slotTargets.set_extra_mana_per_target(1);
    // Ability index 2 on permanent 100.
    auto &abilityTargets = (*actions.mutable_valid_targets_by_ability())[(quint64(100) << 32) | 2u];
    abilityTargets.add_valid_permanent_ids(200);
    abilityTargets.set_can_target_self(true);
    apply(batch);

    EXPECT_TRUE(state->isValidSpellTarget(1, 0, 100));
    EXPECT_FALSE(state->isValidSpellTarget(1, 0, 101));
    // A different face of the same slot carries its own (here: empty) target set.
    EXPECT_FALSE(state->isValidSpellTarget(1, 1, 100));
    EXPECT_TRUE(state->isValidSpellStackTarget(1, 0, 300));
    EXPECT_TRUE(state->isValidSpellGraveyardTarget(1, 0, 500));
    EXPECT_TRUE(state->canSpellTargetOpponent(1, 0));
    EXPECT_FALSE(state->canSpellTargetSelf(1, 0));
    EXPECT_TRUE(state->spellIsDamageTargets(1, 0));
    EXPECT_EQ(state->spellMaxTargets(1, 0), 3);
    EXPECT_EQ(state->spellFixedDamage(1, 0), 4);
    EXPECT_EQ(state->spellExtraManaPerTarget(1, 0), 1);

    EXPECT_TRUE(state->abilityNeedsTarget(100, 2));
    EXPECT_FALSE(state->abilityNeedsTarget(100, 0));
    EXPECT_TRUE(state->isValidAbilityTarget(100, 2, 200));
    EXPECT_TRUE(state->canAbilityTargetSelf(100, 2));
}

TEST_F(RuledClientTest, RequirementSetsSurviveABatchWithoutLegalActions)
{
    ruled::v1::RuledEventBatch withActions;
    auto &actions = (*withActions.mutable_legal_by_player())[kLocalPlayer];
    actions.add_labels("Cast Grizzly Bears (hand idx 0)");
    actions.add_required_attacker_ids(100); // CR 508.1d
    actions.add_required_blocker_ids(200);  // CR 509.1c
    apply(withActions);
    ASSERT_EQ(state->requiredAttackerOids.size(), 1);

    // A Servatrice-synthesized preview echo has no legal_by_player entry: legal actions clear,
    // but the engine-authoritative must-attack / must-block sets must survive.
    ruled::v1::RuledEventBatch preview;
    auto *ap = preview.add_events()->mutable_attackers_preview();
    ap->set_declaring_player_id(kOpponent);
    ap->add_attacker_object_ids(100);
    apply(preview);

    EXPECT_FALSE(state->isSpellCastLegalForHandIndex(0));
    EXPECT_TRUE(state->requiredAttackerOids.contains(100));
    EXPECT_TRUE(state->requiredBlockerOids.contains(200));
    EXPECT_TRUE(state->remoteAttackerPreviewOids.contains(100));
}

TEST_F(RuledClientTest, LegalActionsBatchEmitsUndoableManaCount)
{
    QSignalSpy spy(state, &RuledClientState::undoableManaAbilitiesChanged);
    ruled::v1::RuledEventBatch batch;
    (*batch.mutable_legal_by_player())[kLocalPlayer].set_undoable_mana_abilities(2);
    apply(batch);
    ASSERT_EQ(spy.count(), 1);
    EXPECT_EQ(spy.at(0).at(0).toInt(), 2);

    // No entry for us this batch → the affordance is retracted.
    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN1, kLocalPlayer));
    ASSERT_EQ(spy.count(), 2);
    EXPECT_EQ(spy.at(1).at(0).toInt(), 0);
}

TEST_F(RuledClientTest, LegalActionsForAnotherPlayerAreIgnored)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kOpponent];
    actions.add_labels("Cast Lightning Bolt (hand idx 1, target)");
    apply(batch);
    EXPECT_FALSE(state->isSpellCastLegalForHandIndex(1));
}

// ---------------------------------------------------------------------------------------
// Stack tracking
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, StackPushAndResolveKeepLifoOrder)
{
    QSignalSpy orderSpy(state, &RuledClientState::stackOrderChanged);
    ruled::v1::RuledEventBatch batch;
    batch.add_events()->mutable_stack_pushed()->set_object_id(10);
    auto *second = batch.add_events()->mutable_stack_pushed();
    second->set_object_id(11);
    second->mutable_targets()->Add()->set_object_id(100);
    apply(batch);

    // Front of the list resolves first.
    EXPECT_EQ(state->getStackOidOrder(), QList<quint32>({11, 10}));
    EXPECT_TRUE(state->hasStackItems());
    EXPECT_EQ(state->stackTargetsByStackOid.value(11), QVector<quint32>({100}));
    EXPECT_EQ(orderSpy.count(), 1);

    ruled::v1::RuledEventBatch resolve;
    resolve.add_events()->mutable_stack_resolved()->set_object_id(11);
    apply(resolve);
    EXPECT_EQ(state->getStackOidOrder(), QList<quint32>({10}));
    EXPECT_FALSE(state->stackTargetsByStackOid.contains(11));
    EXPECT_EQ(host.removedSyntheticCards, QVector<quint32>({11}));
}

TEST_F(RuledClientTest, CounteredSpellLeavesTheStackWithItsCounterspell)
{
    ruled::v1::RuledEventBatch push;
    push.add_events()->mutable_stack_pushed()->set_object_id(10); // the spell
    auto *counter = push.add_events()->mutable_stack_pushed();
    counter->set_object_id(11); // Counterspell targeting it
    counter->mutable_targets()->Add()->set_object_id(10);
    apply(push);
    ASSERT_EQ(state->getStackOidOrder().size(), 2);

    // Only the Counterspell gets a StackResolved; the countered spell must go too.
    ruled::v1::RuledEventBatch resolve;
    resolve.add_events()->mutable_stack_resolved()->set_object_id(11);
    apply(resolve);
    EXPECT_TRUE(state->getStackOidOrder().isEmpty());
    EXPECT_FALSE(state->hasStackItems());
}

TEST_F(RuledClientTest, AbilityOnStackGetsASyntheticCardAndClearsThePendingTrigger)
{
    ruled::v1::RuledEventBatch needsTarget;
    auto *tnt = needsTarget.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Return target creature card from your graveyard to your hand.");
    tnt->set_controller_player_id(kLocalPlayer);
    apply(needsTarget);
    ASSERT_TRUE(state->hasPendingTriggerTarget());
    EXPECT_EQ(state->pendingTriggerSource(), 100u);

    ruled::v1::RuledEventBatch push;
    auto *sp = push.add_events()->mutable_stack_pushed();
    sp->set_object_id(900);
    sp->set_description("Gravedigger ETB");
    sp->set_ability_annotation("Return target creature card...");
    // No card_id => an ability, which has no physical CardItem.
    apply(push);

    EXPECT_FALSE(state->hasPendingTriggerTarget());
    EXPECT_EQ(state->stackSourceOidByStackOid.value(900), 100u);
    ASSERT_EQ(host.createdSyntheticCards.size(), 1);
    EXPECT_EQ(host.createdSyntheticCards[0].oid, 900u);
    EXPECT_EQ(host.createdSyntheticCards[0].name, QStringLiteral("Gravedigger ETB"));
    EXPECT_EQ(host.createdSyntheticCards[0].controllerPlayerId, kLocalPlayer);
    EXPECT_EQ(state->stackAnnotation(900), QStringLiteral("Return target creature card..."));
}

TEST_F(RuledClientTest, TriggerNeedsTargetOnlyPendsForItsController)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_text("Draw a card.");
    tnt->set_controller_player_id(kOpponent);
    apply(batch);
    // We see the trigger, but only its controller may answer it.
    EXPECT_FALSE(state->hasPendingTriggerTarget());
    EXPECT_EQ(state->pendingTriggerController(), kOpponent);
}

TEST_F(RuledClientTest, SpellCopyInheritsTheOriginalPrinting)
{
    host.providerIds.insert(10, QStringLiteral("lea"));
    ruled::v1::RuledEventBatch push;
    push.add_events()->mutable_stack_pushed()->set_object_id(10);
    auto *copy = push.add_events()->mutable_stack_pushed();
    copy->set_object_id(11);
    copy->set_card_id("lightning_bolt");
    copy->set_description("Lightning Bolt (copy)");
    copy->set_ability_annotation("copy");
    copy->set_is_copy(true);
    copy->set_copy_source_object_id(10);
    apply(push);

    ASSERT_EQ(host.createdSyntheticCards.size(), 1);
    EXPECT_EQ(host.createdSyntheticCards[0].oid, 11u);
    EXPECT_EQ(host.createdSyntheticCards[0].setName, QStringLiteral("lea"));
}

TEST_F(RuledClientTest, PhaseChangeEmptiesTheStack)
{
    ruled::v1::RuledEventBatch push;
    push.add_events()->mutable_stack_pushed()->set_object_id(10);
    apply(push);
    ASSERT_TRUE(state->hasStackItems());

    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN2, kLocalPlayer));
    EXPECT_FALSE(state->hasStackItems());
    EXPECT_TRUE(state->stackTargetsByStackOid.isEmpty());
}

// ---------------------------------------------------------------------------------------
// Phase / combat state machine
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, PhaseChangeMapsToToolbarSlotAndCombatPhase)
{
    apply(phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer));
    EXPECT_EQ(host.toolbarPhase, 5);
    EXPECT_EQ(host.activePlayer, kLocalPlayer);
    EXPECT_EQ(state->getCombatPhase(), RuledClientState::RuledCombatPhase::DeclareAttackers);
    EXPECT_TRUE(state->localPlayerIsActive());
    EXPECT_FALSE(state->localPlayerIsDefender());

    // CR 510.4: assign-combat-damage shares the declare-blockers toolbar slot.
    apply(phaseBatch(ruled::v1::PHASE_ID_ASSIGN_COMBAT_DAMAGE, kLocalPlayer));
    EXPECT_EQ(host.toolbarPhase, 6);
    EXPECT_EQ(state->getCombatPhase(), RuledClientState::RuledCombatPhase::AssignCombatDamage);
}

TEST_F(RuledClientTest, FirstStrikeStepTransitionsAreAnnounced)
{
    QSignalSpy spy(state, &RuledClientState::firstStrikeDamageStepActiveChanged);
    apply(phaseBatch(ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE, kLocalPlayer));
    EXPECT_TRUE(state->inFirstStrikeDamageStep());
    ASSERT_EQ(spy.count(), 1);
    EXPECT_TRUE(spy.at(0).at(0).toBool());

    apply(phaseBatch(ruled::v1::PHASE_ID_COMBAT_DAMAGE, kLocalPlayer));
    EXPECT_FALSE(state->inFirstStrikeDamageStep());
    ASSERT_EQ(spy.count(), 2);
    EXPECT_FALSE(spy.at(1).at(0).toBool());
}

TEST_F(RuledClientTest, OpeningPhaseSlugIsRecognised)
{
    apply(phaseBatch(ruled::v1::PHASE_ID_OPENING_MULLIGAN, kLocalPlayer));
    EXPECT_TRUE(state->engineOpeningPhaseActive());
    apply(phaseBatch(ruled::v1::PHASE_ID_UNTAP, kLocalPlayer));
    EXPECT_FALSE(state->engineOpeningPhaseActive());
}

TEST_F(RuledClientTest, AttackerStagingSyncsAPreviewAndClearsOnDeclaration)
{
    apply(phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer));
    ASSERT_TRUE(state->localPlayerIsActive());
    host.sentCommands.clear();

    state->togglePendingAttacker(100);
    EXPECT_TRUE(state->isPendingAttacker(100));
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_TRUE(host.sentCommands[0].has_preview_declare_attackers());
    EXPECT_EQ(host.sentCommands[0].preview_declare_attackers().creature_ids_size(), 1);

    state->togglePendingAttacker(100);
    EXPECT_FALSE(state->isPendingAttacker(100));
    ASSERT_EQ(host.sentCommands.size(), 2);
    EXPECT_EQ(host.sentCommands[1].preview_declare_attackers().creature_ids_size(), 0);

    ruled::v1::RuledEventBatch declared;
    auto *ad = declared.add_events()->mutable_attackers_declared();
    ad->set_attacking_player_id(kLocalPlayer);
    ad->add_attacker_object_ids(100);
    apply(declared);
    EXPECT_TRUE(state->isCurrentAttacker(100));
    EXPECT_TRUE(state->getPendingAttackerOids().isEmpty());
    EXPECT_TRUE(state->hasAttackersSubmittedThisStep());
    // The declare step is over for us: no longer "active" for combat-control purposes.
    EXPECT_FALSE(state->localPlayerIsActive());
}

TEST_F(RuledClientTest, ConfirmAttackersIsGatedOnMustAttackRequirements)
{
    ruled::v1::RuledEventBatch batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_required_attacker_ids(100); // CR 508.1d "attacks if able"
    apply(batch);

    EXPECT_FALSE(state->combatDeclarationSatisfied());
    state->togglePendingAttacker(100);
    EXPECT_TRUE(state->combatDeclarationSatisfied());
}

TEST_F(RuledClientTest, BlockerStagingPairsToAnAttackerAndSyncsAPreview)
{
    ruled::v1::RuledEventBatch declared;
    auto *ad = declared.add_events()->mutable_attackers_declared();
    ad->add_attacker_object_ids(100);
    apply(declared);
    // The opponent is the active player during our declare-blockers step.
    apply(phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kOpponent));
    ASSERT_TRUE(state->localPlayerIsDefender());
    host.sentCommands.clear();

    state->toggleStagedBlocker(200);
    EXPECT_TRUE(state->hasStagedBlocker());
    EXPECT_TRUE(state->isStagedBlocker(200));

    state->pairStagedBlockerToAttacker(100);
    EXPECT_FALSE(state->hasStagedBlocker());
    EXPECT_EQ(state->pendingBlockTargetForBlocker(200), 100u);
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_TRUE(host.sentCommands[0].has_preview_declare_blockers());
    ASSERT_EQ(host.sentCommands[0].preview_declare_blockers().block_pairs_size(), 1);
    EXPECT_EQ(host.sentCommands[0].preview_declare_blockers().block_pairs(0).blocker_id(), 200u);

    // Pairing to a creature that is not a declared attacker is a no-op.
    state->toggleStagedBlocker(201);
    state->pairStagedBlockerToAttacker(999);
    EXPECT_EQ(state->pendingBlockTargetForBlocker(201), 0u);
}

TEST_F(RuledClientTest, RejectedBlockDeclarationRollsBackTheLocalGuard)
{
    ruled::v1::RuledEventBatch declared;
    declared.add_events()->mutable_attackers_declared()->add_attacker_object_ids(100);
    apply(declared);
    apply(phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kOpponent));
    state->toggleStagedBlocker(200);
    state->pairStagedBlockerToAttacker(100);
    host.sentCommands.clear();

    QSignalSpy rejectedSpy(state, &RuledClientState::blockerRejected);
    state->confirmBlockers();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_TRUE(host.sentCommands[0].has_declare_blockers());
    // Optimistically committed while the round-trip is in flight.
    EXPECT_TRUE(state->hasBlockersSubmittedThisStep());
    EXPECT_EQ(state->getCommittedBlocks().value(200), 100u);

    host.answerPendingAck(false);
    EXPECT_EQ(rejectedSpy.count(), 1);
    EXPECT_FALSE(state->hasBlockersSubmittedThisStep());
    // The staged pairs come back so the defender can fix and resubmit.
    EXPECT_EQ(state->getPendingBlocks().value(200), 100u);
    EXPECT_TRUE(state->getCommittedBlocks().isEmpty());
}

TEST_F(RuledClientTest, RemovedFromCombatPrunesAttackersAndBlockPairs)
{
    ruled::v1::RuledEventBatch setup;
    auto *ad = setup.add_events()->mutable_attackers_declared();
    ad->add_attacker_object_ids(100);
    ad->add_attacker_object_ids(101);
    auto *bd = setup.add_events()->mutable_blockers_declared();
    auto *pair = bd->add_block_pairs();
    pair->set_attacker_id(100);
    pair->set_blocker_id(200);
    apply(setup);
    ASSERT_TRUE(state->isCurrentAttacker(100));
    ASSERT_EQ(state->getCommittedBlocks().value(200), 100u);

    // CR 701.15a: regeneration removes the blocker from combat.
    ruled::v1::RuledEventBatch removed;
    removed.add_events()->mutable_removed_from_combat()->add_object_ids(200);
    apply(removed);
    EXPECT_TRUE(state->getCommittedBlocks().isEmpty());
    EXPECT_FALSE(state->committedBlockerGroups.contains(100));
    EXPECT_TRUE(state->isCurrentAttacker(100));

    ruled::v1::RuledEventBatch removedAttacker;
    removedAttacker.add_events()->mutable_removed_from_combat()->add_object_ids(101);
    apply(removedAttacker);
    EXPECT_FALSE(state->isCurrentAttacker(101));
}

TEST_F(RuledClientTest, StalePairsArePrunedWhenPermanentsLeaveTheBattlefield)
{
    ruled::v1::RuledEventBatch setup;
    auto *bd = setup.add_events()->mutable_blockers_declared();
    auto *pair = bd->add_block_pairs();
    pair->set_attacker_id(100);
    pair->set_blocker_id(200);
    apply(setup);
    ASSERT_EQ(state->getCommittedBlocks().value(200), 100u);

    // A battlefield map that no longer lists the blocker prunes the pair.
    ruled::v1::RuledEventBatch mapOnly;
    addPermanent(mapOnly.add_events(), kLocalPlayer, 100, 7);
    apply(mapOnly);
    EXPECT_TRUE(state->getCommittedBlocks().isEmpty());
}

// ---------------------------------------------------------------------------------------
// Combat damage assignment (CR 510.1a-d, 702.19)
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, MultipleBlockersQueueAnAssignmentSeededLethalFirst)
{
    apply(phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kLocalPlayer));
    ruled::v1::RuledEventBatch batch;
    // A 5/5 attacker blocked by a 2/2 and a 3/3.
    auto *ev = batch.add_events();
    addPermanent(ev, kLocalPlayer, 100, 1);
    addPermanent(ev, kOpponent, 200, 2);
    addPermanent(ev, kOpponent, 201, 3);
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    for (const quint32 oid : {100u, 200u, 201u}) {
        view->add_battlefield_object_id(oid);
    }
    view->add_battlefield_power(5);
    view->add_battlefield_power(2);
    view->add_battlefield_power(3);
    view->add_battlefield_toughness(5);
    view->add_battlefield_toughness(2);
    view->add_battlefield_toughness(3);
    auto *bd = batch.add_events()->mutable_blockers_declared();
    for (const quint32 blocker : {200u, 201u}) {
        auto *pair = bd->add_block_pairs();
        pair->set_attacker_id(100);
        pair->set_blocker_id(blocker);
    }
    apply(batch);

    EXPECT_EQ(state->currentCombatDamageAttackerOid(), 100u);
    EXPECT_EQ(state->currentCombatDamageAttackerPower(), 5);
    // Greedy lethal-first: 2 to the 2/2, the remaining 3 to the last blocker.
    EXPECT_EQ(state->assignedCombatDamageForBlocker(200), 2u);
    EXPECT_EQ(state->assignedCombatDamageForBlocker(201), 3u);
    EXPECT_EQ(state->localCombatDamageAssignedTotal(), 5);
    EXPECT_TRUE(state->localCombatDamageAssignmentLegal());

    // Reducing below the attacker's power makes it illegal again (no trample here).
    state->bumpBlockerCombatDamage(201, -1);
    EXPECT_EQ(state->localCombatDamageAssignedTotal(), 4);
    EXPECT_FALSE(state->localCombatDamageAssignmentLegal());
    // Bumping never exceeds the attacker's power.
    state->bumpBlockerCombatDamage(201, +99);
    EXPECT_EQ(state->assignedCombatDamageForBlocker(201), 5u);
}

TEST_F(RuledClientTest, TrampleAssignsLethalToBlockerAndTheRemainderToThePlayer)
{
    apply(phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kLocalPlayer));
    ruled::v1::RuledEventBatch batch;
    auto *ev = batch.add_events();
    addPermanent(ev, kLocalPlayer, 100, 1)->set_trample(true); // CR 702.19
    addPermanent(ev, kOpponent, 200, 2);
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    view->add_battlefield_object_id(100);
    view->add_battlefield_object_id(200);
    view->add_battlefield_power(6);
    view->add_battlefield_power(1);
    view->add_battlefield_toughness(6);
    view->add_battlefield_toughness(2);
    auto *pair = batch.add_events()->mutable_blockers_declared()->add_block_pairs();
    pair->set_attacker_id(100);
    pair->set_blocker_id(200);
    apply(batch);

    // A single blocker still needs an explicit assignment when the attacker tramples.
    EXPECT_EQ(state->currentCombatDamageAttackerOid(), 100u);
    EXPECT_EQ(state->assignedCombatDamageForBlocker(200), 2u);
    EXPECT_EQ(state->localCombatDamagePlayerDamage(), 4);
    EXPECT_TRUE(state->localCombatDamageAssignmentLegal());

    host.sentCommands.clear();
    state->confirmCombatDamageForCurrentAttacker();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &acd = host.sentCommands[0].assign_combat_damage();
    EXPECT_EQ(acd.attacker_id(), 100u);
    ASSERT_EQ(acd.assignments_size(), 1);
    EXPECT_EQ(acd.assignments(0).blocker_id(), 200u);
    EXPECT_EQ(acd.assignments(0).damage(), 2u);
    EXPECT_EQ(acd.defending_player_damage(), 4u);

    // Assigning less than lethal to the blocker is illegal under CR 702.19.
    state->bumpBlockerCombatDamage(200, -1);
    EXPECT_FALSE(state->localCombatDamageAssignmentLegal());
}

TEST_F(RuledClientTest, CombatPowerFallsBackToTheHostWhenZoneViewIsAbsent)
{
    host.fallbackPt.insert(100, {4, 4});
    EXPECT_EQ(state->combatPowerForCreatureOid(100), 4);
    EXPECT_EQ(state->combatToughnessForCreatureOid(100), 4);
    // Nothing known at all: power 0, toughness 1 (the documented floor).
    EXPECT_EQ(state->combatPowerForCreatureOid(999), 0);
    EXPECT_EQ(state->combatToughnessForCreatureOid(999), 1);
}

// ---------------------------------------------------------------------------------------
// Zone view: marked damage, activated abilities, first-strike flag
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, ZoneViewParsesDamageAndPipeDelimitedAbilities)
{
    QSignalSpy fsSpy(state, &RuledClientState::firstStrikeStepPendingChanged);
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    view->set_first_strike_step_pending(true);
    view->add_battlefield_object_id(100);
    view->add_battlefield_damage(3);
    view->add_battlefield_activated_ability_texts("Add {G}.|Sacrifice this: draw a card.");
    view->add_battlefield_activated_ability_mana_costs("|1");
    view->add_battlefield_activated_ability_mana_produced("G|");
    view->add_battlefield_activated_ability_cost_labels("{T}|Sacrifice this");
    apply(batch);

    EXPECT_EQ(state->markedDamageForEngineOid(100), 3);
    EXPECT_EQ(state->activatedAbilitiesForOid(100),
              QStringList({QStringLiteral("Add {G}."), QStringLiteral("Sacrifice this: draw a card.")}));
    EXPECT_EQ(state->activatedAbilityManaCostsForOid(100), QStringList({QString(), QStringLiteral("1")}));
    // CR 605: an empty produced entry marks a non-mana ability.
    EXPECT_EQ(state->activatedAbilityManaProducedForOid(100), QStringList({QStringLiteral("G"), QString()}));
    EXPECT_EQ(state->activatedAbilityCostLabelsForOid(100),
              QStringList({QStringLiteral("{T}"), QStringLiteral("Sacrifice this")}));

    EXPECT_TRUE(state->isFirstStrikeStepPending());
    ASSERT_EQ(fsSpy.count(), 1);
    EXPECT_TRUE(fsSpy.at(0).at(0).toBool());

    // The flag only re-announces on a change.
    apply(batch);
    EXPECT_EQ(fsSpy.count(), 1);
}

// ---------------------------------------------------------------------------------------
// Tier-3 resolution choices (CR 608)
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, HandCardsChoiceStartsAClickToPickAndSubmitsInClickOrder)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS); // Brainstorm
    rcr->set_prompt_text("Put two cards from your hand on top of your library.");
    rcr->set_min(2);
    rcr->set_max(2);
    rcr->set_ordered(true);
    for (const quint32 oid : {11u, 12u, 13u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {1, 2, 3}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Hand);
    EXPECT_EQ(state->resolutionHandPickRequired(), 2);
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(1));
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(99));

    state->toggleResolutionHandPickCard(3);
    state->toggleResolutionHandPickCard(1);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(3), 1);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(1), 2);
    // Capped at max.
    state->toggleResolutionHandPickCard(2);
    EXPECT_EQ(state->resolutionHandPickSelected(), 2);

    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &sub = host.sentCommands[0].submit_resolution_choice();
    ASSERT_EQ(sub.chosen_object_ids_size(), 2);
    // Click order is preserved — Brainstorm's ordering is load-bearing.
    EXPECT_EQ(sub.chosen_object_ids(0), 13u);
    EXPECT_EQ(sub.chosen_object_ids(1), 11u);
    EXPECT_FALSE(state->isResolutionHandPickActive());
}

TEST_F(RuledClientTest, SubmitIsRefusedBelowTheMinimum)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    rcr->set_min(2);
    rcr->set_max(2);
    rcr->add_candidate_object_ids(11);
    rcr->add_candidate_server_card_ids(1);
    apply(batch);

    state->toggleResolutionHandPickCard(1);
    host.sentCommands.clear();
    state->submitResolutionHandPick();
    EXPECT_TRUE(host.sentCommands.isEmpty());
    EXPECT_TRUE(state->isResolutionHandPickActive());
}

TEST_F(RuledClientTest, LibrarySearchChoiceEnforcesUniqueNamesAndOpensTheDeckView)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH); // Gifts Ungiven step 1
    rcr->set_min(4);
    rcr->set_max(4);
    rcr->set_unique_names(true);
    for (const quint32 oid : {11u, 12u, 13u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {1, 2, 3}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    rcr->add_candidate_names("Forest");
    rcr->add_candidate_names("Forest");
    rcr->add_candidate_names("Island");
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    ASSERT_EQ(started.count(), 1);
    EXPECT_EQ(started.at(0).at(0).toStringList(),
              QStringList({QStringLiteral("Forest"), QStringLiteral("Forest"), QStringLiteral("Island")}));

    state->toggleResolutionHandPickCard(1); // Forest
    // The second Forest is no longer selectable ("four cards with different names").
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(2));
    state->toggleResolutionHandPickCard(2);
    EXPECT_EQ(state->resolutionHandPickSelected(), 1);
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(3));
    state->toggleResolutionHandPickCard(3);
    EXPECT_EQ(state->resolutionHandPickSelected(), 2);
}

TEST_F(RuledClientTest, RevealedChoiceAnnouncesAndClosesThePopup)
{
    QSignalSpy revealed(state, &RuledClientState::revealedPickChanged);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_REVEALED); // Gifts Ungiven step 2
    rcr->set_min(2);
    rcr->set_max(2);
    for (const quint32 oid : {11u, 12u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {1, 2}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    rcr->add_candidate_names("Forest");
    rcr->add_candidate_names("Island");
    apply(batch);

    ASSERT_EQ(revealed.count(), 1);
    EXPECT_TRUE(revealed.at(0).at(0).toBool());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Revealed);

    state->toggleResolutionHandPickCard(1);
    state->toggleResolutionHandPickCard(2);
    state->submitResolutionHandPick();
    ASSERT_EQ(revealed.count(), 2);
    EXPECT_FALSE(revealed.at(1).at(0).toBool());
}

TEST_F(RuledClientTest, TargetObjectAndLegendKeepChoicesUseClickToSelect)
{
    {
        ruled::v1::RuledEventBatch batch;
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_TARGET_OBJECTS); // CR 707.10c copy retarget
        rcr->set_prompt_text("Choose new targets for the copy.");
        rcr->add_candidate_object_ids(100);
        apply(batch);
        ASSERT_TRUE(state->hasPendingCopyTargetChoice());
        EXPECT_TRUE(state->isValidCopyTarget(100));
        EXPECT_FALSE(state->isValidCopyTarget(101));
        EXPECT_FALSE(state->isResolutionHandPickActive()); // no list dialog for this kind
        EXPECT_EQ(host.dialogRequests, 0);

        host.sentCommands.clear();
        state->submitCopyTargetChoice(100);
        ASSERT_EQ(host.sentCommands.size(), 1);
        EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 100u);
        EXPECT_FALSE(state->hasPendingCopyTargetChoice());
    }
    {
        ruled::v1::RuledEventBatch batch;
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LEGEND_KEEP); // CR 704.5j legend rule
        rcr->add_candidate_object_ids(100);
        rcr->add_candidate_object_ids(101);
        apply(batch);
        ASSERT_TRUE(state->hasPendingLegendKeepChoice());
        EXPECT_TRUE(state->isValidLegendKeepTarget(101));

        host.sentCommands.clear();
        state->submitLegendKeepChoice(101);
        ASSERT_EQ(host.sentCommands.size(), 1);
        EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 101u);
        EXPECT_FALSE(state->hasPendingLegendKeepChoice());
    }
}

TEST_F(RuledClientTest, UnrecognisedChoiceKindFallsBackToTheModalDialog)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS); // no parallel server card ids
    rcr->set_prompt_text("Choose one.");
    rcr->add_candidate_object_ids(11);
    rcr->add_candidate_names("Forest");
    apply(batch);

    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(host.dialogRequests, 1);
    EXPECT_EQ(host.lastDialogPrompt, QStringLiteral("Choose one."));
}

TEST_F(RuledClientTest, ChoicesForAnotherPlayerNeverPromptUs)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kOpponent);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    rcr->add_candidate_object_ids(11);
    rcr->add_candidate_server_card_ids(1);
    apply(batch);

    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(host.dialogRequests, 0);
}

// ---------------------------------------------------------------------------------------
// Opening sequence commands
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, OpeningBottomSendsIndicesAdjustedForPriorRemovals)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_labels("Keep opening hand (opening)");
    for (int i = 0; i < 7; ++i) {
        actions.add_labels("Put Card on bottom (opening, hand idx " + std::to_string(i) + ")");
    }
    apply(batch);
    ASSERT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::BottomLibrary);

    // One mulligan taken → one card must go on the bottom (London mulligan).
    host.sentCommands.clear();
    state->openingMulliganRedraw();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_TRUE(host.sentCommands[0].has_mulligan());
    EXPECT_FALSE(host.sentCommands[0].mulligan().keep());
    apply(batch); // engine re-offers the bottoming labels after the redraw
    ASSERT_EQ(state->openingBottomRequiredCount(), 1);

    state->toggleOpeningBottomHandIndex(2);
    EXPECT_EQ(state->openingBottomClickOrderFor(2), 1);
    host.sentCommands.clear();
    state->openingBottomDone();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].put_opening_hand_on_bottom().hand_card_index(), 2u);
}

TEST_F(RuledClientTest, OpeningBottomAdjustsLaterIndicesForEarlierRemovals)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    for (int i = 0; i < 7; ++i) {
        actions.add_labels("Put Card on bottom (opening, hand idx " + std::to_string(i) + ")");
    }
    apply(batch);
    state->openingMulliganRedraw();
    state->openingMulliganRedraw();
    apply(batch);
    ASSERT_EQ(state->openingBottomRequiredCount(), 2);

    // Click low index first: the later index shifts down by one when it is sent.
    state->toggleOpeningBottomHandIndex(1);
    state->toggleOpeningBottomHandIndex(4);
    host.sentCommands.clear();
    state->openingBottomDone();
    ASSERT_EQ(host.sentCommands.size(), 2);
    EXPECT_EQ(host.sentCommands[0].put_opening_hand_on_bottom().hand_card_index(), 1u);
    EXPECT_EQ(host.sentCommands[1].put_opening_hand_on_bottom().hand_card_index(), 3u);
}

TEST_F(RuledClientTest, ChooseStartingPlayerAndKeepSendTheirCommands)
{
    host.sentCommands.clear();
    state->openingPickFirstSeat(kOpponent);
    state->openingMulliganKeep();
    ASSERT_EQ(host.sentCommands.size(), 2);
    EXPECT_EQ(host.sentCommands[0].choose_starting_player().starting_player_id(), kOpponent);
    EXPECT_TRUE(host.sentCommands[1].mulligan().keep());
}

// ---------------------------------------------------------------------------------------
// Session lifecycle
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, ClearSessionStateResetsEverythingCarriedBetweenGames)
{
    ruled::v1::RuledEventBatch batch;
    auto *sp = batch.add_events()->mutable_stack_pushed();
    sp->set_object_id(900);
    sp->set_ability_annotation("some ability");
    auto *gy = batch.add_events()->mutable_graveyard_object_map()->add_entries();
    gy->set_engine_object_id(500);
    gy->set_server_card_id(11);
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_TARGET_OBJECTS);
    rcr->add_candidate_object_ids(100);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_labels("Cast Grizzly Bears (hand idx 0)");
    apply(batch);
    ASSERT_TRUE(state->hasStackItems());
    ASSERT_TRUE(state->hasPendingCopyTargetChoice());

    QSignalSpy resetSpy(state, &RuledClientState::sessionReset);
    state->clearSessionState();

    EXPECT_EQ(resetSpy.count(), 1);
    EXPECT_FALSE(state->hasStackItems());
    EXPECT_TRUE(state->stackAnnotation(900).isEmpty());
    EXPECT_FALSE(state->hasPendingCopyTargetChoice());
    EXPECT_FALSE(state->hasPendingTriggerTarget());
    EXPECT_FALSE(state->isSpellCastLegalForHandIndex(0));
    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::None);
    EXPECT_EQ(state->getOpeningMulliganCount(), 0);
    // Phantom graveyard targets are the reason this map must not survive a game boundary.
    EXPECT_EQ(state->graveyardEngineOidForServerCardId(11), 0u);
}

TEST_F(RuledClientTest, EveryBatchSchedulesAnArrowResync)
{
    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN1, kLocalPlayer));
    EXPECT_EQ(host.arrowSyncRequests, 1);
    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN2, kLocalPlayer));
    EXPECT_EQ(host.arrowSyncRequests, 2);
}

TEST_F(RuledClientTest, MalformedPayloadIsRejectedWithoutCrashing)
{
    EXPECT_FALSE(dispatcher->processPayload(std::string("\xff\xff\xff\xff", 4)));
}

} // namespace
