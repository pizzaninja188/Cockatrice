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
#include "game/ruled/ruled_auto_pass_policy.h"
#include "game/ruled/ruled_client_state.h"
#include "game/ruled/ruled_dev_command_parser.h"
#include "game/ruled/ruled_event_dispatcher.h"
#include "game/ruled/ruled_pending_cast.h"

#include <QSignalSpy>
#include <QString>
#include <QTest>
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
    bool autoSubmitDialogChoice = false;

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
                                       const QVector<quint32> &candidateOids,
                                       const QStringList &,
                                       int,
                                       int,
                                       bool,
                                       bool) override
    {
        ++dialogRequests;
        lastDialogPrompt = prompt;
        if (autoSubmitDialogChoice && !candidateOids.isEmpty()) {
            ruled::v1::RuledCommand command;
            command.mutable_submit_resolution_choice()->add_chosen_object_ids(candidateOids.first());
            sendRuledCommand(command);
        }
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

    static ruled::v1::LegalHandAction *addHandAction(ruled::v1::LegalActions &actions,
                                                     ruled::v1::HandActionKind kind,
                                                     int handIndex,
                                                     const std::string &cardName,
                                                     int faceIndex = 0,
                                                     bool needsTarget = false,
                                                     const std::string &cost = {})
    {
        auto *action = actions.add_hand_actions();
        action->set_kind(kind);
        action->set_hand_index(static_cast<quint32>(handIndex));
        action->set_card_name(cardName);
        action->set_face_index(static_cast<quint32>(faceIndex));
        action->set_needs_target(needsTarget);
        action->set_cost(cost);
        return action;
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
    e1->add_keywords("Haste");
    auto *e2 = addPermanent(ev, kOpponent, 200, 9);
    e2->add_keywords("Trample");
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

TEST(RuledAutoPassPolicyTest, MapsToolbarStopsAndSharesCombatDamageStop)
{
    std::array<bool, 11> own{};
    std::array<bool, 11> opponent{};
    own[2] = true;      // Draw
    own[7] = true;      // Combat damage (both CR 510.4 steps)
    opponent[4] = true; // Beginning of combat

    const ruled::v1::SetAutoPassPolicy policy = RuledAutoPassPolicy::fromToolbarStops(own, opponent);
    EXPECT_EQ(policy.stop_on_own_turn_size(), 3);
    EXPECT_EQ(policy.stop_on_own_turn(0), ruled::v1::PHASE_ID_DRAW);
    EXPECT_EQ(policy.stop_on_own_turn(1), ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE);
    EXPECT_EQ(policy.stop_on_own_turn(2), ruled::v1::PHASE_ID_COMBAT_DAMAGE);
    ASSERT_EQ(policy.stop_on_opponent_turn_size(), 1);
    EXPECT_EQ(policy.stop_on_opponent_turn(0), ruled::v1::PHASE_ID_BEGIN_COMBAT);
}

TEST_F(RuledClientTest, EngineCommandPendingLocksImmediatelyAndRejectsDuplicateBegin)
{
    QSignalSpy spy(state, &RuledClientState::engineCommandPendingUiChanged);
    EXPECT_TRUE(state->beginEngineCommand());
    EXPECT_TRUE(state->isEngineCommandPending());
    EXPECT_FALSE(state->isEngineCommandIndicatorVisible());
    EXPECT_FALSE(state->beginEngineCommand());
    EXPECT_EQ(spy.count(), 1);

    state->showEngineCommandIndicator();
    EXPECT_TRUE(state->isEngineCommandIndicatorVisible());
    EXPECT_EQ(spy.count(), 2);

    state->finishEngineCommand();
    EXPECT_FALSE(state->isEngineCommandPending());
    EXPECT_FALSE(state->isEngineCommandIndicatorVisible());
    EXPECT_EQ(spy.count(), 3);
}

TEST_F(RuledClientTest, EngineCommandIndicatorAppearsOnlyAfterDelay)
{
    ASSERT_TRUE(state->beginEngineCommand());
    QTest::qWait(100);
    EXPECT_FALSE(state->isEngineCommandIndicatorVisible());
    QTRY_VERIFY_WITH_TIMEOUT(state->isEngineCommandIndicatorVisible(), 200);

    state->finishEngineCommand();
}

TEST_F(RuledClientTest, FinishedCommandCancelsItsDelayedIndicator)
{
    ASSERT_TRUE(state->beginEngineCommand());
    state->finishEngineCommand();
    QTest::qWait(175);
    EXPECT_FALSE(state->isEngineCommandPending());
    EXPECT_FALSE(state->isEngineCommandIndicatorVisible());
}

TEST_F(RuledClientTest, StaleIndicatorCallbackCannotAffectLaterCommand)
{
    ASSERT_TRUE(state->beginEngineCommand());
    QTest::qWait(50);
    state->finishEngineCommand();
    ASSERT_TRUE(state->beginEngineCommand());

    // The first command's 150 ms callback has now fired, but the second command has not reached
    // its own threshold. Its generation token must keep the new prompt unchanged.
    QTest::qWait(110);
    EXPECT_TRUE(state->isEngineCommandPending());
    EXPECT_FALSE(state->isEngineCommandIndicatorVisible());
    QTRY_VERIFY_WITH_TIMEOUT(state->isEngineCommandIndicatorVisible(), 100);
    state->finishEngineCommand();
}

TEST(RuledPendingPaymentTest, LastManaPipConsumedDuringEngineCommandIsReadyAfterUnlock)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForTarget = false;
    spell.remainingCost.insert(QChar('G'), 0);
    EXPECT_EQ(readyRuledPendingPaymentAction(spell, {}), RuledPendingPaymentAction::CastSpell);

    spell.remainingCost[QChar('G')] = 1;
    EXPECT_EQ(readyRuledPendingPaymentAction(spell, {}), RuledPendingPaymentAction::None);

    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = false;
    ability.waitingForMana = false; // the last-pip path clears this before command submission
    EXPECT_EQ(readyRuledPendingPaymentAction({}, ability), RuledPendingPaymentAction::ActivateAbility);
}

TEST_F(RuledClientTest, HandSlotAndPublicZoneMapsAreQueryable)
{
    ruled::v1::RuledEventBatch batch;
    auto *hs = batch.add_events()->mutable_hand_slot_map();
    auto *he = hs->add_entries();
    he->set_player_id(kOpponent);
    he->set_server_card_id(42);
    he->set_hand_index(3);
    auto *gy = batch.add_events()->mutable_graveyard_object_map();
    auto *ge = gy->add_entries();
    ge->set_player_id(kLocalPlayer);
    ge->set_engine_object_id(500);
    ge->set_server_card_id(11);
    auto *xe = batch.add_events()->mutable_exile_object_map()->add_entries();
    xe->set_player_id(kOpponent);
    xe->set_engine_object_id(700);
    xe->set_server_card_id(13);
    apply(batch);

    EXPECT_EQ(state->engineHandSlotForServerCard(kOpponent, 42), 3);
    EXPECT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 42), -1);
    EXPECT_EQ(state->graveyardEngineOidForOwnedCard(kLocalPlayer, 11), 500u);
    EXPECT_EQ(state->graveyardEngineOidForOwnedCard(kLocalPlayer, 12), 0u);
    // Card ids repeat across players' zones, so the owner has to be part of the key.
    EXPECT_EQ(state->graveyardEngineOidForOwnedCard(kOpponent, 11), 0u);
    EXPECT_EQ(state->exileEngineOidForOwnedCard(kOpponent, 13), 700u);
    EXPECT_EQ(state->exileEngineOidForOwnedCard(kLocalPlayer, 13), 0u);
}

TEST_F(RuledClientTest, HandSlotMapPersistsUntilReplaced)
{
    ruled::v1::RuledEventBatch first;
    auto *he = first.add_events()->mutable_hand_slot_map()->add_entries();
    he->set_player_id(kLocalPlayer);
    he->set_server_card_id(5);
    he->set_hand_index(0);
    apply(first);
    ASSERT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 5), 0);

    // Servatrice omits the map on batches with no hand change, so an absent map means "unchanged"
    // and the slot has to survive.
    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN1, kLocalPlayer));
    EXPECT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 5), 0);

    // A present map is a full replacement: card 5 left the hand, card 7 took slot 0.
    ruled::v1::RuledEventBatch second;
    auto *he2 = second.add_events()->mutable_hand_slot_map()->add_entries();
    he2->set_player_id(kLocalPlayer);
    he2->set_server_card_id(7);
    he2->set_hand_index(0);
    apply(second);
    EXPECT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 5), -1);
    EXPECT_EQ(state->engineHandSlotForServerCard(kLocalPlayer, 7), 0);
}

// ---------------------------------------------------------------------------------------
// Structured legal hand actions — one case per kind
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, AppliesStructuredLandActionsIncludingMdfcFaces)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_PLAY_LAND, 2, "Forest");
    addHandAction(actions, ruled::v1::HAND_ACTION_PLAY_LAND, 4, "Cragcrown Pathway", 0);
    addHandAction(actions, ruled::v1::HAND_ACTION_PLAY_LAND, 4, "Timbercrown Pathway", 1);
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 1, "Lightning Bolt", 0, true);
    apply(batch);

    constexpr auto kLand = ruled::v1::HAND_ACTION_PLAY_LAND;
    EXPECT_TRUE(state->isHandActionLegal(kLand, 2));
    EXPECT_TRUE(state->isHandActionLegal(kLand, 4));
    EXPECT_FALSE(state->isHandActionLegal(kLand, 3));
    EXPECT_EQ(state->handActionIndicesForCardName(kLand, "Forest"), QList<int>({2}));
    // A cast action never lands in the PlayLand set.
    EXPECT_FALSE(state->isHandActionLegal(kLand, 1));
    EXPECT_TRUE(state->isHandActionLegal(ruled::v1::HAND_ACTION_CAST_SPELL, 1));

    // CR 712: one hand slot, two playable faces, sorted by face index.
    const QVector<RuledFaceOption> faces = state->handActionFaceOptions(kLand, 4);
    ASSERT_EQ(faces.size(), 2);
    EXPECT_EQ(faces[0].faceIndex, 0);
    EXPECT_EQ(faces[0].faceName, QStringLiteral("Cragcrown Pathway"));
    EXPECT_EQ(faces[1].faceIndex, 1);
    EXPECT_EQ(faces[1].faceName, QStringLiteral("Timbercrown Pathway"));
    // A single-face land still reports exactly one option, at face 0.
    ASSERT_EQ(state->handActionFaceOptions(kLand, 2).size(), 1);
    EXPECT_EQ(state->handActionFaceOptions(kLand, 2)[0].faceIndex, 0);
}

TEST_F(RuledClientTest, AppliesStructuredCastActionsAndTargetRequirement)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 1, "Lightning Bolt", 0, true);
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 3, "Llanowar Elves");
    apply(batch);

    constexpr auto kCast = ruled::v1::HAND_ACTION_CAST_SPELL;
    EXPECT_TRUE(state->isHandActionLegal(kCast, 1));
    EXPECT_TRUE(state->handActionNeedsTarget(kCast, 1));
    EXPECT_TRUE(state->isHandActionLegal(kCast, 3));
    EXPECT_FALSE(state->handActionNeedsTarget(kCast, 3));
    EXPECT_EQ(state->handActionIndexForCard(kCast, "Llanowar Elves", 99), 3);
    EXPECT_EQ(state->handActionIndexForCard(kCast, "Nonexistent", 0), -1);
}

TEST_F(RuledClientTest, AdventureCastFacesCarryEngineNamesAndCosts)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 6, "Bonecrusher Giant", 0, false, "{2}{R}");
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 6, "Stomp", 1, true, "{1}{R}");
    apply(batch);

    const QVector<RuledFaceOption> faces =
        state->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, 6);
    ASSERT_EQ(faces.size(), 2);
    EXPECT_EQ(faces[0].faceIndex, 0);
    EXPECT_EQ(faces[0].faceName, QStringLiteral("Bonecrusher Giant"));
    EXPECT_EQ(faces[0].manaCost, QStringLiteral("{2}{R}"));
    EXPECT_FALSE(state->handActionNeedsTarget(ruled::v1::HAND_ACTION_CAST_SPELL, 6, 0));
    EXPECT_EQ(faces[1].faceIndex, 1);
    EXPECT_EQ(faces[1].faceName, QStringLiteral("Stomp"));
    EXPECT_EQ(faces[1].manaCost, QStringLiteral("{1}{R}"));
    EXPECT_TRUE(state->handActionNeedsTarget(ruled::v1::HAND_ACTION_CAST_SPELL, 6, 1));
}

TEST_F(RuledClientTest, AppliesAuthoritativeModalModeDataPerFace)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *cast = addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 5, "Boros Charm");
    cast->set_min_modes(1);
    cast->set_max_modes(1);
    auto *damage = cast->add_modes();
    damage->set_mode_index(0);
    damage->set_label("Deal 4 damage");
    damage->set_selectable(true);
    damage->set_needs_target(true);
    damage->mutable_targets()->set_can_target_opponent(true);
    auto *disabled = cast->add_modes();
    disabled->set_mode_index(2);
    disabled->set_label("Creature gains double strike");
    disabled->set_selectable(false);
    disabled->set_needs_target(true);
    apply(batch);

    const auto &set = state->handActions[ruled::v1::HAND_ACTION_CAST_SPELL];
    const int key = RuledClientState::spellTargetKey(5, 0);
    ASSERT_TRUE(set.modalOptionsByCastKey.contains(key));
    EXPECT_EQ(set.modalMinModesByCastKey.value(key), 1);
    EXPECT_EQ(set.modalMaxModesByCastKey.value(key), 1);
    const auto modes = set.modalOptionsByCastKey.value(key);
    ASSERT_EQ(modes.size(), 2);
    EXPECT_EQ(modes[0].modeIndex, 0);
    EXPECT_TRUE(modes[0].targets.canTargetOpponent);
    EXPECT_FALSE(modes[1].selectable);
}

TEST_F(RuledClientTest, AppliesStructuredCleanupDiscardActionsAndRequiredCount)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    // CR 514.1: nine cards in hand means two must be discarded.
    for (int i = 0; i < 9; ++i) {
        addHandAction(actions, ruled::v1::HAND_ACTION_CLEANUP_DISCARD, i, "Card" + std::to_string(i));
    }
    apply(batch);

    EXPECT_TRUE(state->localPlayerMustCleanupDiscard());
    EXPECT_EQ(state->cleanupDiscardRequiredCount(), 2);
    EXPECT_EQ(state->cleanupDiscardSelectedCount(), 0);
    // Cleanup clicks resolve by authoritative hand-slot identity. A client display-name mismatch
    // must not empty the candidate list and make the required discard unclickable.
    EXPECT_EQ(state->handActionClickCandidates(ruled::v1::HAND_ACTION_CLEANUP_DISCARD, "Different display name"),
              QList<int>({0, 1, 2, 3, 4, 5, 6, 7, 8}));

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
        addHandAction(actions, ruled::v1::HAND_ACTION_OPENING_BOTTOM, 0, "Forest");
        addHandAction(actions, ruled::v1::HAND_ACTION_OPENING_BOTTOM, 5, "Mountain");
        apply(batch);
        // The bottoming step wins over the mulligan prompt when both labels are present.
        EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::BottomLibrary);
        constexpr auto kBottom = ruled::v1::HAND_ACTION_OPENING_BOTTOM;
        EXPECT_TRUE(state->isHandActionLegal(kBottom, 0));
        EXPECT_TRUE(state->isHandActionLegal(kBottom, 5));
        EXPECT_EQ(state->handActionLegalIndicesSorted(kBottom), QList<int>({0, 5}));
        // The label's card name is captured too, so a name-keyed lookup works for every kind.
        EXPECT_EQ(state->handActionIndicesForCardName(kBottom, "Mountain"), QList<int>({5}));
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
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 0, "Grizzly Bears");
    actions.add_required_attacker_ids(100); // CR 508.1d
    actions.add_required_blocker_ids(200);  // CR 509.1c
    actions.add_selectable_attacker_ids(100);
    actions.add_selectable_blocker_ids(200);
    apply(withActions);
    ASSERT_EQ(state->requiredAttackerOids.size(), 1);

    // A Servatrice-synthesized preview echo has no legal_by_player entry: legal actions clear,
    // but the engine-authoritative must-attack / must-block sets must survive.
    ruled::v1::RuledEventBatch preview;
    auto *ap = preview.add_events()->mutable_attackers_preview();
    ap->set_declaring_player_id(kOpponent);
    ap->add_attacker_object_ids(100);
    apply(preview);

    EXPECT_FALSE(state->isHandActionLegal(ruled::v1::HAND_ACTION_CAST_SPELL, 0));
    EXPECT_TRUE(state->requiredAttackerOids.contains(100));
    EXPECT_TRUE(state->requiredBlockerOids.contains(200));
    EXPECT_TRUE(state->isSelectableAttacker(100));
    EXPECT_TRUE(state->isSelectableBlocker(200));
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
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 1, "Lightning Bolt", 0, true);
    apply(batch);
    EXPECT_FALSE(state->isHandActionLegal(ruled::v1::HAND_ACTION_CAST_SPELL, 1));
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

// CR 608.2b: a target that changes zones becomes a new object, so the arrow endpoint recorded when
// the target was chosen is never revised. Without the write-once latch, an oid that enters the
// graveyard map after its permanent dies re-resolves to the graveyard pile and the arrow points
// there instead of disappearing.
TEST_F(RuledClientTest, TargetKindIsLatchedOnceAndNotRevisedWhenTheTargetDies)
{
    ruled::v1::RuledEventBatch push;
    auto *ability = push.add_events()->mutable_stack_pushed();
    ability->set_object_id(11);
    ability->mutable_targets()->Add()->set_object_id(100);
    apply(push);

    EXPECT_EQ(state->latchedTargetKind(11, 100), RuledTargetItemKind::Unknown);
    state->latchTargetKind(11, 100, RuledTargetItemKind::Battlefield);
    EXPECT_EQ(state->latchedTargetKind(11, 100), RuledTargetItemKind::Battlefield);

    // The permanent dies and its oid now also appears in the graveyard map. A re-classification
    // must not overwrite the original answer.
    state->latchTargetKind(11, 100, RuledTargetItemKind::Graveyard);
    EXPECT_EQ(state->latchedTargetKind(11, 100), RuledTargetItemKind::Battlefield);
}

// An unresolvable target (its CardItem does not exist yet) leaves the entry unlatched so a later
// sync can classify it, rather than freezing Unknown and never drawing the arrow.
TEST_F(RuledClientTest, UnknownTargetKindIsNotLatched)
{
    state->latchTargetKind(11, 100, RuledTargetItemKind::Unknown);
    EXPECT_TRUE(state->stackTargetKindByStackAndTargetOid.isEmpty());

    state->latchTargetKind(11, 100, RuledTargetItemKind::Graveyard);
    EXPECT_EQ(state->latchedTargetKind(11, 100), RuledTargetItemKind::Graveyard);
}

// The latch is keyed per (stack object, target), so two spells aimed at the same object are
// independent — and resolving one must not strip the other's endpoint.
TEST_F(RuledClientTest, TargetKindLatchIsClearedWhenItsStackObjectResolves)
{
    ruled::v1::RuledEventBatch push;
    auto *first = push.add_events()->mutable_stack_pushed();
    first->set_object_id(11);
    first->mutable_targets()->Add()->set_object_id(100);
    auto *second = push.add_events()->mutable_stack_pushed();
    second->set_object_id(12);
    second->mutable_targets()->Add()->set_object_id(100);
    apply(push);

    state->latchTargetKind(11, 100, RuledTargetItemKind::Battlefield);
    state->latchTargetKind(12, 100, RuledTargetItemKind::Battlefield);

    ruled::v1::RuledEventBatch resolve;
    resolve.add_events()->mutable_stack_resolved()->set_object_id(11);
    apply(resolve);

    EXPECT_EQ(state->latchedTargetKind(11, 100), RuledTargetItemKind::Unknown);
    EXPECT_EQ(state->latchedTargetKind(12, 100), RuledTargetItemKind::Battlefield);
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
    // No card_id => an ability, which has no physical CardItem. is_triggered distinguishes a
    // *triggered* ability (this one) from an activated one; only the former retires the prompt.
    sp->set_is_triggered(true);
    apply(push);

    EXPECT_FALSE(state->hasPendingTriggerTarget());
    EXPECT_EQ(state->stackSourceOidByStackOid.value(900), 100u);
    ASSERT_EQ(host.createdSyntheticCards.size(), 1);
    EXPECT_EQ(host.createdSyntheticCards[0].oid, 900u);
    EXPECT_EQ(host.createdSyntheticCards[0].name, QStringLiteral("Gravedigger ETB"));
    EXPECT_EQ(host.createdSyntheticCards[0].controllerPlayerId, kLocalPlayer);
    EXPECT_EQ(state->stackAnnotation(900), QStringLiteral("Return target creature card..."));
}

// Paying a sacrifice cost (Bottle Gnomes) queues a dies trigger, and the activated ability that
// consumed the cost reaches the stack in the same batch. Both an activated and a triggered ability
// are card_id-less, so without is_triggered the activated one wiped the prompt for the trigger it
// had just caused — the player was left with a choice the engine was still blocking on.
TEST_F(RuledClientTest, ActivatedAbilityOnStackKeepsThePendingTriggerPrompt)
{
    ruled::v1::RuledEventBatch batch;
    auto *sp = batch.add_events()->mutable_stack_pushed();
    sp->set_object_id(900);
    sp->set_description("Bottle Gnomes");
    sp->set_ability_annotation("Sacrifice this creature: You gain 3 life.");
    sp->set_is_triggered(false); // an *activated* ability
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Whenever this creature or another creature dies, target player loses 1 life...");
    tnt->set_controller_player_id(kLocalPlayer);
    apply(batch);

    EXPECT_TRUE(state->hasPendingTriggerTarget())
        << "an activated ability must not retire the trigger prompt it caused";
    EXPECT_EQ(state->pendingTriggerSource(), 100u);
}

// Gravedigger ETB: a pending trigger whose only legal targets sit in a graveyard makes the tab
// auto-open the local graveyard view, so the player can click the target without hunting for it.
//
// NB the engine currently only fills LegalActions.valid_targets_by_ability from a permanent's
// *activated* abilities (engine/legal_actions.rs), so a triggered ability never gets an entry and
// this never fires in a real game. This pins the client half of the contract; the engine half is
// the open half.
TEST_F(RuledClientTest, PendingTriggerWithGraveyardTargetsAsksForTheGraveyardView)
{
    QSignalSpy spy(state, &RuledClientState::graveyardTargetsNeeded);

    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Return target creature card from your graveyard to your hand.");
    tnt->set_controller_player_id(kLocalPlayer);
    // The signal names players, not a bool, so the oid has to be mapped to a graveyard first.
    auto *ge = batch.add_events()->mutable_graveyard_object_map()->add_entries();
    ge->set_player_id(kLocalPlayer);
    ge->set_engine_object_id(500);
    ge->set_server_card_id(11);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto &abilityTargets = (*actions.mutable_valid_targets_by_ability())[(quint64(100) << 32) | 0u];
    abilityTargets.add_valid_graveyard_ids(500);
    apply(batch);

    ASSERT_TRUE(state->hasPendingTriggerTarget());
    ASSERT_GE(spy.count(), 1);
    EXPECT_EQ(spy.last().at(0).value<QList<int>>(), QList<int>{kLocalPlayer});

    // Once the ability itself is on the stack the target has been chosen, so the view may close.
    // A *triggered* ability push (card_id-less, annotated, is_triggered) retires the prompt.
    const int beforePush = spy.count();
    ruled::v1::RuledEventBatch push;
    auto *sp = push.add_events()->mutable_stack_pushed();
    sp->set_object_id(900);
    sp->set_ability_annotation("Return target creature card...");
    sp->set_is_triggered(true);
    apply(push);
    EXPECT_FALSE(state->hasPendingTriggerTarget());
    ASSERT_GT(spy.count(), beforePush);
    EXPECT_TRUE(spy.last().at(0).value<QList<int>>().isEmpty());
}

// Reanimate reads *a* graveyard, so a pending cast must be able to ask for the opponent's view.
TEST_F(RuledClientTest, PendingCastGraveyardTargetsNameTheOwningPlayer)
{
    QSignalSpy spy(state, &RuledClientState::graveyardTargetsNeeded);

    ruled::v1::RuledEventBatch batch;
    auto *gy = batch.add_events()->mutable_graveyard_object_map();
    auto *mine = gy->add_entries();
    mine->set_player_id(kLocalPlayer);
    mine->set_engine_object_id(500);
    mine->set_server_card_id(11);
    auto *theirs = gy->add_entries();
    theirs->set_player_id(kOpponent);
    theirs->set_engine_object_id(501);
    theirs->set_server_card_id(11); // same card id, different owner — must not collide
    apply(batch);

    // A cast whose only legal target sits in the opponent's graveyard.
    state->setPendingCastGraveyardTargets({501u});
    ASSERT_GE(spy.count(), 1);
    EXPECT_EQ(spy.last().at(0).value<QList<int>>(), QList<int>{kOpponent});

    // Targeting both graveyards asks for both views.
    state->setPendingCastGraveyardTargets({500u, 501u});
    QList<int> both = spy.last().at(0).value<QList<int>>();
    std::sort(both.begin(), both.end());
    EXPECT_EQ(both, (QList<int>{kLocalPlayer, kOpponent}));

    // Cancelling the cast retracts the request.
    state->setPendingCastGraveyardTargets({});
    EXPECT_TRUE(spy.last().at(0).value<QList<int>>().isEmpty());
}

// A cast spell keeps its target's graveyard open until it leaves the stack, so the targeting
// arrow stays anchored to the card instead of to a pile the player can no longer see.
TEST_F(RuledClientTest, GraveyardStaysRequestedWhileTheSpellIsOnTheStack)
{
    QSignalSpy spy(state, &RuledClientState::graveyardTargetsNeeded);

    // A creature in the opponent's graveyard, and a spell on the stack targeting it.
    ruled::v1::RuledEventBatch batch;
    auto *ge = batch.add_events()->mutable_graveyard_object_map()->add_entries();
    ge->set_player_id(kOpponent);
    ge->set_engine_object_id(700);
    ge->set_server_card_id(21);
    auto *sp = batch.add_events()->mutable_stack_pushed();
    sp->set_object_id(800);
    sp->add_targets()->set_object_id(700);
    apply(batch);

    // No pending cast any more — the spell has been cast — but the graveyard is still wanted.
    ASSERT_GE(spy.count(), 1);
    EXPECT_EQ(spy.last().at(0).value<QList<int>>(), QList<int>{kOpponent})
        << "the spell is on the stack, so its target's graveyard must stay open";

    // It resolves: nothing wants the graveyard any more.
    ruled::v1::RuledEventBatch resolved;
    auto *sr = resolved.add_events()->mutable_stack_resolved();
    sr->set_object_id(800);
    apply(resolved);
    EXPECT_TRUE(spy.last().at(0).value<QList<int>>().isEmpty()) << "once the spell leaves the stack the view may close";
}

// A target chosen in a graveyard is latched as such the moment it is put on the stack, without
// waiting for the (deferred) arrow sync — `emitGraveyardTargetsNeeded` runs first and needs the
// answer already.
TEST_F(RuledClientTest, GraveyardTargetIsLatchedWhenTheSpellIsPushed)
{
    ruled::v1::RuledEventBatch batch;
    auto *ge = batch.add_events()->mutable_graveyard_object_map()->add_entries();
    ge->set_player_id(kOpponent);
    ge->set_engine_object_id(700);
    ge->set_server_card_id(21);
    auto *sp = batch.add_events()->mutable_stack_pushed();
    sp->set_object_id(800);
    sp->add_targets()->set_object_id(700);
    apply(batch);

    EXPECT_EQ(state->latchedTargetKind(800, 700), RuledTargetItemKind::Graveyard);
}

// The regression this pairs with: an ability targeting a *permanent* must never open a graveyard
// view, even when that permanent dies while the ability is still on the stack and its oid joins the
// graveyard map. Only targets chosen in a graveyard count.
TEST_F(RuledClientTest, ADyingTargetDoesNotRequestAGraveyardView)
{
    // The ability goes on the stack while its target is a permanent — nothing in any graveyard yet.
    ruled::v1::RuledEventBatch push;
    auto *sp = push.add_events()->mutable_stack_pushed();
    sp->set_object_id(800);
    sp->add_targets()->set_object_id(100);
    apply(push);
    EXPECT_EQ(state->latchedTargetKind(800, 100), RuledTargetItemKind::Unknown);

    QSignalSpy spy(state, &RuledClientState::graveyardTargetsNeeded);

    // The target is destroyed in response: its oid now appears in a graveyard map.
    ruled::v1::RuledEventBatch died;
    auto *ge = died.add_events()->mutable_graveyard_object_map()->add_entries();
    ge->set_player_id(kOpponent);
    ge->set_engine_object_id(100);
    ge->set_server_card_id(21);
    apply(died);

    ASSERT_GE(spy.count(), 1);
    EXPECT_TRUE(spy.last().at(0).value<QList<int>>().isEmpty())
        << "an ability that targeted a permanent must not pop graveyard views when it dies";
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
    auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    (*batch.mutable_legal_by_player())[kLocalPlayer].add_selectable_attacker_ids(100);
    apply(batch);
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
    actions.add_selectable_attacker_ids(100);
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
    auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kOpponent);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_selectable_blocker_ids(200);
    actions.add_selectable_blocker_ids(201);
    apply(batch);
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
    auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kOpponent);
    (*batch.mutable_legal_by_player())[kLocalPlayer].add_selectable_blocker_ids(200);
    apply(batch);
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

TEST_F(RuledClientTest, AppliesAndClearsObjectIdKeyedExileCastActions)
{
    constexpr quint32 objectId = 0x01020304u;
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *cast = actions.add_zone_cast_actions();
    cast->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    cast->set_object_id(objectId);
    cast->set_face_index(0);
    cast->set_card_name("Bonecrusher Giant");
    cast->set_cost("{2}{R}");
    cast->set_needs_target(true);
    auto &targets = (*actions.mutable_valid_targets_by_zone_object())[(quint64(objectId) << 8) | 0u];
    targets.add_valid_permanent_ids(99);
    apply(batch);

    ASSERT_TRUE(state->isZoneActionLegal(objectId));
    ASSERT_EQ(state->zoneActionSource(objectId), RuledCastSource::Exile);
    ASSERT_EQ(state->zoneActionCost(objectId, 0), QStringLiteral("{2}{R}"));
    ASSERT_EQ(state->zoneActionFaceOptions(objectId).size(), 1);
    EXPECT_EQ(state->zoneActionFaceOptions(objectId).first().faceName, QStringLiteral("Bonecrusher Giant"));
    EXPECT_TRUE(state->isValidSpellTarget(static_cast<int>(objectId), 0, 99, RuledCastSource::Exile));

    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN1, kLocalPlayer));
    EXPECT_FALSE(state->isZoneActionLegal(objectId));
    EXPECT_FALSE(state->isValidSpellTarget(static_cast<int>(objectId), 0, 99, RuledCastSource::Exile));
}

TEST_F(RuledClientTest, CombatStagingIgnoresCreaturesOutsideEngineSelectableSets)
{
    auto attackers = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    (*attackers.mutable_legal_by_player())[kLocalPlayer].add_selectable_attacker_ids(100);
    apply(attackers);
    host.sentCommands.clear();

    state->togglePendingAttacker(200);
    EXPECT_FALSE(state->isPendingAttacker(200));
    EXPECT_TRUE(host.sentCommands.isEmpty());
    state->togglePendingAttacker(100);
    EXPECT_TRUE(state->isPendingAttacker(100));

    ruled::v1::RuledEventBatch declared;
    declared.add_events()->mutable_attackers_declared()->add_attacker_object_ids(300);
    apply(declared);
    auto blockers = phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kOpponent);
    (*blockers.mutable_legal_by_player())[kLocalPlayer].add_selectable_blocker_ids(400);
    apply(blockers);

    state->toggleStagedBlocker(500);
    EXPECT_FALSE(state->isStagedBlocker(500));
    state->toggleStagedBlocker(400);
    EXPECT_TRUE(state->isStagedBlocker(400));
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
    for (const auto &[oid, power, toughness] :
         std::initializer_list<std::tuple<quint32, quint32, quint32>>{{100, 5, 5}, {200, 2, 2}, {201, 3, 3}}) {
        auto *object = view->add_battlefield_objects();
        object->set_object_id(oid);
        object->set_power(power);
        object->set_toughness(toughness);
    }
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
    addPermanent(ev, kLocalPlayer, 100, 1)->add_keywords("Trample"); // CR 702.19
    addPermanent(ev, kOpponent, 200, 2);
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    auto *attackerView = view->add_battlefield_objects();
    attackerView->set_object_id(100);
    attackerView->set_power(6);
    attackerView->set_toughness(6);
    auto *blockerView = view->add_battlefield_objects();
    blockerView->set_object_id(200);
    blockerView->set_power(1);
    blockerView->set_toughness(2);
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
    auto *object = view->add_battlefield_objects();
    object->set_object_id(100);
    object->set_damage(3);
    auto *manaAbility = object->add_activated_abilities();
    manaAbility->set_text("Add {G}.");
    manaAbility->set_mana_produced("G");
    manaAbility->set_cost_label("{T}");
    auto *drawAbility = object->add_activated_abilities();
    drawAbility->set_text("Sacrifice this: draw a card.");
    drawAbility->set_mana_cost("1");
    drawAbility->set_cost_label("Sacrifice this");
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

TEST_F(RuledClientTest, BattlefieldOmissionRetainsStateWhileOtherZoneViewFieldsUpdate)
{
    ruled::v1::RuledEventBatch full;
    auto *fullView = full.add_events()->mutable_zone_view()->add_per_player();
    fullView->set_player_id(kLocalPlayer);
    auto *object = fullView->add_battlefield_objects();
    object->set_object_id(100);
    object->set_power(4);
    object->set_toughness(5);
    object->set_damage(2);
    auto *ability = object->add_activated_abilities();
    ability->set_text("Draw a card.");
    ability->set_cost_label("{T}");
    ability->set_activatable(true);
    apply(full);

    ruled::v1::RuledEventBatch omitted;
    auto *omittedZone = omitted.add_events()->mutable_zone_view();
    omittedZone->set_battlefields_unchanged(true);
    auto *omittedView = omittedZone->add_per_player();
    omittedView->set_player_id(kLocalPlayer);
    omittedView->set_first_strike_step_pending(true);
    apply(omitted);

    EXPECT_EQ(state->markedDamageForEngineOid(100), 2);
    EXPECT_EQ(state->combatPowerForCreatureOid(100), 4);
    EXPECT_EQ(state->combatToughnessForCreatureOid(100), 5);
    EXPECT_EQ(state->activatedAbilitiesForOid(100), QStringList({QStringLiteral("Draw a card.")}));
    EXPECT_EQ(state->activatedAbilityCostLabelsForOid(100), QStringList({QStringLiteral("{T}")}));
    EXPECT_TRUE(state->isFirstStrikeStepPending()) << "non-battlefield fields in an omitted view still apply";

    ruled::v1::RuledEventBatch explicitEmpty;
    explicitEmpty.add_events()->mutable_zone_view()->add_per_player()->set_player_id(kLocalPlayer);
    apply(explicitEmpty);
    EXPECT_EQ(state->markedDamageForEngineOid(100), 0);
    EXPECT_TRUE(state->activatedAbilitiesForOid(100).isEmpty());
}

// ---------------------------------------------------------------------------------------
// Simultaneous trigger ordering (CR 603.3b)
// ---------------------------------------------------------------------------------------

namespace
{
/// A two-candidate ordering prompt addressed to `decidingPlayer`.
ruled::v1::RuledEventBatch triggerOrderBatch(int decidingPlayer)
{
    ruled::v1::RuledEventBatch batch;
    auto *tor = batch.add_events()->mutable_trigger_order_required();
    tor->set_deciding_player_id(decidingPlayer);
    auto *first = tor->add_candidates();
    first->set_trigger_object_id(501);
    first->set_source_permanent_id(41);
    first->set_ability_index(0);
    first->set_source_card_name("Blood Artist");
    first->set_ability_text("Target player loses 1 life and you gain 1 life.");
    auto *second = tor->add_candidates();
    second->set_trigger_object_id(502);
    second->set_source_permanent_id(42);
    second->set_ability_index(0);
    second->set_source_card_name("Blood Artist");
    second->set_ability_text("Target player loses 1 life and you gain 1 life.");
    return batch;
}
} // namespace

TEST_F(RuledClientTest, TriggerOrderRequiredOpensTheOrderingChoiceForTheDecider)
{
    apply(triggerOrderBatch(kLocalPlayer));

    ASSERT_TRUE(state->hasPendingTriggerOrder());
    const auto candidates = state->triggerOrderCandidates();
    ASSERT_EQ(candidates.size(), 2);
    EXPECT_EQ(candidates[0].oid, 501u);
    EXPECT_EQ(candidates[0].sourceOid, 41u);
    EXPECT_EQ(candidates[0].cardName, QStringLiteral("Blood Artist"));
    // The ability text is what the popup annotates each card image with.
    EXPECT_FALSE(candidates[0].abilityText.isEmpty());
}

TEST_F(RuledClientTest, TriggerOrderRequiredIsNotPromptedForTheOpponent)
{
    apply(triggerOrderBatch(kLocalPlayer + 1));

    EXPECT_FALSE(state->hasPendingTriggerOrder());
    EXPECT_TRUE(state->triggerOrderCandidates().isEmpty());
}

TEST_F(RuledClientTest, TriggerOrderPopupCardsMapToTheirTriggers)
{
    apply(triggerOrderBatch(kLocalPlayer));

    // The popup identifies cards by index; those are the only ids that count as candidates.
    EXPECT_TRUE(state->isTriggerOrderPickCard(0));
    EXPECT_TRUE(state->isTriggerOrderPickCard(1));
    EXPECT_FALSE(state->isTriggerOrderPickCard(2));
    EXPECT_FALSE(state->isTriggerOrderPickCard(501));
}

TEST_F(RuledClientTest, ClickingAnOrderingCardPlacesThatTriggerImmediately)
{
    apply(triggerOrderBatch(kLocalPlayer));

    host.sentCommands.clear();
    state->pickTriggerOrderCard(1); // the second candidate

    // One click is one placement: no confirm step, and the oid is the clicked card's trigger.
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_trigger_order().trigger_object_id(), 502u);
    // Cleared straight away — the engine replies with either a target prompt or a shorter
    // ordering prompt, and a lingering popup would invite a click it is about to refuse.
    EXPECT_FALSE(state->hasPendingTriggerOrder());
}

TEST_F(RuledClientTest, ClickingANonCandidateCardSendsNothing)
{
    apply(triggerOrderBatch(kLocalPlayer));

    host.sentCommands.clear();
    state->pickTriggerOrderCard(7);

    EXPECT_TRUE(host.sentCommands.empty());
    EXPECT_TRUE(state->hasPendingTriggerOrder());
}

TEST_F(RuledClientTest, PlacingOneTriggerAndReofferingTheRestStaysActiveInOneBatch)
{
    apply(triggerOrderBatch(kLocalPlayer));
    ASSERT_TRUE(state->hasPendingTriggerOrder());

    // The engine's reply to a pick is one batch that both puts the chosen trigger on the stack and
    // re-offers the rest. It must net out to "still ordering" — if the StackPushed clear won, the
    // popup would be torn down and rebuilt, losing its position mid-choice.
    ruled::v1::RuledEventBatch reply;
    auto *sp = reply.add_events()->mutable_stack_pushed();
    sp->set_object_id(501);
    sp->set_description("Blood Artist");
    sp->set_is_triggered(true);
    auto *tor = reply.add_events()->mutable_trigger_order_required();
    tor->set_deciding_player_id(kLocalPlayer);
    auto *remaining = tor->add_candidates();
    remaining->set_trigger_object_id(502);
    remaining->set_source_card_name("Blood Artist");
    remaining->set_ability_text("Target player loses 1 life and you gain 1 life.");
    apply(reply);

    ASSERT_TRUE(state->hasPendingTriggerOrder());
    ASSERT_EQ(state->triggerOrderCandidates().size(), 1);
    EXPECT_EQ(state->triggerOrderCandidates()[0].oid, 502u);
}

TEST_F(RuledClientTest, ARepeatedPromptReplacesTheCandidatesWithWhatIsLeft)
{
    // The engine re-sends the prompt after each pick with one fewer candidate; the popup is
    // rebuilt from whatever the latest prompt carries.
    apply(triggerOrderBatch(kLocalPlayer));
    ASSERT_EQ(state->triggerOrderCandidates().size(), 2);

    ruled::v1::RuledEventBatch second;
    auto *tor = second.add_events()->mutable_trigger_order_required();
    tor->set_deciding_player_id(kLocalPlayer);
    auto *only = tor->add_candidates();
    only->set_trigger_object_id(502);
    only->set_source_card_name("Blood Artist");
    only->set_ability_text("Target player loses 1 life and you gain 1 life.");
    apply(second);

    ASSERT_EQ(state->triggerOrderCandidates().size(), 1);
    EXPECT_EQ(state->triggerOrderCandidates()[0].oid, 502u);
    EXPECT_TRUE(state->isTriggerOrderPickCard(0));
    EXPECT_FALSE(state->isTriggerOrderPickCard(1));
}

TEST_F(RuledClientTest, StackPushedForACandidateClearsTheTriggerOrderState)
{
    apply(triggerOrderBatch(kLocalPlayer));
    ASSERT_TRUE(state->hasPendingTriggerOrder());

    // The engine reserved 501 for this trigger, so seeing it on the stack proves the prompt was
    // answered — covers reconnects and resynced batches, where no local submit happened.
    ruled::v1::RuledEventBatch pushed;
    auto *sp = pushed.add_events()->mutable_stack_pushed();
    sp->set_object_id(501);
    sp->set_description("Blood Artist");
    sp->set_is_triggered(true);
    apply(pushed);

    EXPECT_FALSE(state->hasPendingTriggerOrder());
}

TEST_F(RuledClientTest, TriggerOrderReplacesAnyStalePendingChoice)
{
    // One pending choice at a time: an ordering prompt must tear down whatever was parked before.
    ruled::v1::RuledEventBatch tnt;
    auto *needs = tnt.add_events()->mutable_trigger_needs_target();
    needs->set_controller_player_id(kLocalPlayer);
    needs->set_source_permanent_id(41);
    needs->set_ability_text("Target player loses 1 life.");
    apply(tnt);
    ASSERT_TRUE(state->hasPendingTriggerTarget());

    apply(triggerOrderBatch(kLocalPlayer));

    EXPECT_TRUE(state->hasPendingTriggerOrder());
    EXPECT_FALSE(state->hasPendingTriggerTarget());
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
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Search your library"));
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

// CR 701.18 scry reuses the library-search deck popup, retitled. The ordering step submits in
// click order, which is how the engine reads `ordered: true` (same convention as Brainstorm).
TEST_F(RuledClientTest, LibraryTopChoiceOpensTheScryViewAndSubmitsInClickOrder)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_TOP); // Preordain, ordering step
    rcr->set_min(2);
    rcr->set_max(2);
    rcr->set_ordered(true);
    for (const quint32 oid : {21u, 22u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    rcr->add_candidate_names("Island");
    rcr->add_candidate_names("Island");
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Scry"));
    ASSERT_EQ(started.count(), 1);
    EXPECT_EQ(started.at(0).at(0).toStringList(), QStringList({QStringLiteral("Island"), QStringLiteral("Island")}));
    // Duplicate names must both stay pickable — scry never sets unique_names.
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(0));
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(1));

    state->toggleResolutionHandPickCard(1);
    state->toggleResolutionHandPickCard(0);
    EXPECT_EQ(state->resolutionHandPickSelected(), 2);

    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &sub = host.sentCommands[0].submit_resolution_choice();
    ASSERT_EQ(sub.chosen_object_ids_size(), 2);
    // Click order reaches the engine verbatim; it reads the list bottom-first, so the last card
    // clicked here (21) is the one that ends up on top.
    EXPECT_EQ(sub.chosen_object_ids(0), 22u);
    EXPECT_EQ(sub.chosen_object_ids(1), 21u);
    EXPECT_FALSE(state->isResolutionHandPickActive());
}

// A scry step that keeps every card on top submits nothing: min 0 must be answerable.
TEST_F(RuledClientTest, LibraryTopChoiceAllowsSubmittingAnEmptyBottomPile)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_TOP); // Opt, scry 1
    rcr->set_min(0);
    rcr->set_max(1);
    rcr->add_candidate_object_ids(31u);
    rcr->add_candidate_server_card_ids(0);
    rcr->add_candidate_names("Island");
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickRequired(), 0);

    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids_size(), 0);
    EXPECT_FALSE(state->isResolutionHandPickActive());
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
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Revealed cards"));

    state->toggleResolutionHandPickCard(1);
    state->toggleResolutionHandPickCard(2);
    state->submitResolutionHandPick();
    ASSERT_EQ(revealed.count(), 2);
    EXPECT_FALSE(revealed.at(1).at(0).toBool());
}

/// Thoughtseize/Coercion (CR 701.7): the popup renders like a revealed set but is a hand, and
/// must say so — it is built on the deck zone as a scaffold, which used to name the window.
TEST_F(RuledClientTest, OpponentHandChoiceRendersAsARevealedPickTitledAsAHand)
{
    QSignalSpy revealed(state, &RuledClientState::revealedPickChanged);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    rcr->set_min(1);
    rcr->set_max(1);
    for (const quint32 oid : {21u, 22u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    rcr->add_candidate_names("Black Lotus");
    rcr->add_candidate_names("Swamp");
    apply(batch);

    ASSERT_EQ(revealed.count(), 1);
    EXPECT_TRUE(revealed.at(0).at(0).toBool());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Revealed);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Target player's hand"));
    EXPECT_EQ(revealed.at(0).at(1).toStringList(),
              QStringList({QStringLiteral("Black Lotus"), QStringLiteral("Swamp")}));

    state->toggleResolutionHandPickCard(0);
    EXPECT_EQ(state->resolutionHandPickSelected(), 1);
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
        ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
        EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, 100));
        EXPECT_FALSE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, 101));
        EXPECT_FALSE(state->isResolutionHandPickActive()); // no list dialog for this kind
        EXPECT_EQ(host.dialogRequests, 0);

        host.sentCommands.clear();
        state->submitPendingChoiceObject(100);
        ASSERT_EQ(host.sentCommands.size(), 1);
        EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 100u);
        EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
    }
    {
        ruled::v1::RuledEventBatch batch;
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LEGEND_KEEP); // CR 704.5j legend rule
        rcr->add_candidate_object_ids(100);
        rcr->add_candidate_object_ids(101);
        apply(batch);
        ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::LegendKeep));
        EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::LegendKeep, 101));

        host.sentCommands.clear();
        state->submitPendingChoiceObject(101);
        ASSERT_EQ(host.sentCommands.size(), 1);
        EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 101u);
        EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::LegendKeep));
    }
}

// The engine parks one choice at a time, so the holder is exclusive: installing a new choice
// tears the previous one down — including the revealed-cards popup a pick had opened.
TEST_F(RuledClientTest, InstallingAChoiceTearsDownTheOneItReplaces)
{
    ruled::v1::RuledEventBatch reveal;
    auto *rcr = reveal.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_REVEALED);
    rcr->set_min(1);
    rcr->set_max(1);
    rcr->add_candidate_object_ids(10);
    rcr->add_candidate_server_card_ids(0);
    rcr->add_candidate_names("Swamp");
    apply(reveal);
    ASSERT_TRUE(state->isResolutionHandPickActive());

    QSignalSpy revealed(state, &RuledClientState::revealedPickChanged);
    ruled::v1::RuledEventBatch trigger;
    auto *tnt = trigger.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_text("Draw a card.");
    tnt->set_controller_player_id(kLocalPlayer);
    apply(trigger);

    EXPECT_TRUE(state->hasPendingTriggerTarget());
    EXPECT_FALSE(state->isResolutionHandPickActive());
    ASSERT_EQ(revealed.count(), 1);
    EXPECT_FALSE(revealed.at(0).at(0).toBool()); // popup told to close
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

TEST_F(RuledClientTest, ReplacementEffectChoiceUsesModalFallbackAndSubmitsOpaqueApplicationId)
{
    host.autoSubmitDialogChoice = true;
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT);
    rcr->set_prompt_text("Choose the next replacement effect for Diregraf Ghoul entering the battlefield.");
    rcr->set_min(1);
    rcr->set_max(1);
    rcr->add_candidate_object_ids(7001);
    rcr->add_candidate_names("Orb of Dreams - permanents enter tapped");
    apply(batch);

    EXPECT_EQ(host.dialogRequests, 1);
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_TRUE(host.sentCommands[0].has_submit_resolution_choice());
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 7001u);
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
        addHandAction(actions, ruled::v1::HAND_ACTION_OPENING_BOTTOM, i, "Card");
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
        addHandAction(actions, ruled::v1::HAND_ACTION_OPENING_BOTTOM, i, "Card");
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
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].put_opening_hand_on_bottom().hand_card_index(), 1u);
    host.answerPendingAck(true);
    ASSERT_EQ(host.sentCommands.size(), 2);
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
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 0, "Grizzly Bears");
    apply(batch);
    ASSERT_TRUE(state->hasStackItems());
    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));

    QSignalSpy resetSpy(state, &RuledClientState::sessionReset);
    state->clearSessionState();

    EXPECT_EQ(resetSpy.count(), 1);
    EXPECT_FALSE(state->hasStackItems());
    EXPECT_TRUE(state->stackAnnotation(900).isEmpty());
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
    EXPECT_FALSE(state->hasPendingTriggerTarget());
    EXPECT_FALSE(state->isHandActionLegal(ruled::v1::HAND_ACTION_CAST_SPELL, 0));
    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::None);
    EXPECT_EQ(state->getOpeningMulliganCount(), 0);
    // Phantom graveyard targets are the reason this map must not survive a game boundary.
    EXPECT_EQ(state->graveyardEngineOidForOwnedCard(kLocalPlayer, 11), 0u);
}

// The server broadcasts a new session's first RuledEventBatch *before* the Event_GameStateChanged
// that flips game_started, so the game-start teardown runs with the incoming opening prompt already
// applied. Clearing it there strands the opening: the engine is blocked on ChooseStartingPlayer and
// never re-sends the prompt.
TEST_F(RuledClientTest, GameStartResetKeepsTheIncomingSessionsOpeningPrompt)
{
    ruled::v1::RuledEventBatch batch;
    // Residue from the finished game that must still be cleared…
    batch.add_events()->mutable_stack_pushed()->set_object_id(900);
    // …alongside the incoming session's opening prompt, which must survive.
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_labels("You start (opening pick)");
    actions.add_labels("Opponent starts (opening pick)");
    apply(batch);
    ASSERT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::ChooseFirst);

    state->clearSessionState(RuledSessionResetScope::KeepCurrentBatch);

    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::ChooseFirst);
    EXPECT_FALSE(state->hasStackItems()); // the finished game's residue still goes

    // The game-stop transition is the symmetric case: nothing survives it.
    state->clearSessionState(RuledSessionResetScope::All);
    EXPECT_EQ(state->getOpeningUiKind(), RuledClientState::RuledOpeningUiKind::None);
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

// ---------------------------------------------------------------------------------------------
// Dev console grammar (RuledDevCommandParser)
//
// The console is the one ruled input that starts as free text, so the grammar is the only place a
// typo turns into a wrong command rather than a compile error. Seats here are ids {7, 9} rather
// than {0, 1} so an ordinal can never accidentally equal the id it resolves to.
// ---------------------------------------------------------------------------------------------

namespace
{
const QVector<int> kSeats = {7, 9};
constexpr int kMe = 7;

RuledDevCommandParser::Result parseLine(const QString &line)
{
    return RuledDevCommandParser::parse(line, kMe, kSeats);
}
} // namespace

TEST(RuledDevCommandParserTest, PutDefaultsToTheLocalSeat)
{
    const auto r = parseLine(QStringLiteral("put hand Serra Angel"));
    ASSERT_TRUE(r.ok) << r.error.toStdString();
    const auto &dev = r.command.dev_command();
    EXPECT_EQ(dev.target_player_id(), kMe);
    EXPECT_EQ(dev.put_card_in_zone().card_name(), "Serra Angel");
    EXPECT_EQ(dev.put_card_in_zone().zone(), ruled::v1::DEV_ZONE_HAND);
    EXPECT_FALSE(dev.put_card_in_zone().ready());
}

TEST(RuledDevCommandParserTest, SeatIsAOneBasedOrdinalNotAPlayerId)
{
    const auto r = parseLine(QStringLiteral("put 2 gy Lightning Bolt"));
    ASSERT_TRUE(r.ok) << r.error.toStdString();
    EXPECT_EQ(r.command.dev_command().target_player_id(), 9) << "ordinal 2 is the second seat";
    EXPECT_EQ(r.command.dev_command().put_card_in_zone().zone(), ruled::v1::DEV_ZONE_GRAVEYARD);
}

TEST(RuledDevCommandParserTest, OutOfRangeSeatIsRejected)
{
    const auto r = parseLine(QStringLiteral("put 5 hand Serra Angel"));
    EXPECT_FALSE(r.ok);
    EXPECT_FALSE(r.error.isEmpty());
}

TEST(RuledDevCommandParserTest, EveryZoneAliasResolves)
{
    const QVector<QPair<QString, ruled::v1::DevZone>> cases = {
        {QStringLiteral("hand"), ruled::v1::DEV_ZONE_HAND},
        {QStringLiteral("bf"), ruled::v1::DEV_ZONE_BATTLEFIELD},
        {QStringLiteral("battlefield"), ruled::v1::DEV_ZONE_BATTLEFIELD},
        {QStringLiteral("board"), ruled::v1::DEV_ZONE_BATTLEFIELD},
        {QStringLiteral("gy"), ruled::v1::DEV_ZONE_GRAVEYARD},
        {QStringLiteral("graveyard"), ruled::v1::DEV_ZONE_GRAVEYARD},
        {QStringLiteral("exile"), ruled::v1::DEV_ZONE_EXILE},
        {QStringLiteral("lib"), ruled::v1::DEV_ZONE_LIBRARY},
        {QStringLiteral("library"), ruled::v1::DEV_ZONE_LIBRARY},
        {QStringLiteral("deck"), ruled::v1::DEV_ZONE_LIBRARY},
    };
    for (const auto &[word, zone] : cases) {
        const auto r = parseLine(QStringLiteral("put %1 Grizzly Bears").arg(word));
        ASSERT_TRUE(r.ok) << word.toStdString() << ": " << r.error.toStdString();
        EXPECT_EQ(r.command.dev_command().put_card_in_zone().zone(), zone) << word.toStdString();
    }
}

TEST(RuledDevCommandParserTest, UnknownZoneIsRejected)
{
    EXPECT_FALSE(parseLine(QStringLiteral("put sideboard Serra Angel")).ok);
}

TEST(RuledDevCommandParserTest, ReadyIsStrippedOnlyAsATrailingToken)
{
    const auto readied = parseLine(QStringLiteral("put bf Grizzly Bears ready"));
    ASSERT_TRUE(readied.ok);
    EXPECT_TRUE(readied.command.dev_command().put_card_in_zone().ready());
    EXPECT_EQ(readied.command.dev_command().put_card_in_zone().card_name(), "Grizzly Bears");

    // A card named "ready" is still a card name: the flag is only consumed when something
    // precedes it, so the grammar does not depend on what is in the card pool.
    const auto named = parseLine(QStringLiteral("put bf ready"));
    ASSERT_TRUE(named.ok);
    EXPECT_FALSE(named.command.dev_command().put_card_in_zone().ready());
    EXPECT_EQ(named.command.dev_command().put_card_in_zone().card_name(), "ready");
}

TEST(RuledDevCommandParserTest, PutWithoutACardNameIsRejected)
{
    EXPECT_FALSE(parseLine(QStringLiteral("put bf")).ok);
    EXPECT_FALSE(parseLine(QStringLiteral("put")).ok);
}

TEST(RuledDevCommandParserTest, MoveSharesPutsGrammarButBuildsTheOtherPayload)
{
    const auto r = parseLine(QStringLiteral("move 2 gy Serra Angel"));
    ASSERT_TRUE(r.ok) << r.error.toStdString();
    const auto &dev = r.command.dev_command();
    EXPECT_EQ(dev.target_player_id(), 9);
    ASSERT_TRUE(dev.has_move_card()) << "move must not build a put payload";
    EXPECT_FALSE(dev.has_put_card_in_zone());
    EXPECT_EQ(dev.move_card().card_name(), "Serra Angel");
    EXPECT_EQ(dev.move_card().zone(), ruled::v1::DEV_ZONE_GRAVEYARD);

    // `ready` applies to move too: relocating onto the battlefield re-sickens the permanent.
    const auto readied = parseLine(QStringLiteral("move bf Grizzly Bears ready"));
    ASSERT_TRUE(readied.ok);
    EXPECT_TRUE(readied.command.dev_command().move_card().ready());

    EXPECT_FALSE(parseLine(QStringLiteral("move gy")).ok);
    EXPECT_FALSE(parseLine(QStringLiteral("move")).ok);
}

TEST(RuledDevCommandParserTest, PutAndMoveAreDistinctVerbs)
{
    // The whole point of the split: the same line with a different verb produces a different
    // payload, so "give me a card" and "relocate this one" can no longer be confused.
    const auto put = parseLine(QStringLiteral("put bf Serra Angel"));
    const auto moved = parseLine(QStringLiteral("move bf Serra Angel"));
    ASSERT_TRUE(put.ok);
    ASSERT_TRUE(moved.ok);
    EXPECT_TRUE(put.command.dev_command().has_put_card_in_zone());
    EXPECT_TRUE(moved.command.dev_command().has_move_card());
}

TEST(RuledDevCommandParserTest, ManaCountsColourPipsAndGenericDigits)
{
    const auto r = parseLine(QStringLiteral("mana 3RR"));
    ASSERT_TRUE(r.ok) << r.error.toStdString();
    const auto &m = r.command.dev_command().add_mana();
    EXPECT_EQ(m.c(), 3u);
    EXPECT_EQ(m.r(), 2u);
    EXPECT_EQ(m.w(), 0u);

    const auto multi = parseLine(QStringLiteral("mana WWU"));
    ASSERT_TRUE(multi.ok);
    EXPECT_EQ(multi.command.dev_command().add_mana().w(), 2u);
    EXPECT_EQ(multi.command.dev_command().add_mana().u(), 1u);

    // Digits form one number, so this is twelve generic rather than one and two. It also has to
    // survive the seat rule: a lone leading number is symbols, not an out-of-range seat.
    const auto twelve = parseLine(QStringLiteral("mana 12"));
    ASSERT_TRUE(twelve.ok) << twelve.error.toStdString();
    EXPECT_EQ(twelve.command.dev_command().add_mana().c(), 12u);
    EXPECT_EQ(twelve.command.dev_command().target_player_id(), kMe);
}

TEST(RuledDevCommandParserTest, ManaSeatOnlyWinsWhenItIsAValidOrdinalWithSymbolsAfterIt)
{
    // "2 UU" is the second seat; "3 RR" cannot be (there is no seat 3), so it falls back to
    // reading as mana rather than erroring — three generic and two red.
    const auto notASeat = parseLine(QStringLiteral("mana 3 RR"));
    ASSERT_TRUE(notASeat.ok) << notASeat.error.toStdString();
    EXPECT_EQ(notASeat.command.dev_command().target_player_id(), kMe);
    EXPECT_EQ(notASeat.command.dev_command().add_mana().c(), 3u);
    EXPECT_EQ(notASeat.command.dev_command().add_mana().r(), 2u);

    // And the unambiguous spelling of the same thing.
    const auto joined = parseLine(QStringLiteral("mana 3RR"));
    ASSERT_TRUE(joined.ok);
    EXPECT_EQ(joined.command.dev_command().target_player_id(), kMe);
}

TEST(RuledDevCommandParserTest, ManaAcceptsASeatAndRejectsJunkSymbols)
{
    const auto seated = parseLine(QStringLiteral("mana 2 UU"));
    ASSERT_TRUE(seated.ok) << seated.error.toStdString();
    EXPECT_EQ(seated.command.dev_command().target_player_id(), 9);
    EXPECT_EQ(seated.command.dev_command().add_mana().u(), 2u);

    EXPECT_FALSE(parseLine(QStringLiteral("mana XYZ")).ok);
    EXPECT_FALSE(parseLine(QStringLiteral("mana")).ok);
}

TEST(RuledDevCommandParserTest, LeadingSlashAndCaseAreTolerated)
{
    const auto slashed = parseLine(QStringLiteral("/PUT Hand Serra Angel"));
    ASSERT_TRUE(slashed.ok) << slashed.error.toStdString();
    EXPECT_EQ(slashed.command.dev_command().put_card_in_zone().zone(), ruled::v1::DEV_ZONE_HAND);
    // The card name keeps its typed casing — the engine matches Oracle names case-insensitively,
    // but the log should read the way the user wrote it.
    EXPECT_EQ(slashed.command.dev_command().put_card_in_zone().card_name(), "Serra Angel");
}

TEST(RuledDevCommandParserTest, HelpIsHandledLocallyAndUnknownVerbsAreNot)
{
    const auto help = parseLine(QStringLiteral("help"));
    EXPECT_FALSE(help.ok);
    EXPECT_TRUE(help.handledLocally);
    EXPECT_FALSE(help.message.isEmpty());

    const auto unknown = parseLine(QStringLiteral("summon Serra Angel"));
    EXPECT_FALSE(unknown.ok);
    EXPECT_FALSE(unknown.handledLocally);
    EXPECT_FALSE(unknown.error.isEmpty());
}

} // namespace
