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

#include "game/ruled/ruled_auto_pass_policy.h"
#include "game/ruled/ruled_client_host.h"
#include "game/ruled/ruled_client_state.h"
#include "game/ruled/ruled_dev_command_parser.h"
#include "game/ruled/ruled_event_dispatcher.h"
#include "game/ruled/ruled_mana_pool_tracker.h"
#include "game/ruled/ruled_pending_cast.h"
#include "game/ruled/ruled_presentation_resolver.h"
#include "game/ruled/ruled_restricted_mana_model.h"
#include "game/ruled/ruled_zone_snapshot_policy.h"

#include <QSignalSpy>
#include <QDir>
#include <QFile>
#include <QTemporaryDir>
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
        std::optional<TokenStackIdentity> sourceTokenIdentity;
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
                                  const QString &setName,
                                  const std::optional<TokenStackIdentity> &sourceTokenIdentity) override
    {
        createdSyntheticCards.append({virtualOid, displayName, controllerPlayerId, setName, sourceTokenIdentity});
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
        action->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        action->set_hand_index(static_cast<quint32>(handIndex));
        action->set_card_name(cardName);
        action->set_face_index(static_cast<quint32>(faceIndex));
        action->set_needs_target(needsTarget);
        action->set_cost(cost);
        return action;
    }

    static void addLegalBlockPair(ruled::v1::LegalActions &actions, quint32 blockerOid, quint32 attackerOid)
    {
        auto *pair = actions.add_legal_block_pairs();
        pair->set_blocker_id(blockerOid);
        pair->set_attacker_id(attackerOid);
    }

    static ruled::v1::AttackAssignment playerAttackAssignment(quint32 attackerOid, int playerId = kOpponent)
    {
        ruled::v1::AttackAssignment assignment;
        assignment.set_attacker_object_id(attackerOid);
        assignment.set_defending_player_id(playerId);
        assignment.mutable_defender()->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
        assignment.mutable_defender()->set_object_id(static_cast<quint32>(playerId));
        return assignment;
    }

    static void addLegalPlayerAttack(ruled::v1::LegalActions &actions, quint32 attackerOid)
    {
        *actions.add_legal_attack_assignments() = playerAttackAssignment(attackerOid);
    }

    static ruled::v1::AttackAssignment
    permanentAttackAssignment(quint32 attackerOid, quint32 defenderOid, quint64 defenderGeneration)
    {
        ruled::v1::AttackAssignment assignment;
        assignment.set_attacker_object_id(attackerOid);
        assignment.set_attacker_zone_change_generation(7);
        assignment.set_defending_player_id(kOpponent);
        assignment.mutable_defender()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
        assignment.mutable_defender()->set_object_id(defenderOid);
        assignment.set_defender_zone_change_generation(defenderGeneration);
        return assignment;
    }
};

// ---------------------------------------------------------------------------------------
// Identity maps
// ---------------------------------------------------------------------------------------

TEST_F(RuledClientTest, ConvokePreviewPreservesLegalActionsAndRejectsObsoleteReplies)
{
    state->handActions[ruled::v1::HAND_ACTION_CAST_SPELL].handIndices.insert(4);
    auto &payment = state->payment;
    payment.begin();
    const auto old = payment.request({});
    ASSERT_TRUE(payment.payMana('U'));
    const auto current = payment.request({});
    ruled::v1::RuledEventBatch batch;
    auto *reply = batch.mutable_payment_preview();
    reply->set_transaction_id(old.transaction_id());
    reply->set_revision(old.revision());
    reply->set_valid(true);
    reply->set_complete(true);
    QSignalSpy spy(state, &RuledClientState::paymentPreviewReceived);
    EXPECT_TRUE(dispatcher->processPayload(batch.SerializeAsString()));
    EXPECT_TRUE(payment.pending);
    EXPECT_EQ(spy.count(), 0);
    reply->set_revision(current.revision());
    *reply->mutable_selection() = payment.selection;
    EXPECT_TRUE(dispatcher->processPayload(batch.SerializeAsString()));
    EXPECT_FALSE(payment.pending);
    EXPECT_EQ(spy.count(), 1);
    EXPECT_TRUE(state->isHandActionLegal(ruled::v1::HAND_ACTION_CAST_SPELL, 4));
    EXPECT_TRUE(dispatcher->processPayload(batch.SerializeAsString()));
    EXPECT_EQ(spy.count(), 1);
    payment.clear();
    EXPECT_TRUE(dispatcher->processPayload(batch.SerializeAsString()));
    EXPECT_FALSE(payment.active);
    EXPECT_EQ(spy.count(), 1);
}

TEST(RuledZoneSnapshotPolicyTest, OpenPermissionMirrorDoesNotDuplicatePublicExileOnFullSnapshots)
{
    EXPECT_TRUE(ruledSnapshotPreservesEventAuthoritativeZone(QString::fromLatin1(ZoneNames::STACK)));
    EXPECT_TRUE(ruledSnapshotPreservesEventAuthoritativeZone(QString::fromLatin1(ZoneNames::GRAVE)));
    EXPECT_TRUE(ruledSnapshotPreservesEventAuthoritativeZone(QString::fromLatin1(ZoneNames::EXILE)));
    EXPECT_FALSE(ruledSnapshotPreservesEventAuthoritativeZone(QString::fromLatin1(ZoneNames::HAND)));
}

TEST_F(RuledClientTest, ConvokeSelectionUsesPublishedColorAndGeneration)
{
    auto &payment = state->payment;
    payment.begin();
    auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    auto *candidate = response.add_candidates();
    candidate->mutable_object()->set_object_id(44);
    candidate->mutable_object()->set_zone_change_generation(7);
    candidate->add_options(ruled::v1::OBJECT_PAYMENT_KIND_BLUE);
    candidate->add_options(ruled::v1::OBJECT_PAYMENT_KIND_GENERIC);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_FALSE(payment.select(44, ruled::v1::OBJECT_PAYMENT_KIND_RED));
    EXPECT_TRUE(payment.select(44, ruled::v1::OBJECT_PAYMENT_KIND_BLUE));
    EXPECT_FALSE(payment.select(44, ruled::v1::OBJECT_PAYMENT_KIND_GENERIC));
    EXPECT_EQ(payment.selection.convoke(0).object().zone_change_generation(), 7u);
    request = payment.request({});
    response.set_revision(request.revision());
    *response.mutable_selection() = payment.selection;
    ASSERT_TRUE(payment.apply(response));
    EXPECT_TRUE(payment.remove(44));
    EXPECT_FALSE(payment.selected(44));
}

TEST_F(RuledClientTest, WaterbendUsesSharedPaymentSelectionWithoutConvokeColorChoices)
{
    auto &payment = state->payment;
    payment.begin();
    const auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    auto *candidate = response.add_candidates();
    candidate->mutable_object()->set_object_id(44);
    candidate->mutable_object()->set_zone_change_generation(7);
    candidate->add_options(ruled::v1::OBJECT_PAYMENT_KIND_WATERBEND);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_FALSE(payment.select(44, ruled::v1::OBJECT_PAYMENT_KIND_BLUE));
    ASSERT_TRUE(payment.select(44, ruled::v1::OBJECT_PAYMENT_KIND_WATERBEND));
    ASSERT_EQ(payment.selection.waterbend_size(), 1);
    EXPECT_EQ(payment.selection.waterbend(0).zone_change_generation(), 7u);
    EXPECT_EQ(payment.selection.convoke_size(), 0);
}

TEST_F(RuledClientTest, SharedPaymentTracksOptimisticPoolDebitsAndRetiresSanitizedMana)
{
    auto &payment = state->payment;
    payment.begin();
    auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    ASSERT_TRUE(payment.apply(response));

    ASSERT_TRUE(payment.payMana('G', 0, 17));
    EXPECT_EQ(payment.optimisticManaCounterSpendCount(17), 1);
    RuledManaPoolTracker tracker;
    EXPECT_EQ(tracker.observe(17, 0, 3, 0).displayedBeforeNewStaging, 3);
    EXPECT_EQ(tracker.observe(17, 2, 3, payment.optimisticManaCounterSpendCount(17)).displayedBeforeNewStaging, 2);
    request = payment.request({});
    response.set_revision(request.revision());
    *response.mutable_selection() = request.cast_spell().payment();
    ASSERT_TRUE(payment.apply(response));
    EXPECT_TRUE(payment.takeRetiredOptimisticManaCounterIds().isEmpty());

    // The engine can sanitize a stale or excessive staged pip. The UI must restore precisely
    // that optimistic display debit instead of leaving the pool counter visually reduced.
    request = payment.request({});
    response.set_revision(request.revision());
    response.mutable_selection()->Clear();
    response.set_selection_changed(true);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_EQ(payment.optimisticManaCounterSpendCount(17), 0);
    EXPECT_EQ(payment.takeRetiredOptimisticManaCounterIds(), QVector<int>({17}));

    ASSERT_TRUE(payment.payMana('G', 0, 17));
    payment.clear();
    EXPECT_EQ(payment.takeRetiredOptimisticManaCounterIds(), QVector<int>({17}));
}

TEST_F(RuledClientTest, SubmittedSharedPaymentStopsDebitingAuthoritativeSnapshots)
{
    auto &payment = state->payment;
    payment.begin();
    auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    ASSERT_TRUE(payment.apply(response));
    ASSERT_TRUE(payment.payMana('R', 0, 17));

    request = payment.request({});
    response.set_revision(request.revision());
    response.set_complete(true);
    *response.mutable_selection() = request.cast_spell().payment();
    ASSERT_TRUE(payment.apply(response));
    ASSERT_TRUE(payment.beginSubmission());

    EXPECT_EQ(payment.optimisticManaCounterSpendCount(17), 0)
        << "the accepted server snapshot already contains the submitted debit";
    EXPECT_EQ(payment.takeAllOptimisticManaCounterIds(), QVector<int>({17}))
        << "the identity remains available until acceptance or rejection is known";
}

TEST_F(RuledClientTest, SharedPaymentCarriesExactPermanentActionMana)
{
    auto &payment = state->payment;
    payment.begin();
    ruled::v1::RuledCommand action;
    auto *permanent = action.mutable_execute_permanent_action();
    permanent->set_kind(ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR);
    permanent->set_object_id(81);
    permanent->set_expected_zone_change_generation(9);

    auto request = payment.requestAction(action);
    ASSERT_TRUE(request.has_execute_permanent_action());
    EXPECT_EQ(request.execute_permanent_action().object_id(), 81u);

    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    response.mutable_selection()->set_expected_state_revision(22);
    response.mutable_selection()->mutable_source()->set_object_id(81);
    response.mutable_selection()->mutable_source()->set_zone_change_generation(9);
    ASSERT_TRUE(payment.apply(response));
    ASSERT_TRUE(payment.payMana('R'));

    request = payment.requestAction(action);
    ASSERT_TRUE(request.has_execute_permanent_action());
    EXPECT_EQ(request.execute_permanent_action().payment().mana().r(), 1u);
    payment.writePayment(action);
    EXPECT_EQ(action.execute_permanent_action().payment().source().object_id(), 81u);
    EXPECT_EQ(action.execute_permanent_action().payment().mana().r(), 1u);
}

TEST_F(RuledClientTest, ConvokeCompletionSubmitsExactlyOnceAndStateRefreshInvalidatesReplies)
{
    auto &payment = state->payment;
    payment.begin();
    auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    response.set_complete(true);
    payment.invalidate(); // authoritative gameplay batch, before the queued refresh query
    EXPECT_FALSE(payment.apply(response));
    EXPECT_FALSE(payment.beginSubmission());
    request = payment.request({});
    response.set_revision(request.revision());
    response.set_selection_changed(true);
    response.set_error("Payment changed; stale or unavailable selections were removed.");
    ASSERT_TRUE(payment.apply(response));
    EXPECT_FALSE(payment.beginSubmission());
    request = payment.request({});
    response.set_revision(request.revision());
    response.set_selection_changed(false);
    response.clear_error();
    ASSERT_TRUE(payment.apply(response));
    EXPECT_EQ(payment.view.error(), "Payment changed; stale or unavailable selections were removed.");
    EXPECT_TRUE(payment.beginSubmission());
    EXPECT_FALSE(payment.beginSubmission());
    EXPECT_FALSE(payment.apply(response));
    payment.clear();
    EXPECT_FALSE(payment.beginSubmission());
}

TEST_F(RuledClientTest, WaterbendDisarmsSanitizedPaymentsAndSerializesEachContext)
{
    auto &payment = state->payment;
    payment.begin(true);
    ruled::v1::RuledCommand ability;
    ability.mutable_activate_ability()->set_source_object_id(81);
    auto request = payment.requestAction(ability);
    EXPECT_TRUE(request.has_activate_ability());
    EXPECT_FALSE(request.has_cast_spell());
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    response.set_complete(false);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_FALSE(payment.beginSubmission());
    ASSERT_TRUE(payment.payMana('U'));
    request = payment.requestAction(ability);
    response.set_revision(request.revision());
    response.set_selection_changed(true);
    response.set_complete(true);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_FALSE(payment.beginSubmission());
    request = payment.requestAction(ability);
    response.set_revision(request.revision());
    response.set_selection_changed(false);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_FALSE(payment.beginSubmission()); // Sanitizing a stale selection disarms submission.
    ASSERT_TRUE(payment.payMana('U'));
    request = payment.requestAction(ability);
    response.set_revision(request.revision());
    *response.mutable_selection() = request.activate_ability().payment();
    ASSERT_TRUE(payment.apply(response));
    ASSERT_TRUE(payment.beginSubmission());
    EXPECT_FALSE(payment.beginSubmission());
    payment.writePayment(ability);
    EXPECT_EQ(ability.activate_ability().payment().mana().u(), 1u);
    ruled::v1::RuledCommand resolution;
    resolution.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
    payment.writePayment(resolution);
    EXPECT_EQ(resolution.submit_resolution_choice().payment().SerializeAsString(),
              ability.activate_ability().payment().SerializeAsString());
}

TEST_F(RuledClientTest, ZeroCostWaterbendCompletesWithoutAnImpossibleContribution)
{
    auto &payment = state->payment;
    payment.begin(true); // Opening the activation/payment branch is itself intentional.
    const auto request = payment.request({});
    ruled::v1::PaymentPreview response;
    response.set_transaction_id(request.transaction_id());
    response.set_revision(request.revision());
    response.set_valid(true);
    response.set_complete(true);
    ASSERT_TRUE(payment.apply(response));
    EXPECT_TRUE(payment.beginSubmission());
    EXPECT_FALSE(payment.beginSubmission());
}

TEST_F(RuledClientTest, WaterbendResolutionRefreshAndRejectedSubmissionPreserveStaging)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_source_object_id(81);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    choice->set_waterbend(true);
    choice->set_generic_mana_cost(2);
    apply(batch);
    ASSERT_TRUE(state->isWaterbendResolutionPayment());
    auto &payment = state->payment;
    payment.begin(true);
    payment.selection.add_waterbend()->set_object_id(44);
    payment.selection.mutable_waterbend(0)->set_zone_change_generation(7);
    const auto transaction = payment.transaction();
    apply(batch); // Mana abilities republish the same parked choice.
    ASSERT_TRUE(payment.active);
    EXPECT_EQ(payment.transaction(), transaction);
    ASSERT_EQ(payment.selection.waterbend_size(), 1);
    payment.view.set_valid(true);
    payment.view.set_complete(true);
    state->payResolutionMana();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().payment().waterbend(0).zone_change_generation(), 7u);
    host.answerPendingAck(false);
    ASSERT_TRUE(state->isWaterbendResolutionPayment());
    ASSERT_EQ(payment.selection.waterbend_size(), 1);
    EXPECT_FALSE(payment.submitting);
    EXPECT_FALSE(state->resolutionPaymentCurrentlyLegal());
    payment.pending = false;
    state->payResolutionMana();
    apply(batch); // A same-source refresh can arrive before the rejection acknowledgement.
    host.answerPendingAck(false);
    EXPECT_FALSE(payment.submitting);
    EXPECT_EQ(payment.selection.waterbend_size(), 1);
    state->declineResolutionMana();
    EXPECT_FALSE(host.sentCommands.last().submit_resolution_choice().has_payment());
    host.answerPendingAck(true);
    choice->set_source_object_id(82);
    apply(batch);
    EXPECT_FALSE(payment.active); // A different resolution owns a new transaction.
}

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

TEST_F(RuledClientTest, EarthbendAnimationAndReturnReplaceDerivedCharacteristics)
{
    auto publish = [&](bool creature, quint64 generation) {
        ruled::v1::RuledEventBatch batch;
        auto *entry = addPermanent(batch.add_events(), kLocalPlayer, 100, 7);
        entry->set_is_creature(creature);
        if (creature) {
            entry->add_keywords("Haste");
        }
        auto *player = batch.add_events()->mutable_zone_view()->add_per_player();
        player->set_player_id(kLocalPlayer);
        auto *object = player->add_battlefield_objects();
        object->set_object_id(100);
        object->set_zone_change_generation(generation);
        object->set_power(creature ? 2 : 0);
        object->set_toughness(creature ? 2 : 0);
        apply(batch);
    };
    publish(false, 0);
    EXPECT_FALSE(state->isEngineOidCreature(100));
    publish(true, 0);
    EXPECT_TRUE(state->isEngineOidCreature(100));
    EXPECT_TRUE(state->isEngineOidHaste(100));
    EXPECT_EQ(state->combatPowerForCreatureOid(100), 2);
    EXPECT_EQ(state->combatToughnessForCreatureOid(100), 2);
    publish(false, 2);
    EXPECT_FALSE(state->isEngineOidCreature(100));
    EXPECT_FALSE(state->isEngineOidHaste(100));
    EXPECT_EQ(state->battlefieldGenerationByOid.value(100), 2u);
    EXPECT_EQ(state->cardIdForEngineOid(100), 7);
    EXPECT_EQ(state->combatPowerForCreatureOid(100), 0);
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
    ability.waitingForCost = true;
    EXPECT_EQ(readyRuledPendingPaymentAction({}, ability), RuledPendingPaymentAction::None);
}

TEST(RuledPendingPaymentTest, LegacyOverlapResumesTheSpellBeforeTheAbility)
{
    PendingRuledSpellCast spell;
    spell.valid = true;

    PendingActivatedAbility ability;
    ability.valid = true;

    // This overlap should disappear when the pending controller becomes exclusive. Characterize
    // the current recovery order first so an in-flight client cannot submit two commands while the
    // state is being extracted: the older code always resumes the spell and leaves the ability
    // pending for the next settled batch.
    EXPECT_EQ(readyRuledPendingPaymentAction(spell, ability), RuledPendingPaymentAction::CastSpell);
}

TEST(RuledPendingPaymentTest, ControllerKeepsSpellAndAbilityInteractionsExclusive)
{
    RuledPendingCast pending;
    pending.beginSpell().cardName = QStringLiteral("Fireball");
    EXPECT_EQ(pending.activeInteraction(), RuledPendingCast::InteractionKind::Spell);
    EXPECT_TRUE(pending.spell.valid);
    EXPECT_FALSE(pending.ability.valid);

    pending.beginAbility().cardName = QStringLiteral("Bottle Gnomes");
    EXPECT_EQ(pending.activeInteraction(), RuledPendingCast::InteractionKind::Ability);
    EXPECT_FALSE(pending.spell.valid);
    EXPECT_TRUE(pending.ability.valid);

    pending.clearAbility();
    EXPECT_EQ(pending.activeInteraction(), RuledPendingCast::InteractionKind::None);
}

TEST(RuledPendingTargetTest, ClickEligibilityUsesLatestAuthoritativeModalTargets)
{
    FakeHost host;
    RuledClientState state(&host);
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForTarget = true;
    spell.handIndex = 3;
    spell.faceIndex = 0;
    spell.activeModePosition = 0;
    spell.activeTargetGroupPosition = 0;

    RuledSpellTargetData stale;
    stale.validPermanentIds.insert(100);
    spell.selectedModes.append({7, QStringLiteral("mode"), true, stale, {}, {}});

    RuledSpellTargetData current;
    current.validPermanentIds.insert(200);
    current.groups.append(static_cast<const RuledTargetGroupData &>(current));
    RuledModalSpellOption mode{7, QStringLiteral("mode"), true, true, current};
    state.handActions[ruled::v1::HAND_ACTION_CAST_SPELL]
        .modalOptionsByCastKey[RuledClientState::handCastActionKey(3, 0, ruled::v1::CAST_METHOD_NORMAL)] = {mode};

    EXPECT_EQ(ruledTargetClickEligibility(spell, {}, state, RuledTargetCandidateKind::Battlefield, 100, 0),
              RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledTargetClickEligibility(spell, {}, state, RuledTargetCandidateKind::Battlefield, 200, 0),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility(spell, {}, state, RuledTargetCandidateKind::Player, 0, 0),
              RuledTargetClickEligibility::Illegal);
}

TEST(RuledPendingTargetTest, SpellPromptUsesFaceAndActiveModeContext)
{
    FakeHost host;
    RuledClientState state(&host);
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForTarget = true;
    spell.handIndex = 3;
    spell.faceIndex = 0;
    spell.cardName = QStringLiteral("Mystic Confluence");
    spell.activeModePosition = 0;
    spell.activeTargetGroupPosition = 0;

    RuledSpellTargetData targets;
    RuledTargetGroupData group;
    group.promptText = QStringLiteral("Choose target spell.");
    group.minTargets = 1;
    group.maxTargets = 1;
    targets.groups.append(group);
    spell.selectedModes.append({7, QStringLiteral("Counter target spell unless its controller pays {3}"), true,
                                targets, {}, {}});
    state.handActions[ruled::v1::HAND_ACTION_CAST_SPELL]
        .modalOptionsByCastKey[RuledClientState::handCastActionKey(3, 0, ruled::v1::CAST_METHOD_NORMAL)] = {
        {7, QStringLiteral("Counter target spell unless its controller pays {3}"), true, true, targets}};

    EXPECT_EQ(ruledPendingSpellTargetPrompt(spell, state),
              QString::fromUtf8("Choose a target for “Counter target spell unless its controller pays {3}”.\n"
                                "Choose target spell."));
}

TEST(RuledPresentationResolverTest, ResolvesExactFaceLinesAndFallsBackAsOneUnit)
{
    QTemporaryDir directory;
    ASSERT_TRUE(directory.isValid());
    const QString cardDatabasePath = directory.filePath(QStringLiteral("cards.xml"));
    const QString cachePath = RuledOracleCache::cachePathForCardDatabase(cardDatabasePath);
    const QString oracleText = QStringLiteral("Choose one —\n• First mode\n• Second mode");
    QString writeError;
    ASSERT_TRUE(RuledOracleCache::writeAtomic(
        cachePath, QStringLiteral("https://example.test/AllPrintings.json"), QStringLiteral("2026-08-31"),
        {{QStringLiteral("Fire // Ice"), QStringLiteral("Fire"), oracleText}}, &writeError))
        << writeError.toStdString();

    RuledPresentationResolver resolver;
    QString loadError;
    ASSERT_TRUE(resolver.loadForCardDatabase(cardDatabasePath, &loadError)) << loadError.toStdString();

    ruled::v1::PresentationRef presentation;
    presentation.set_card_id("fire_ice");
    presentation.set_face_id("fire");
    presentation.set_external_card_name("Fire // Ice");
    presentation.set_external_face_name("Fire");
    presentation.set_oracle_text_sha256(RuledOracleCache::textSha256(oracleText).toStdString());
    presentation.add_oracle_line_indices(1);
    presentation.add_oracle_line_indices(3);
    presentation.set_fallback_text("fallback");
    EXPECT_EQ(resolver.resolve(presentation), QString::fromUtf8("Choose one —\n• Second mode"));

    presentation.set_oracle_text_sha256(std::string(64, '0'));
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("fallback"));
    presentation.set_oracle_text_sha256(RuledOracleCache::textSha256(oracleText).toStdString());
    presentation.add_oracle_line_indices(99);
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("fallback"));
}

TEST(RuledPresentationResolverTest, EmptyMappingAlwaysUsesEngineFallback)
{
    ruled::v1::PresentationRef presentation;
    presentation.set_fallback_text("engine-authored fallback");
    RuledPresentationResolver resolver;
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("engine-authored fallback"));
}

TEST(RuledPresentationResolverTest, NormalizesLineEndingsWhitespaceHeadingsAndBullets)
{
    EXPECT_EQ(RuledOracleCache::normalizedText(
                  QString::fromUtf8("  Choose two —\r\n\r\n • First mode  \r  • Second mode\n\t\nTail  ")),
              QString::fromUtf8("Choose two —\n• First mode\n• Second mode\nTail"));
}

TEST(RuledPresentationResolverTest, SelectsExactSplitRoomAndTransformFaces)
{
    QTemporaryDir directory;
    ASSERT_TRUE(directory.isValid());
    const QString cardDatabasePath = directory.filePath(QStringLiteral("cards.xml"));
    const QString cachePath = RuledOracleCache::cachePathForCardDatabase(cardDatabasePath);
    const QList<RuledOracleFace> faces{
        {QStringLiteral("Fire // Ice"), QStringLiteral("Fire"), QStringLiteral("Fire deals 2 damage.")},
        {QStringLiteral("Fire // Ice"), QStringLiteral("Ice"), QStringLiteral("Tap target permanent.\nDraw a card.")},
        {QStringLiteral("Derelict Attic // Widow's Walk"), QStringLiteral("Derelict Attic"),
         QStringLiteral("When you unlock this door, draw two cards, then lose 2 life.")},
        {QStringLiteral("Brutal Cathar // Moonrage Brute"), QStringLiteral("Moonrage Brute"),
         QStringLiteral("First strike\nWard—Pay 3 life.")},
    };
    QString error;
    ASSERT_TRUE(RuledOracleCache::writeAtomic(cachePath, QStringLiteral("https://example.test/source"),
                                              QStringLiteral("snapshot-1"), faces, &error))
        << error.toStdString();
    RuledPresentationResolver resolver;
    ASSERT_TRUE(resolver.loadForCardDatabase(cardDatabasePath, &error)) << error.toStdString();

    const auto resolveFace = [&](const QString &card, const QString &face, std::initializer_list<quint32> lines) {
        const auto found = std::find_if(faces.begin(), faces.end(), [&](const RuledOracleFace &candidate) {
            return candidate.cardName == card && candidate.faceName == face;
        });
        EXPECT_NE(found, faces.end());
        ruled::v1::PresentationRef presentation;
        presentation.set_external_card_name(card.toStdString());
        presentation.set_external_face_name(face.toStdString());
        presentation.set_oracle_text_sha256(RuledOracleCache::textSha256(found->oracleText).toStdString());
        presentation.set_fallback_text("fallback");
        for (const quint32 line : lines) {
            presentation.add_oracle_line_indices(line);
        }
        return resolver.resolve(presentation);
    };

    EXPECT_EQ(resolveFace(QStringLiteral("Fire // Ice"), QStringLiteral("Ice"), {1, 2}),
              QStringLiteral("Tap target permanent.\nDraw a card."));
    EXPECT_EQ(resolveFace(QStringLiteral("Derelict Attic // Widow's Walk"), QStringLiteral("Derelict Attic"), {1}),
              QStringLiteral("When you unlock this door, draw two cards, then lose 2 life."));
    EXPECT_EQ(resolveFace(QStringLiteral("Brutal Cathar // Moonrage Brute"), QStringLiteral("Moonrage Brute"), {2}),
              QString::fromUtf8("Ward—Pay 3 life."));
}

TEST(RuledPresentationResolverTest, RejectsMissingCorruptIncompatibleAndIncompleteCachesAsOneUnit)
{
    QTemporaryDir directory;
    ASSERT_TRUE(directory.isValid());
    const QString cardDatabasePath = directory.filePath(QStringLiteral("cards.xml"));
    const QString cachePath = RuledOracleCache::cachePathForCardDatabase(cardDatabasePath);
    const QString text = QStringLiteral("One\nTwo");
    QString error;
    ASSERT_TRUE(RuledOracleCache::writeAtomic(
        cachePath, QStringLiteral("https://example.test/source"), QStringLiteral("snapshot-1"),
        {{QStringLiteral("Card"), QStringLiteral("Face"), text}}, &error));
    RuledPresentationResolver resolver;
    ASSERT_TRUE(resolver.loadForCardDatabase(cardDatabasePath, &error));

    ruled::v1::PresentationRef presentation;
    presentation.set_external_card_name("Missing");
    presentation.set_external_face_name("Face");
    presentation.set_oracle_text_sha256(RuledOracleCache::textSha256(text).toStdString());
    presentation.set_fallback_text("whole fallback");
    presentation.add_oracle_line_indices(1);
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("whole fallback"));

    presentation.set_external_card_name("Card");
    presentation.set_external_face_name("Missing");
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("whole fallback"));
    presentation.set_external_face_name("Face");
    presentation.set_oracle_text_sha256(std::string(64, 'f'));
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("whole fallback"));
    presentation.set_oracle_text_sha256(RuledOracleCache::textSha256(text).toStdString());
    presentation.set_oracle_line_indices(0, 0);
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("whole fallback"));
    presentation.set_oracle_line_indices(0, 1);
    presentation.add_oracle_line_indices(3);
    EXPECT_EQ(resolver.resolve(presentation), QStringLiteral("whole fallback"));

    QFile corrupt(cachePath);
    ASSERT_TRUE(corrupt.open(QIODevice::WriteOnly | QIODevice::Truncate));
    ASSERT_EQ(corrupt.write("{not-json"), 9);
    corrupt.close();
    RuledPresentationResolver corruptResolver;
    EXPECT_FALSE(corruptResolver.loadForCardDatabase(cardDatabasePath, &error));
    EXPECT_EQ(corruptResolver.resolve(presentation), QStringLiteral("whole fallback"));
    RuledPresentationResolver missingResolver;
    EXPECT_FALSE(missingResolver.loadForCardDatabase(directory.filePath(QStringLiteral("missing.xml")), &error));
    EXPECT_EQ(missingResolver.resolve(presentation), QStringLiteral("whole fallback"));
}

TEST(RuledPresentationResolverTest, AtomicCacheRefreshRetainsProvenanceAndWorksOffline)
{
    QTemporaryDir directory;
    ASSERT_TRUE(directory.isValid());
    const QString cardDatabasePath = directory.filePath(QStringLiteral("cards.xml"));
    const QString cachePath = RuledOracleCache::cachePathForCardDatabase(cardDatabasePath);
    QString error;
    ASSERT_TRUE(RuledOracleCache::writeAtomic(
        cachePath, QStringLiteral("https://example.test/source"), QStringLiteral("snapshot-1"),
        {{QStringLiteral("Card"), QStringLiteral("Face"), QStringLiteral("Offline text")}}, &error));
    RuledOracleCache offline;
    ASSERT_TRUE(offline.load(cachePath, &error));
    EXPECT_EQ(offline.sourceUrl(), QStringLiteral("https://example.test/source"));
    EXPECT_EQ(offline.sourceVersion(), QStringLiteral("snapshot-1"));
    EXPECT_EQ(offline.compatibleFaceText(QStringLiteral("Card"), QStringLiteral("Face"),
                                         RuledOracleCache::textSha256(QStringLiteral("Offline text"))),
              QStringLiteral("Offline text"));

    ASSERT_TRUE(RuledOracleCache::writeAtomic(
        cachePath, QStringLiteral("https://example.test/source"), QStringLiteral("snapshot-2"),
        {{QStringLiteral("Card"), QStringLiteral("Face"), QStringLiteral("Longer refreshed offline text")}},
        &error));
    ASSERT_TRUE(offline.load(cachePath, &error));
    EXPECT_EQ(offline.sourceVersion(), QStringLiteral("snapshot-2"));
    EXPECT_EQ(QDir(directory.path()).entryList({QStringLiteral("cards.ruled-oracle.json*")}, QDir::Files).size(), 1);
}

TEST(RuledPendingTargetTest, SpellPromptComposesResolvedOrFallbackWordingWithEngineGuidance)
{
    FakeHost host;
    RuledClientState state(&host);
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForTarget = true;
    spell.handIndex = 4;
    spell.faceIndex = 0;
    spell.cardName = QStringLiteral("Deterministic spell fallback");
    spell.presentationText = QStringLiteral("Resolved exact Oracle wording");
    spell.activeTargetGroupPosition = 0;
    RuledSpellTargetData targets;
    RuledTargetGroupData group;
    group.promptText = QStringLiteral("Choose a target controlled by an opponent.");
    targets.groups.append(group);
    state.validTargetsByHandSlot.insert(RuledClientState::spellTargetKey(4, 0), targets);

    EXPECT_EQ(ruledPendingSpellTargetPrompt(spell, state),
              QString::fromUtf8("Choose a target for “Resolved exact Oracle wording”.\n"
                                "Choose a target controlled by an opponent."));
    spell.presentationText.clear();
    EXPECT_EQ(ruledPendingSpellTargetPrompt(spell, state),
              QString::fromUtf8("Choose a target for “Deterministic spell fallback”.\n"
                                "Choose a target controlled by an opponent."));
}

TEST(RuledPendingTargetTest, ActivatedAbilityPromptUsesAbilityTextAndTargetGuidance)
{
    FakeHost host;
    RuledClientState state(&host);
    PendingActivatedAbility ability;
    ability.valid = true;
    ability.permanentOid = 44;
    ability.abilityIndex = 2;
    ability.abilityText = QStringLiteral("{W}, {T}: Tap target creature.");

    RuledSpellTargetData targets;
    RuledTargetGroupData group;
    group.promptText = QStringLiteral("Choose target creature an opponent controls.");
    group.minTargets = 1;
    group.maxTargets = 1;
    targets.groups.append(group);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(44, 2), targets);

    EXPECT_EQ(ruledPendingAbilityTargetPrompt(ability, state),
              QString::fromUtf8("Choose a target for “{W}, {T}: Tap target creature.”\n"
                                "Choose target creature an opponent controls."));
}

TEST(RuledPendingTargetTest, ClickEligibilityCoversAbilitiesTriggersAndCopyRetargeting)
{
    FakeHost host;
    RuledClientState state(&host);
    PendingActivatedAbility ability;
    ability.valid = true;
    ability.waitingForTarget = true;
    ability.permanentOid = 44;
    ability.abilityIndex = 2;
    RuledSpellTargetData targets;
    targets.validGraveyardIds.insert(300);
    targets.canTargetOpponent = true;
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(44, 2), targets);

    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Graveyard, 300, 0),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Player, 1, 0),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, ability, state, RuledTargetCandidateKind::Battlefield, 301, 0),
              RuledTargetClickEligibility::Illegal);

    ability = {};
    RuledClientState::RuledPendingChoice trigger;
    trigger.kind = RuledClientState::ChoiceKind::TriggerTarget;
    state.setPendingChoice(trigger);
    state.lastTriggerSourceOid = 55;
    state.lastTriggerAbilityIndex = 4;
    targets = {};
    targets.validStackIds.insert(400);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(55, 4), targets);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, state, RuledTargetCandidateKind::Stack, 400, 0),
              RuledTargetClickEligibility::Legal);

    RuledClientState::RuledPendingChoice copy;
    copy.kind = RuledClientState::ChoiceKind::CopyTarget;
    copy.candidateOids = {500};
    state.setPendingChoice(copy);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, state, RuledTargetCandidateKind::Battlefield, 500, 0),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, state, RuledTargetCandidateKind::Graveyard, 500, 0),
              RuledTargetClickEligibility::Illegal);
}

TEST(RuledPendingTargetTest, CastCostObjectEligibilityUsesThePublishedOptionCandidates)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForCastCostObject = true;
    spell.nextCastCostGroup = 0;
    spell.activeCastCostOption = 3;

    RuledCastCostOption behold;
    behold.optionIndex = 3;
    behold.kind = RuledCastCostOptionKind::Behold;
    behold.selectable = true;
    behold.validHandIndices = {7};
    behold.validPermanentIds = {900};
    RuledCastCostGroup group;
    group.options = {behold};
    spell.castCostGroups = {group};

    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Hand, 7),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Hand, 8),
              RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Permanent, 900),
              RuledTargetClickEligibility::Legal);

    spell.waitingForCastCostObject = false;
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Hand, 7),
              RuledTargetClickEligibility::NotTargeting);
}

TEST(RuledPendingTargetTest, HarmonizeCastCostOnlyAcceptsPublishedPermanentCandidates)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForCastCostObject = true;
    spell.nextCastCostGroup = 0;
    spell.activeCastCostOption = 0;

    RuledCastCostOption harmonize;
    harmonize.optionIndex = 0;
    harmonize.kind = RuledCastCostOptionKind::TapPermanentForGenericReduction;
    harmonize.selectable = true;
    harmonize.validHandIndices = {7};
    harmonize.validPermanentIds = {900};
    RuledCastCostGroup group;
    group.options = {harmonize};
    spell.castCostGroups = {group};

    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Hand, 7),
              RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Permanent, 900),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledCastCostObjectEligibility(spell, RuledCastCostCandidateKind::Permanent, 901),
              RuledTargetClickEligibility::Illegal);
}

TEST(RuledPendingTargetTest, HarmonizeCostIsNotFinalizedWhileItsOptionChoiceIsPending)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.remainingCost[QLatin1Char('X')] = 5;
    spell.nextCastCostGroup = 0;

    RuledCastCostOption harmonize;
    harmonize.optionIndex = 0;
    harmonize.kind = RuledCastCostOptionKind::TapPermanentForGenericReduction;
    harmonize.selectable = true;
    RuledCastCostGroup group;
    group.options = {harmonize};
    spell.castCostGroups = {group};

    EXPECT_FALSE(ruledCastCostGroupsComplete(spell));

    spell.castCostGenericReduction = 2;
    spell.nextCastCostGroup = 1;
    EXPECT_TRUE(ruledCastCostGroupsComplete(spell));
    EXPECT_EQ(ruledFinalGenericCost(spell.remainingCost.value(QLatin1Char('X')), 0, spell.castCostGenericReduction), 3);
}

TEST(RuledPendingTargetTest, BoundedCastCostGroupStaysOpenUntilItsSelectionCountIsConfirmed)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.nextCastCostGroup = 0;

    RuledCastCostGroup group;
    group.groupIndex = 4;
    group.min = 1;
    group.max = 2;
    group.options = {{10, QStringLiteral("First"), RuledCastCostOptionKind::Mana},
                     {11, QStringLiteral("Second"), RuledCastCostOptionKind::Mana}};
    spell.castCostGroups = {group};

    EXPECT_EQ(ruledCastCostGroupSelectionCount(spell, 4), 0);
    EXPECT_FALSE(ruledCastCostGroupCanConfirm(spell, group));

    spell.castCostSelections.append(
        {4, 10, RuledPendingCastCostSelection::ObjectKind::None, 0, 0, 0});
    EXPECT_EQ(ruledCastCostGroupSelectionCount(spell, 4), 1);
    EXPECT_TRUE(ruledCastCostOptionAlreadySelected(spell, 4, 10));
    EXPECT_TRUE(ruledCastCostGroupCanConfirm(spell, group));
    EXPECT_FALSE(ruledCastCostGroupsComplete(spell));

    spell.castCostSelections.append(
        {4, 11, RuledPendingCastCostSelection::ObjectKind::None, 0, 0, 0});
    EXPECT_EQ(ruledCastCostGroupSelectionCount(spell, 4), 2);
    EXPECT_TRUE(ruledCastCostGroupCanConfirm(spell, group));

    spell.nextCastCostGroup = 1;
    EXPECT_TRUE(ruledCastCostGroupsComplete(spell));
}

TEST(RuledPendingTargetTest, PermanentChoicePromptKeepsEngineSpecificityAndAddsClickGuidance)
{
    EXPECT_EQ(formatRuledPermanentChoicePrompt(QStringLiteral("Choose a creature you control.")),
              QStringLiteral("Choose a creature you control.\nClick a highlighted permanent on the battlefield."));
}

TEST(RuledPendingTargetTest, ReconcileDropsTargetsMissingFromLatestLegalSnapshot)
{
    FakeHost host;
    RuledClientState state(&host);
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForTarget = true;
    spell.handIndex = 6;
    spell.activeTargetGroupPosition = 0;
    spell.selectedTargetOids = {10, 20};
    spell.selectedTargetDamages = {1, 2};
    spell.selectedTargetOidsByGroup = {{10, 20}};
    spell.selectedTargetDamagesByGroup = {{1, 2}};
    spell.targetDamageAllocations = {1, 2};
    RuledSpellTargetData current;
    current.validPermanentIds.insert(20);
    current.groups.append(static_cast<const RuledTargetGroupData &>(current));
    state.validTargetsByHandSlot.insert(RuledClientState::spellTargetKey(6, 0), current);

    PendingActivatedAbility ability;
    EXPECT_TRUE(reconcileRuledPendingTargets(spell, ability, state, 0));
    EXPECT_EQ(spell.selectedTargetOids, QVector<quint32>({20}));
    EXPECT_EQ(spell.selectedTargetDamages, QVector<quint32>({2}));
    EXPECT_EQ(spell.targetDamageAllocations, QVector<int>({2}));
}

TEST(RuledPendingTargetTest, ReconcileRepairsBothLegacyPendingFamiliesInOneLegalRefresh)
{
    FakeHost host;
    RuledClientState state(&host);

    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.handIndex = 2;
    spell.activeTargetGroupPosition = 0;
    spell.selectedTargetOids = {10};
    spell.selectedTargetOidsByGroup = {{10}};

    PendingActivatedAbility ability;
    ability.valid = true;
    ability.permanentOid = 30;
    ability.abilityIndex = 1;
    ability.selectedTargetOid = 40;

    RuledSpellTargetData noSpellCandidates;
    noSpellCandidates.groups.append(static_cast<const RuledTargetGroupData &>(noSpellCandidates));
    state.validTargetsByHandSlot.insert(RuledClientState::spellTargetKey(2, 0), noSpellCandidates);
    state.validTargetsByAbility.insert(RuledClientState::abilityTargetKey(30, 1), {});

    EXPECT_TRUE(reconcileRuledPendingTargets(spell, ability, state, 0));
    EXPECT_TRUE(spell.selectedTargetOids.isEmpty());
    EXPECT_EQ(ability.selectedTargetOid, 0u);
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

TEST_F(RuledClientTest, LegalHandSlotRequiresStableServerCardIdentity)
{
    ruled::v1::RuledEventBatch legalOnly;
    auto &actions = (*legalOnly.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_PLAY_LAND, 0, "Forest");
    apply(legalOnly);

    // A newly visible dev-conjured card can arrive before its HandSlotMap refresh. Its visual
    // position must never be treated as engine slot 0, or clicking it plays the existing Forest.
    EXPECT_EQ(state->legalHandSlotForServerCard(ruled::v1::HAND_ACTION_PLAY_LAND, kLocalPlayer, 99), -1);

    ruled::v1::RuledEventBatch mapped;
    auto &mappedActions = (*mapped.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(mappedActions, ruled::v1::HAND_ACTION_PLAY_LAND, 0, "Forest");
    auto *forest = mapped.add_events()->mutable_hand_slot_map()->add_entries();
    forest->set_player_id(kLocalPlayer);
    forest->set_server_card_id(5);
    forest->set_hand_index(0);
    apply(mapped);

    EXPECT_EQ(state->legalHandSlotForServerCard(ruled::v1::HAND_ACTION_PLAY_LAND, kLocalPlayer, 5), 0);
    EXPECT_EQ(state->legalHandSlotForServerCard(ruled::v1::HAND_ACTION_PLAY_LAND, kLocalPlayer, 99), -1);
}

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

    const QVector<RuledFaceOption> faces = state->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, 6);
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

TEST_F(RuledClientTest, OmenFacesOptionalTargetAndStackAnnotationStayEngineAuthored)
{
    ruled::v1::RuledEventBatch legalBatch;
    auto &actions = (*legalBatch.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 6, "Dirgur Island Dragon", 0, false, "{5}{U}");
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 6, "Skimming Strike", 1, true, "{1}{U}");
    auto &targets = (*actions.mutable_valid_targets_by_hand_slot())[(6u << 8) | 1u];
    auto *group = targets.add_groups();
    group->set_group_index(0);
    group->set_prompt_text("Choose up to one target creature");
    group->set_min(0);
    group->set_max(1);
    group->add_valid_permanent_ids(101u);
    apply(legalBatch);

    const QVector<RuledFaceOption> faces = state->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, 6);
    ASSERT_EQ(faces.size(), 2);
    EXPECT_EQ(faces[0].faceName, QStringLiteral("Dirgur Island Dragon"));
    EXPECT_EQ(faces[0].manaCost, QStringLiteral("{5}{U}"));
    EXPECT_EQ(faces[1].faceName, QStringLiteral("Skimming Strike"));
    EXPECT_EQ(faces[1].manaCost, QStringLiteral("{1}{U}"));
    ASSERT_EQ(state->spellTargetData(6, 1).groups.size(), 1);
    EXPECT_TRUE(ruledTargetGroupUsesExplicitConfirmation(state->spellTargetData(6, 1).groups.first()));

    ruled::v1::RuledEventBatch stackBatch;
    auto *push = stackBatch.add_events()->mutable_stack_pushed();
    push->set_object_id(700u);
    push->set_card_id("dirgur_island_dragon_skimming_strike");
    push->set_description("Skimming Strike");
    push->set_ability_annotation("Skimming Strike");
    apply(stackBatch);
    EXPECT_EQ(state->stackAnnotation(700u), QStringLiteral("Skimming Strike"));
    EXPECT_EQ(state->getStackOidOrder(), QList<quint32>({700u}));

    ruled::v1::RuledEventBatch resolvedBatch;
    auto *resolved = resolvedBatch.add_events()->mutable_stack_resolved();
    resolved->set_object_id(700u);
    resolved->set_destination(ruled::v1::STACK_RESOLVE_DESTINATION_LIBRARY);
    apply(resolvedBatch);
    EXPECT_FALSE(state->hasStackItems());
    EXPECT_TRUE(state->stackAnnotation(700u).isEmpty());
}

TEST_F(RuledClientTest, PhysicalSpellPrimaryPresentationDoesNotBecomeStackAnnotation)
{
    ruled::v1::RuledEventBatch ordinaryBatch;
    auto *ordinary = ordinaryBatch.add_events()->mutable_stack_pushed();
    ordinary->set_object_id(701u);
    ordinary->set_card_id("serra_angel");
    ordinary->set_description("Serra Angel");
    ordinary->mutable_primary_presentation()->set_fallback_text("Serra Angel");
    apply(ordinaryBatch);

    EXPECT_TRUE(state->stackAnnotation(701u).isEmpty());

    ruled::v1::RuledEventBatch multiFaceBatch;
    auto *multiFace = multiFaceBatch.add_events()->mutable_stack_pushed();
    multiFace->set_object_id(702u);
    multiFace->set_card_id("fire_ice");
    multiFace->set_description("Fire");
    multiFace->set_ability_annotation("Fire");
    multiFace->mutable_primary_presentation()->set_fallback_text("Fire deals 2 damage.");
    apply(multiFaceBatch);

    EXPECT_EQ(state->stackAnnotation(702u), QStringLiteral("Fire"));
}

TEST_F(RuledClientTest, StackAnnotationKeepsAbilityAndStructuredChoicePresentation)
{
    ruled::v1::RuledEventBatch abilityBatch;
    auto *ability = abilityBatch.add_events()->mutable_stack_pushed();
    ability->set_object_id(703u);
    ability->set_description("Triggered ability");
    ability->set_ability_annotation("Engine fallback ability");
    ability->mutable_primary_presentation()->set_fallback_text("Resolved ability text");
    apply(abilityBatch);

    EXPECT_EQ(state->stackAnnotation(703u), QStringLiteral("Resolved ability text"));

    ruled::v1::RuledEventBatch choicesBatch;
    auto *choices = choicesBatch.add_events()->mutable_stack_pushed();
    choices->set_object_id(704u);
    choices->set_card_id("boros_charm");
    choices->set_description("Boros Charm");
    choices->set_ability_annotation("Fallback mode\nFallback cost");
    choices->mutable_primary_presentation()->set_fallback_text("Boros Charm");
    choices->add_chosen_mode_presentations()->set_fallback_text("Resolved mode text");
    choices->add_chosen_cast_cost_presentations()->set_fallback_text("Resolved cost text");
    apply(choicesBatch);

    EXPECT_EQ(state->stackAnnotation(704u), QStringLiteral("Resolved mode text\nResolved cost text"));

    ruled::v1::RuledEventBatch markerBatch;
    auto *marker = markerBatch.add_events()->mutable_stack_pushed();
    marker->set_object_id(705u);
    marker->set_card_id("fireball");
    marker->set_description("Fireball");
    marker->set_ability_annotation("X = 4");
    marker->mutable_primary_presentation()->set_fallback_text("Fireball");
    apply(markerBatch);

    EXPECT_EQ(state->stackAnnotation(705u), QStringLiteral("X = 4"));
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
    auto *damageGroup = damage->mutable_targets()->add_groups();
    damageGroup->set_group_index(0);
    damageGroup->set_prompt_text("Choose target player");
    damageGroup->set_min(1);
    damageGroup->set_max(1);
    damageGroup->set_can_target_opponent(true);
    auto *disabled = cast->add_modes();
    disabled->set_mode_index(2);
    disabled->set_label("Creature gains double strike");
    disabled->set_selectable(false);
    disabled->set_needs_target(true);
    apply(batch);

    const auto &set = state->handActions[ruled::v1::HAND_ACTION_CAST_SPELL];
    const auto key = RuledClientState::handCastActionKey(5, 0, ruled::v1::CAST_METHOD_NORMAL);
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
    slotTargets.set_is_damage_targets(true);
    slotTargets.set_fixed_damage(4);
    slotTargets.set_extra_mana_per_target(1);
    auto *group = slotTargets.add_groups();
    group->set_group_index(0);
    group->set_prompt_text("Choose up to two target creatures");
    group->set_min(0);
    group->set_max(2);
    group->add_valid_permanent_ids(101);
    group->add_valid_stack_ids(300);
    group->add_valid_graveyard_ids(500);
    group->set_can_target_opponent(true);
    auto *secondGroup = slotTargets.add_groups();
    secondGroup->set_group_index(1);
    secondGroup->set_prompt_text("Choose another target");
    secondGroup->set_min(1);
    secondGroup->set_max(1);
    secondGroup->add_valid_permanent_ids(202);
    secondGroup->add_distinct_from_group_indices(0);
    // Ability index 2 on permanent 100.
    auto &abilityTargets = (*actions.mutable_valid_targets_by_ability())[(quint64(100) << 32) | 2u];
    auto *abilityGroup = abilityTargets.add_groups();
    abilityGroup->set_group_index(0);
    abilityGroup->set_prompt_text("Choose target");
    abilityGroup->set_min(1);
    abilityGroup->set_max(1);
    abilityGroup->add_valid_permanent_ids(200);
    abilityGroup->set_can_target_self(true);
    apply(batch);

    EXPECT_FALSE(state->isValidSpellTarget(1, 0, 100));
    EXPECT_TRUE(state->isValidSpellTarget(1, 0, 101));
    EXPECT_EQ(state->spellTargetData(1, 0).minTargets, 0);
    EXPECT_EQ(state->spellTargetData(1, 0).maxTargets, 2);
    EXPECT_EQ(state->spellTargetData(1, 0).promptText, "Choose up to two target creatures");
    ASSERT_EQ(state->spellTargetData(1, 0).groups.size(), 2);
    EXPECT_EQ(state->spellTargetData(1, 0).groups.at(1).validPermanentIds, QSet<quint32>({202}));
    EXPECT_EQ(state->spellTargetData(1, 0).groups.at(1).distinctFromGroupIndices, QVector<int>({0}));
    // A different face of the same slot carries its own (here: empty) target set.
    EXPECT_FALSE(state->isValidSpellTarget(1, 1, 100));
    EXPECT_TRUE(state->isValidSpellStackTarget(1, 0, 300));
    EXPECT_TRUE(state->isValidSpellGraveyardTarget(1, 0, 500));
    EXPECT_TRUE(state->canSpellTargetOpponent(1, 0));
    EXPECT_FALSE(state->canSpellTargetSelf(1, 0));
    EXPECT_TRUE(state->spellIsDamageTargets(1, 0));
    EXPECT_EQ(state->spellMaxTargets(1, 0), 2);
    EXPECT_EQ(state->spellFixedDamage(1, 0), 4);
    EXPECT_EQ(state->spellExtraManaPerTarget(1, 0), 1);

    EXPECT_TRUE(state->abilityNeedsTarget(100, 2));
    EXPECT_FALSE(state->abilityNeedsTarget(100, 0));
    EXPECT_TRUE(state->isValidAbilityTarget(100, 2, 200));
    EXPECT_TRUE(state->canAbilityTargetSelf(100, 2));
}

TEST_F(RuledClientTest, MultiZoneSearchCarriesSourceAnnotationsAndMalformedMetadataFailsClosed)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_ZONE_SEARCH);
    choice->set_prompt_text("Search the selected zones for Altanak");
    choice->set_min(0);
    choice->set_max(1);
    for (const quint32 oid : {71u, 72u, 73u})
        choice->add_candidate_object_ids(oid);
    for (const int scid : {0, 1, 2})
        choice->add_candidate_server_card_ids(scid);
    for (int i = 0; i < 3; ++i)
        choice->add_candidate_names("Altanak, the Thrice-Called");
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_GRAVEYARD);
    choice->add_candidate_source_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Search selected zones"));
    EXPECT_FALSE(state->resolutionHandPickShowViewControls());
    EXPECT_EQ(state->resolutionHandPickCandidateAnnotations(),
              QStringList({QStringLiteral("Hand"), QStringLiteral("Graveyard"), QStringLiteral("Library")}));
    ASSERT_EQ(started.count(), 1);

    state->clearPendingChoice();
    ruled::v1::RuledEventBatch malformed;
    auto *bad = malformed.add_events()->mutable_resolution_choice_required();
    bad->CopyFrom(*choice);
    bad->mutable_candidate_source_zones()->RemoveLast();
    apply(malformed);
    EXPECT_FALSE(state->isResolutionHandPickActive());
}

TEST_F(RuledClientTest, ParsesAuthoritativeActivatedCostChoicesAndPayability)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    const quint64 key = (quint64(100) << 32) | 2u;
    auto &costs = (*actions.mutable_cost_choices_by_ability())[key];
    costs.set_non_mana_costs_payable(false);
    auto *discard = costs.add_choices();
    discard->set_cost_index(0);
    discard->set_zone(ruled::v1::COST_CHOICE_ZONE_HAND);
    discard->add_candidate_ids(1);
    discard->add_candidate_ids(3);
    auto *sacrifice = costs.add_choices();
    sacrifice->set_cost_index(2);
    sacrifice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
    sacrifice->set_kind(ruled::v1::COST_CHOICE_KIND_SACRIFICE);
    sacrifice->add_candidate_ids(100);
    auto *sacrificeRef = sacrifice->add_candidate_objects();
    sacrificeRef->mutable_object()->set_object_id(100);
    sacrificeRef->mutable_object()->set_zone_change_generation(7);
    auto *graveyard = costs.add_choices();
    graveyard->set_cost_index(3);
    graveyard->set_zone(ruled::v1::COST_CHOICE_ZONE_GRAVEYARD);
    graveyard->set_min(2);
    graveyard->set_max(2);
    graveyard->add_candidate_ids(501);
    graveyard->add_candidate_ids(502);
    apply(batch);

    EXPECT_FALSE(state->abilityActivatable(100, 2));
    const auto choices = state->abilityCostChoices(100, 2);
    ASSERT_EQ(choices.size(), 3);
    EXPECT_EQ(choices.at(0).costIndex, 0);
    EXPECT_EQ(choices.at(0).zone, RuledCostChoiceZone::Hand);
    EXPECT_EQ(choices.at(0).candidateIds, QSet<quint32>({1, 3}));
    EXPECT_EQ(choices.at(1).zone, RuledCostChoiceZone::Battlefield);
    EXPECT_EQ(choices.at(1).kind, RuledCostChoiceKind::Sacrifice);
    EXPECT_TRUE(choices.at(1).candidateIds.contains(100));
    EXPECT_EQ(choices.at(1).candidateGenerations.value(100), 7u);
    EXPECT_EQ(choices.at(2).zone, RuledCostChoiceZone::Graveyard);
    EXPECT_EQ(choices.at(2).min, 2);
    EXPECT_EQ(choices.at(2).max, 2);
    EXPECT_EQ(choices.at(2).candidateIds, QSet<quint32>({501, 502}));
}

TEST(RuledPendingCostSelectionTest, GraveyardSelectionMembershipTracksPhysicalObjectIds)
{
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.waitingForCost = true;
    RuledCostChoice choice;
    choice.costIndex = 3;
    choice.zone = RuledCostChoiceZone::Graveyard;
    choice.candidateIds = {501u, 502u};
    choice.candidateGenerations = {{501u, 7u}, {502u, 8u}};
    choice.min = choice.max = 2;
    pending.costChoices.append(choice);
    pending.costSelections.append(
        {3, RuledCostChoiceZone::Graveyard, QVector<quint32>({501u, 502u}), QVector<quint64>({7u, 8u})});

    EXPECT_TRUE(ruledPendingGraveyardCostSelectionContains(pending, 501u));
    EXPECT_TRUE(ruledPendingGraveyardCostSelectionContains(pending, 502u));
    EXPECT_FALSE(ruledPendingGraveyardCostSelectionContains(pending, 503u));
    pending.waitingForCost = false;
    EXPECT_FALSE(ruledPendingGraveyardCostSelectionContains(pending, 501u));
}

TEST(RuledPendingCostSelectionTest, GraveyardProgressCountsOnlyCurrentEngineCandidates)
{
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.waitingForCost = true;
    pending.nextCostChoice = 0;

    RuledCostChoice choice;
    choice.costIndex = 4;
    choice.zone = RuledCostChoiceZone::Graveyard;
    choice.candidateIds = {501u, 502u};
    choice.candidateGenerations = {{501u, 7u}, {502u, 8u}};
    choice.min = 2;
    choice.max = 2;
    pending.costChoices = {choice};

    // 500 is the activated source and is intentionally absent from the engine-authored cohort.
    // It must not make Confirm legal even if a stale local transaction contains it.
    pending.costSelections = {{4, RuledCostChoiceZone::Graveyard, {500u, 501u, 501u}, {0u, 7u, 7u}}};

    const auto progress = ruledPendingGraveyardCostSelectionProgress(pending);
    ASSERT_TRUE(progress.has_value());
    EXPECT_EQ(progress->required, 2);
    EXPECT_EQ(progress->selected, 1);
    EXPECT_FALSE(progress->confirmable);
    EXPECT_FALSE(ruledPendingGraveyardCostSelectionContains(pending, 500u));
    EXPECT_TRUE(ruledPendingGraveyardCostSelectionContains(pending, 501u));
}

TEST(RuledPendingCostSelectionTest, TapPaymentProgressDoesNotRequestAGraveyardView)
{
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.waitingForCost = true;
    RuledCostChoice choice;
    choice.costIndex = 1;
    choice.zone = RuledCostChoiceZone::Battlefield;
    choice.kind = RuledCostChoiceKind::Tap;
    choice.min = choice.max = 1;
    choice.candidateIds = {501};
    pending.costChoices = {choice};
    const auto progress = ruledPendingGraveyardCostSelectionProgress(pending);
    ASSERT_TRUE(progress.has_value());
    EXPECT_EQ(progress->zone, RuledCostChoiceZone::Battlefield);
    EXPECT_EQ(progress->required, 1);
    EXPECT_FALSE(progress->confirmable);
}

TEST_F(RuledClientTest, AggregateObjectPaymentUsesAuthoritativeContributionsAndGenerations)
{
    ruled::v1::RuledEventBatch batch;
    auto &costs = (*(*batch.mutable_legal_by_player())[kLocalPlayer].mutable_cost_choices_by_ability())[178];
    costs.set_non_mana_costs_payable(true);
    auto *choice = costs.add_choices();
    choice->set_cost_index(0);
    choice->set_zone(ruled::v1::COST_CHOICE_ZONE_GRAVEYARD);
    choice->set_kind(ruled::v1::COST_CHOICE_KIND_EXILE);
    choice->set_max(2);
    choice->mutable_aggregate_minimum()->set_minimum(3);
    choice->mutable_aggregate_minimum()->set_contribution_kind(
        ruled::v1::OBJECT_CONTRIBUTION_KIND_MANA_VALUE);
    for (const auto &[objectId, generation, contribution] :
         std::array<std::tuple<quint32, quint64, qint64>, 2>{{{501, 7, 1}, {502, 9, 2}}}) {
        choice->add_candidate_ids(objectId);
        auto *candidate = choice->add_candidate_objects();
        candidate->mutable_object()->set_object_id(objectId);
        candidate->mutable_object()->set_zone_change_generation(generation);
        candidate->set_contribution(contribution);
    }
    apply(batch);

    const auto parsed = state->abilityCostChoices(0, 178);
    ASSERT_EQ(parsed.size(), 1);
    EXPECT_EQ(parsed[0].aggregateMinimum, 3);
    EXPECT_EQ(parsed[0].contributionKind, RuledObjectContributionKind::ManaValue);
    EXPECT_EQ(parsed[0].candidateContributions.value(502), 2);
    EXPECT_EQ(parsed[0].candidateGenerations.value(502), 9u);

    PendingActivatedAbility pending;
    pending.valid = true;
    pending.waitingForCost = true;
    pending.costChoices = parsed;
    pending.costSelections = {{0, RuledCostChoiceZone::Graveyard, {501}, {7}}};
    auto progress = ruledPendingGraveyardCostSelectionProgress(pending);
    ASSERT_TRUE(progress);
    EXPECT_EQ(progress->selected, 1);
    EXPECT_FALSE(progress->confirmable);
    pending.costSelections[0].selectedIds.append(502);
    pending.costSelections[0].selectedGenerations.append(9);
    progress = ruledPendingGraveyardCostSelectionProgress(pending);
    ASSERT_TRUE(progress);
    EXPECT_EQ(progress->selected, 3);
    EXPECT_TRUE(progress->confirmable);

    ruled::v1::CostSelection command;
    ruledWriteCostObjectRefs(pending.costSelections[0], command);
    ASSERT_EQ(command.graveyard_objects().objects_size(), 2);
    EXPECT_EQ(command.graveyard_objects().objects(1).object_id(), 502u);
    EXPECT_EQ(command.graveyard_objects().objects(1).zone_change_generation(), 9u);
}

TEST_F(RuledClientTest, RequirementSetsSurviveABatchWithoutLegalActions)
{
    ruled::v1::RuledEventBatch withActions;
    auto &actions = (*withActions.mutable_legal_by_player())[kLocalPlayer];
    addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 0, "Grizzly Bears");
    actions.add_required_attacker_ids(100); // CR 508.1d
    actions.add_required_blocker_ids(200);  // CR 509.1c
    actions.add_selectable_attacker_ids(100);
    addLegalPlayerAttack(actions, 100);
    addLegalBlockPair(actions, 200, 300);
    apply(withActions);
    ASSERT_EQ(state->requiredAttackerOids.size(), 1);

    // A Servatrice-synthesized preview echo has no legal_by_player entry: legal actions clear,
    // but the engine-authoritative must-attack / must-block sets must survive.
    ruled::v1::RuledEventBatch preview;
    auto *ap = preview.add_events()->mutable_attackers_preview();
    ap->set_declaring_player_id(kOpponent);
    *ap->add_assignments() = playerAttackAssignment(100);
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

TEST_F(RuledClientTest, RestrictedManaSnapshotsAndPaymentEligibilityStaySeparate)
{
    QSignalSpy manaSpy(state, &RuledClientState::restrictedManaChanged);
    ruled::v1::RuledEventBatch batch;
    auto *pool = batch.add_events()->mutable_mana_pool_updated();
    pool->set_player_id(kLocalPlayer);
    pool->set_r(2); // unrestricted mana remains in the legacy counter snapshot
    auto *restricted = pool->add_restricted_groups();
    restricted->set_restriction_group_id(7);
    restricted->set_r(1);
    restricted->set_display_label("Spend this mana only to cast a creature spell.");

    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *cast = addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 3, "Fire Elemental", 0, false, "3RR");
    cast->add_eligible_restricted_mana_group_ids(7);
    auto &ability = (*actions.mutable_mana_payment_by_ability())[(quint64(100) << 32) | 2u];
    ability.add_eligible_restricted_mana_group_ids(7);
    apply(batch);

    ASSERT_EQ(manaSpy.count(), 1);
    EXPECT_EQ(manaSpy.at(0).at(0).toInt(), kLocalPlayer);
    const auto groups = state->restrictedManaForPlayer(kLocalPlayer);
    ASSERT_EQ(groups.size(), 1);
    EXPECT_EQ(groups.at(0).groupId, 7u);
    EXPECT_EQ(groups.at(0).r, 1);
    EXPECT_EQ(groups.at(0).displayLabel, QStringLiteral("Spend this mana only to cast a creature spell."));
    EXPECT_EQ(state->eligibleRestrictedManaForCast(3, 0, RuledCastSource::Hand), QSet<quint32>({7}));
    EXPECT_EQ(state->eligibleRestrictedManaForAbility(100, 2), QSet<quint32>({7}));

    ruled::v1::RuledEventBatch cleared;
    cleared.add_events()->mutable_mana_pool_updated()->set_player_id(kLocalPlayer);
    apply(cleared);
    EXPECT_TRUE(state->restrictedManaForPlayer(kLocalPlayer).isEmpty());
    EXPECT_TRUE(state->eligibleRestrictedManaForCast(3, 0, RuledCastSource::Hand).isEmpty());
    EXPECT_TRUE(state->eligibleRestrictedManaForAbility(100, 2).isEmpty());
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

TEST_F(RuledClientTest, StackPushPreservesEveryTypedTargetWithoutPlayerIdCollisions)
{
    ruled::v1::RuledEventBatch batch;
    auto *push = batch.add_events()->mutable_stack_pushed();
    push->set_object_id(50);
    auto *first = push->add_targets();
    first->set_object_id(1); // Deliberately collides with kOpponent.
    first->set_group_index(0);
    first->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    auto *second = push->add_targets();
    second->set_object_id(2);
    second->set_group_index(0);
    second->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);

    apply(batch);

    EXPECT_EQ(state->stackTargetsByStackOid.value(50), QVector<quint32>({1, 2}));
    EXPECT_EQ(state->latchedTargetKind(50, 1), RuledTargetItemKind::Battlefield);
    EXPECT_EQ(state->latchedTargetKind(50, 2), RuledTargetItemKind::Battlefield);
}

TEST(RuledTargetRefKindTest, UsesGraveyardCandidateDomainForSelectionPresentation)
{
    RuledTargetGroupData group;
    group.validGraveyardIds.insert(7);

    EXPECT_EQ(ruledTargetRefKind(group, 7, kLocalPlayer), ruled::v1::TARGET_REF_KIND_GRAVEYARD);
}

TEST(RuledTargetGroupConfirmationTest, OptionalSingleTargetNeedsAnExplicitZeroTargetConfirmation)
{
    RuledTargetGroupData optionalCreature;
    optionalCreature.minTargets = 0;
    optionalCreature.maxTargets = 1;

    EXPECT_TRUE(ruledTargetGroupUsesExplicitConfirmation(optionalCreature));

    RuledTargetGroupData requiredSingle;
    requiredSingle.minTargets = 1;
    requiredSingle.maxTargets = 1;
    EXPECT_FALSE(ruledTargetGroupUsesExplicitConfirmation(requiredSingle));

    PendingRuledSpellCast pending;
    pending.valid = true;
    pending.waitingForTarget = true;
    pending.minTargets = 0;
    pending.maxTargets = 1;
    EXPECT_TRUE(ruledPendingTargetSelectionCanConfirm(pending));
}

TEST(RuledRestrictedManaModelTest, FullyStagedGroupConsumesNoLayoutColumn)
{
    RuledRestrictedManaGroup group;
    group.groupId = 7;
    group.r = 1;
    RuledRestrictedManaSelections staged;
    staged[7][QChar('R')] = 1;

    EXPECT_EQ(ruledVisibleRestrictedManaColumnCount({group}, staged), 0);
}

TEST(RuledRestrictedManaModelTest, ReportsOnlyNewlyProducedContributions)
{
    RuledRestrictedManaTracker tracker;
    EXPECT_TRUE(tracker.observe({}).isEmpty());

    RuledRestrictedManaGroup group;
    group.groupId = 7;
    group.r = 1;
    const auto firstProduction = tracker.observe({group});
    ASSERT_EQ(firstProduction.size(), 1);
    EXPECT_EQ(firstProduction.at(0).groupId, 7u);
    EXPECT_EQ(firstProduction.at(0).symbol, QChar('R'));
    EXPECT_EQ(firstProduction.at(0).amount, 1);

    EXPECT_TRUE(tracker.observe({group}).isEmpty());
    group.r = 2;
    const auto secondProduction = tracker.observe({group});
    ASSERT_EQ(secondProduction.size(), 1);
    EXPECT_EQ(secondProduction.at(0).amount, 1);

    group.r = 0;
    EXPECT_TRUE(tracker.observe({group}).isEmpty());
}

TEST(RuledTargetingCostTest, DeduplicatesOneApplicationAcrossGroupsAndTypedIdCollisions)
{
    RuledSpellTargetData data;
    RuledTargetGroupData first;
    first.validPermanentIds.insert(1); // Deliberately collides with kOpponent.
    first.validPermanentIds.insert(10);
    RuledTargetGroupData second;
    second.validPermanentIds.insert(11);
    data.groups = {first, second};

    RuledTargetingCostApplication kopala;
    kopala.applicationId = 99;
    kopala.genericMana = 2;
    kopala.affectedTargets = {
        {ruled::v1::TARGET_REF_KIND_PLAYER, 1},
        {ruled::v1::TARGET_REF_KIND_PERMANENT, 10},
        {ruled::v1::TARGET_REF_KIND_PERMANENT, 11},
    };
    data.targetingCostApplications = {kopala};

    EXPECT_EQ(ruledTargetingCostForSelection(data, {{1, 10}, {11}}, {}, kLocalPlayer), 2);
    EXPECT_EQ(ruledTargetingCostForSelection(data, {{1}, {}}, {}, kLocalPlayer), 0);
}

TEST(RuledTargetingCostTest, SumsDistinctApplicationsButDeduplicatesAcrossModes)
{
    RuledSpellTargetData data;
    RuledTargetGroupData group;
    group.validPermanentIds.insert(20);
    data.groups = {group};
    data.targetingCostApplications = {
        {100, 2, {{ruled::v1::TARGET_REF_KIND_PERMANENT, 20}}},
        {101, 2, {{ruled::v1::TARGET_REF_KIND_PERMANENT, 20}}},
    };
    EXPECT_EQ(ruledTargetingCostForSelection(data, {{20}}, {}, kLocalPlayer), 4);

    PendingRuledSpellCast spell;
    spell.valid = true;
    PendingRuledSpellCast::SelectedMode first;
    first.targets = data;
    first.selectedTargetOids = {20};
    first.selectedTargetOidsByGroup = {{20}};
    PendingRuledSpellCast::SelectedMode second = first;
    spell.selectedModes = {first, second};
    EXPECT_EQ(ruledModalSpellTargetingCost(spell, kLocalPlayer), 4);
}

TEST(RuledTargetedCostReductionTest, DeduplicatesOneApplicationAcrossGroupsAndUsesTypedCandidates)
{
    RuledSpellTargetData data;
    RuledTargetGroupData first;
    first.validPermanentIds.insert(1); // Deliberately collides with kOpponent.
    first.validPermanentIds.insert(20);
    RuledTargetGroupData second;
    second.validPermanentIds.insert(21);
    data.groups = {first, second};

    data.targetedCostReductionApplications = {
        {200,
         3,
         {
             {ruled::v1::TARGET_REF_KIND_PLAYER, 1},
             {ruled::v1::TARGET_REF_KIND_PERMANENT, 20},
             {ruled::v1::TARGET_REF_KIND_PERMANENT, 21},
         }},
    };

    EXPECT_EQ(ruledTargetedCostReductionForSelection(data, {{1, 20}, {21}}, {}, kLocalPlayer), 3);
    EXPECT_EQ(ruledTargetedCostReductionForSelection(data, {{1}, {}}, {}, kLocalPlayer), 0);
}

TEST(RuledTargetedCostReductionTest, SumsDistinctApplicationsButDeduplicatesAcrossModes)
{
    RuledSpellTargetData data;
    RuledTargetGroupData group;
    group.validPermanentIds.insert(30);
    data.groups = {group};
    data.targetedCostReductionApplications = {
        {300, 3, {{ruled::v1::TARGET_REF_KIND_PERMANENT, 30}}},
        {301, 1, {{ruled::v1::TARGET_REF_KIND_PERMANENT, 30}}},
    };
    EXPECT_EQ(ruledTargetedCostReductionForSelection(data, {{30}}, {}, kLocalPlayer), 4);

    PendingRuledSpellCast spell;
    spell.valid = true;
    PendingRuledSpellCast::SelectedMode first;
    first.targets = data;
    first.selectedTargetOids = {30};
    first.selectedTargetOidsByGroup = {{30}};
    PendingRuledSpellCast::SelectedMode second = first;
    spell.selectedModes = {first, second};
    EXPECT_EQ(ruledModalSpellTargetedCostReduction(spell, kLocalPlayer), 4);
}

TEST(RuledTargetingCostTest, AppliesReductionsAfterXAndAllIncreases)
{
    // Base generic includes the chosen X (2); Fireball and targeting taxes add 3; reduction is 4.
    EXPECT_EQ(ruledFinalGenericCost(2, 3, 4), 1);
    EXPECT_EQ(ruledFinalGenericCost(0, 2, 5), 0);
}

TEST_F(RuledClientTest, ParsesRawCostReductionAndTargetApplications)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *cast = addHandAction(actions, ruled::v1::HAND_ACTION_CAST_SPELL, 4, "Test Spell", 0, true, "{3}{U}");
    cast->set_generic_cost_reduction(1);
    auto &targets = (*actions.mutable_valid_targets_by_hand_slot())[quint32(4) << 8];
    auto *group = targets.add_groups();
    group->set_group_index(0);
    group->add_valid_permanent_ids(30);
    auto *application = targets.add_targeting_cost_applications();
    application->set_application_id(700);
    application->set_generic_mana(2);
    auto *candidate = application->add_affected_targets();
    candidate->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    candidate->set_object_id(30);
    auto *reduction = targets.add_targeted_cost_reduction_applications();
    reduction->set_application_id(701);
    reduction->set_generic_mana(3);
    auto *qualifying = reduction->add_qualifying_targets();
    qualifying->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    qualifying->set_object_id(30);

    apply(batch);

    const auto faces = state->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, 4);
    ASSERT_EQ(faces.size(), 1);
    EXPECT_EQ(faces.first().manaCost, QStringLiteral("{3}{U}"));
    EXPECT_EQ(faces.first().genericCostReduction, 1);
    const auto parsed = state->spellTargetData(4, 0, RuledCastSource::Hand);
    ASSERT_EQ(parsed.targetingCostApplications.size(), 1);
    EXPECT_EQ(parsed.targetingCostApplications.first().applicationId, 700u);
    EXPECT_EQ(parsed.targetingCostApplications.first().genericMana, 2);
    ASSERT_EQ(parsed.targetedCostReductionApplications.size(), 1);
    EXPECT_EQ(parsed.targetedCostReductionApplications.first().applicationId, 701u);
    EXPECT_EQ(parsed.targetedCostReductionApplications.first().genericMana, 3);
    ASSERT_EQ(parsed.targetedCostReductionApplications.first().qualifyingTargets.size(), 1);
    EXPECT_EQ(parsed.targetedCostReductionApplications.first().qualifyingTargets.first().oid, 30u);
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

    // The engine explicitly names the countered object; resolving Counterspell retires only
    // Counterspell itself.
    ruled::v1::RuledEventBatch resolve;
    resolve.add_events()->mutable_stack_object_countered()->set_object_id(10);
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

TEST_F(RuledClientTest, TokenActivatedAbilityForwardsItsDisplayIdentityToTheSyntheticStackCard)
{
    ruled::v1::RuledEventBatch batch;
    auto *sp = batch.add_events()->mutable_stack_pushed();
    sp->set_object_id(901);
    sp->set_description("Clue");
    sp->set_ability_annotation("{2}, Sacrifice this token: Draw a card.");
    sp->set_is_triggered(false);
    auto *identity = sp->mutable_source_token_identity();
    identity->set_name("Clue");
    identity->add_types("Artifact");
    identity->add_ability_texts("{2}, Sacrifice this token: Draw a card.");

    apply(batch);

    ASSERT_EQ(host.createdSyntheticCards.size(), 1);
    ASSERT_TRUE(host.createdSyntheticCards[0].sourceTokenIdentity.has_value());
    const auto &forwarded = *host.createdSyntheticCards[0].sourceTokenIdentity;
    EXPECT_EQ(forwarded.name, QStringLiteral("Clue"));
    EXPECT_TRUE(forwarded.basePt.isEmpty());
    EXPECT_TRUE(forwarded.color.isEmpty());
    EXPECT_TRUE(forwarded.keywords.isEmpty());
    EXPECT_EQ(forwarded.abilityTexts, QStringList({QStringLiteral("{2}, Sacrifice this token: Draw a card.")}));
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
    auto *triggerGroup = abilityTargets.add_groups();
    triggerGroup->set_group_index(0);
    triggerGroup->set_prompt_text("Choose target creature card");
    triggerGroup->set_min(1);
    triggerGroup->set_max(1);
    triggerGroup->add_valid_graveyard_ids(500);
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
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_selectable_attacker_ids(100);
    addLegalPlayerAttack(actions, 100);
    apply(batch);
    ASSERT_TRUE(state->localPlayerIsActive());
    host.sentCommands.clear();

    state->togglePendingAttacker(100);
    EXPECT_TRUE(state->isPendingAttacker(100));
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_TRUE(host.sentCommands[0].has_preview_declare_attackers());
    EXPECT_EQ(host.sentCommands[0].preview_declare_attackers().assignments_size(), 1);

    state->togglePendingAttacker(100);
    EXPECT_FALSE(state->isPendingAttacker(100));
    ASSERT_EQ(host.sentCommands.size(), 2);
    EXPECT_EQ(host.sentCommands[1].preview_declare_attackers().assignments_size(), 0);

    ruled::v1::RuledEventBatch declared;
    auto *ad = declared.add_events()->mutable_attackers_declared();
    ad->set_attacking_player_id(kLocalPlayer);
    *ad->add_assignments() = playerAttackAssignment(100);
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
    addLegalPlayerAttack(actions, 100);
    apply(batch);

    EXPECT_FALSE(state->combatDeclarationSatisfied());
    state->togglePendingAttacker(100);
    EXPECT_TRUE(state->combatDeclarationSatisfied());
}

TEST_F(RuledClientTest, MultipleAttackDefendersRequireAnAuthoritativeDestinationBeforeConfirm)
{
    auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    actions.add_selectable_attacker_ids(100);
    *actions.add_legal_attack_assignments() = playerAttackAssignment(100);
    *actions.add_legal_attack_assignments() = permanentAttackAssignment(100, 500, 9);
    apply(batch);
    host.sentCommands.clear();

    state->togglePendingAttacker(100);
    EXPECT_TRUE(state->isPendingAttacker(100));
    EXPECT_TRUE(state->isChoosingAttackDefender());
    EXPECT_FALSE(state->combatDeclarationSatisfied());
    EXPECT_TRUE(state->isLegalAttackPlayerDefender(kOpponent));
    EXPECT_TRUE(state->isLegalAttackPermanentDefender(500));

    ASSERT_TRUE(state->chooseAttackPermanentDefender(500));
    EXPECT_FALSE(state->isChoosingAttackDefender());
    EXPECT_TRUE(state->combatDeclarationSatisfied());
    ASSERT_FALSE(host.sentCommands.isEmpty());
    const auto &preview = host.sentCommands.constLast().preview_declare_attackers();
    ASSERT_EQ(preview.assignments_size(), 1);
    EXPECT_EQ(preview.assignments(0).defender().object_id(), 500u);
    EXPECT_EQ(preview.assignments(0).defender_zone_change_generation(), 9u);

    host.sentCommands.clear();
    state->confirmAttackers();
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_EQ(host.sentCommands[0].declare_attackers().assignments_size(), 1);
    EXPECT_EQ(host.sentCommands[0].declare_attackers().assignments(0).defender().object_id(), 500u);
}

TEST_F(RuledClientTest, MobilizeDefenderChoiceUsesStructuredEngineOptionsAndRestoresAfterRejection)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_ATTACKING_TOKEN_DEFENDER);
    choice->set_min(1);
    choice->set_max(1);
    choice->set_prompt_text("Choose what the Warrior token attacks.");

    auto *player = choice->add_combat_defender_options();
    player->mutable_defender()->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
    player->mutable_defender()->set_object_id(kOpponent);
    player->set_defending_player_id(kOpponent);

    auto *planeswalker = choice->add_combat_defender_options();
    planeswalker->mutable_defender()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    planeswalker->mutable_defender()->set_object_id(500);
    planeswalker->set_defender_zone_change_generation(9);
    planeswalker->set_defending_player_id(kOpponent);

    apply(batch);
    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AttackingTokenDefender));
    EXPECT_TRUE(state->isLegalAttackPlayerDefender(kOpponent));
    EXPECT_TRUE(state->isLegalAttackPermanentDefender(500));
    EXPECT_FALSE(state->isLegalAttackPermanentDefender(501));

    host.sentCommands.clear();
    ASSERT_TRUE(state->chooseAttackPermanentDefender(500));
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submission = host.sentCommands.constFirst().submit_resolution_choice();
    ASSERT_TRUE(submission.has_chosen_combat_defender());
    EXPECT_EQ(submission.chosen_combat_defender().defender().kind(), ruled::v1::TARGET_REF_KIND_PERMANENT);
    EXPECT_EQ(submission.chosen_combat_defender().defender().object_id(), 500u);
    EXPECT_EQ(submission.chosen_combat_defender().defender_zone_change_generation(), 9u);
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AttackingTokenDefender));

    host.answerPendingAck(false);
    EXPECT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AttackingTokenDefender));
    EXPECT_TRUE(state->isLegalAttackPlayerDefender(kOpponent));
}

TEST_F(RuledClientTest, MobilizeAttackersAppendToTheDeclaredCombatCohort)
{
    ruled::v1::RuledEventBatch declared;
    auto *attackers = declared.add_events()->mutable_attackers_declared();
    attackers->set_attacking_player_id(kLocalPlayer);
    *attackers->add_assignments() = playerAttackAssignment(100);
    apply(declared);

    ruled::v1::RuledEventBatch mobilized;
    auto *added = mobilized.add_events()->mutable_attackers_added();
    *added->add_assignments() = permanentAttackAssignment(200, 500, 9);
    apply(mobilized);

    EXPECT_EQ(state->getCurrentAttackerOids(), QSet<quint32>({100u, 200u}));
    ASSERT_TRUE(state->getCurrentAttackAssignments().contains(200));
    EXPECT_EQ(state->getCurrentAttackAssignments().value(200).defender().object_id(), 500u);
    EXPECT_EQ(state->getCurrentAttackAssignments().value(200).defender_zone_change_generation(), 9u);
}

TEST_F(RuledClientTest, BlockerStagingPairsToAnAttackerAndSyncsAPreview)
{
    ruled::v1::RuledEventBatch declared;
    auto *ad = declared.add_events()->mutable_attackers_declared();
    *ad->add_assignments() = playerAttackAssignment(100);
    *ad->add_assignments() = playerAttackAssignment(101); // Only blocker 200 can block this attacker.
    *ad->add_assignments() = playerAttackAssignment(102); // Unblockable: no legal pair targets this attacker.
    apply(declared);
    // The opponent is the active player during our declare-blockers step.
    auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kOpponent);
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *firstPair = actions.add_legal_block_pairs();
    firstPair->set_blocker_id(200);
    firstPair->set_attacker_id(100);
    auto *secondPair = actions.add_legal_block_pairs();
    secondPair->set_blocker_id(201);
    secondPair->set_attacker_id(100);
    auto *thirdPair = actions.add_legal_block_pairs();
    thirdPair->set_blocker_id(200);
    thirdPair->set_attacker_id(101);
    apply(batch);
    ASSERT_TRUE(state->localPlayerIsDefender());
    host.sentCommands.clear();

    state->toggleStagedBlocker(200);
    EXPECT_TRUE(state->hasStagedBlocker());
    EXPECT_TRUE(state->isStagedBlocker(200));

    // No staged blocker has an edge to the unblockable attacker.
    state->pairStagedBlockerToAttacker(102);
    EXPECT_TRUE(state->hasStagedBlocker());
    EXPECT_EQ(state->pendingBlockTargetForBlocker(200), 0u);
    EXPECT_TRUE(host.sentCommands.isEmpty());

    // Pairing multiple staged blockers is all-or-nothing: blocker 200 can block attacker 101,
    // but blocker 201 cannot, so neither blocker is moved and no preview is emitted.
    state->toggleStagedBlocker(201);
    state->pairStagedBlockerToAttacker(101);
    EXPECT_TRUE(state->isStagedBlocker(200));
    EXPECT_TRUE(state->isStagedBlocker(201));
    EXPECT_EQ(state->pendingBlockTargetForBlocker(200), 0u);
    EXPECT_EQ(state->pendingBlockTargetForBlocker(201), 0u);
    EXPECT_TRUE(host.sentCommands.isEmpty());

    state->pairStagedBlockerToAttacker(100);
    EXPECT_FALSE(state->hasStagedBlocker());
    EXPECT_EQ(state->pendingBlockTargetForBlocker(200), 100u);
    EXPECT_EQ(state->pendingBlockTargetForBlocker(201), 100u);
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_TRUE(host.sentCommands[0].has_preview_declare_blockers());
    ASSERT_EQ(host.sentCommands[0].preview_declare_blockers().block_pairs_size(), 2);

    // Pairing to a creature that is not a declared attacker is a no-op.
    state->toggleStagedBlocker(200);
    state->pairStagedBlockerToAttacker(999);
    EXPECT_TRUE(state->isStagedBlocker(200));
}

TEST_F(RuledClientTest, RejectedBlockDeclarationRollsBackTheLocalGuard)
{
    ruled::v1::RuledEventBatch declared;
    *declared.add_events()->mutable_attackers_declared()->add_assignments() = playerAttackAssignment(100);
    apply(declared);
    auto batch = phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kOpponent);
    addLegalBlockPair((*batch.mutable_legal_by_player())[kLocalPlayer], 200, 100);
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
    cast->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast->set_object_id(objectId);
    cast->set_face_index(0);
    cast->set_card_name("Bonecrusher Giant");
    cast->set_cost("{2}{R}");
    cast->set_needs_target(true);
    auto &targets = (*actions.mutable_valid_targets_by_zone_object())[(quint64(objectId) << 8) | 0u];
    auto *zoneGroup = targets.add_groups();
    zoneGroup->set_group_index(0);
    zoneGroup->set_prompt_text("Choose target");
    zoneGroup->set_min(1);
    zoneGroup->set_max(1);
    zoneGroup->add_valid_permanent_ids(99);
    apply(batch);

    ASSERT_TRUE(state->isZoneActionLegal(objectId, RuledCastSource::Exile));
    ASSERT_EQ(state->zoneActionSource(objectId), RuledCastSource::Exile);
    ASSERT_EQ(state->zoneActionCost(objectId, 0, RuledCastSource::Exile), QStringLiteral("{2}{R}"));
    ASSERT_EQ(state->zoneActionFaceOptions(objectId, RuledCastSource::Exile).size(), 1);
    EXPECT_EQ(state->zoneActionFaceOptions(objectId, RuledCastSource::Exile).first().faceName,
              QStringLiteral("Bonecrusher Giant"));
    EXPECT_TRUE(state->isValidSpellTarget(static_cast<int>(objectId), 0, 99, RuledCastSource::Exile));

    apply(phaseBatch(ruled::v1::PHASE_ID_MAIN1, kLocalPlayer));
    EXPECT_FALSE(state->isZoneActionLegal(objectId, RuledCastSource::Exile));
    EXPECT_FALSE(state->isValidSpellTarget(static_cast<int>(objectId), 0, 99, RuledCastSource::Exile));
}

TEST_F(RuledClientTest, CombatStagingIgnoresCreaturesOutsideEngineSelectableSets)
{
    auto attackers = phaseBatch(ruled::v1::PHASE_ID_DECLARE_ATTACKERS, kLocalPlayer);
    auto &attackerActions = (*attackers.mutable_legal_by_player())[kLocalPlayer];
    attackerActions.add_selectable_attacker_ids(100);
    addLegalPlayerAttack(attackerActions, 100);
    apply(attackers);
    host.sentCommands.clear();

    state->togglePendingAttacker(200);
    EXPECT_FALSE(state->isPendingAttacker(200));
    EXPECT_TRUE(host.sentCommands.isEmpty());
    state->togglePendingAttacker(100);
    EXPECT_TRUE(state->isPendingAttacker(100));

    ruled::v1::RuledEventBatch declared;
    *declared.add_events()->mutable_attackers_declared()->add_assignments() = playerAttackAssignment(300);
    apply(declared);
    auto blockers = phaseBatch(ruled::v1::PHASE_ID_DECLARE_BLOCKERS, kOpponent);
    addLegalBlockPair((*blockers.mutable_legal_by_player())[kLocalPlayer], 400, 300);
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
    *ad->add_assignments() = playerAttackAssignment(100);
    *ad->add_assignments() = playerAttackAssignment(101);
    auto *bd = setup.add_events()->mutable_blockers_declared();
    auto *pair = bd->add_block_pairs();
    pair->set_attacker_id(100);
    pair->set_blocker_id(200);
    apply(setup);
    ASSERT_TRUE(state->isCurrentAttacker(100));
    ASSERT_EQ(state->getCommittedBlocks().value(200), 100u);

    // CR 701.19a: regeneration removes the blocker from combat.
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

TEST_F(RuledClientTest, ZoneViewProjectsLoyaltyDefenseAndBattleProtector)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    auto *planeswalker = view->add_battlefield_objects();
    planeswalker->set_object_id(100);
    planeswalker->set_is_planeswalker(true);
    planeswalker->set_loyalty(4);
    auto *battle = view->add_battlefield_objects();
    battle->set_object_id(200);
    battle->set_is_battle(true);
    battle->set_defense(5);
    battle->set_battle_protector_player_id(kOpponent);
    apply(batch);

    EXPECT_EQ(state->loyaltyForPermanentOid(100), 4);
    EXPECT_EQ(state->defenseForPermanentOid(200), 5);
    EXPECT_EQ(state->battleProtectorForPermanentOid(200), kOpponent);
    EXPECT_EQ(state->defenseForPermanentOid(100), -1);

    ruled::v1::RuledEventBatch replacement;
    replacement.add_events()->mutable_zone_view()->add_per_player()->set_player_id(kLocalPlayer);
    apply(replacement);
    EXPECT_EQ(state->loyaltyForPermanentOid(100), -1);
    EXPECT_EQ(state->defenseForPermanentOid(200), -1);
    EXPECT_EQ(state->battleProtectorForPermanentOid(200), -1);
}

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

TEST_F(RuledClientTest, ParsesSpellCostChoicesForHandAndPublicZoneCasts)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *hand = actions.add_hand_actions();
    hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    hand->set_hand_index(3);
    hand->set_face_index(0);
    auto *discard = hand->mutable_cost_choices()->add_choices();
    discard->set_cost_index(0);
    discard->set_zone(ruled::v1::COST_CHOICE_ZONE_HAND);
    discard->add_candidate_ids(1);

    auto *zone = actions.add_zone_cast_actions();
    zone->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    zone->set_object_id(77);
    zone->set_face_index(1);
    auto *sacrifice = zone->mutable_cost_choices()->add_choices();
    sacrifice->set_cost_index(0);
    sacrifice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
    sacrifice->add_candidate_ids(900);
    apply(batch);

    const auto handCosts = state->spellCostData(3, 0, RuledCastSource::Hand);
    ASSERT_EQ(handCosts.choices.size(), 1);
    EXPECT_EQ(handCosts.choices.first().zone, RuledCostChoiceZone::Hand);
    EXPECT_EQ(handCosts.choices.first().candidateIds, QSet<quint32>({1}));
    const auto zoneCosts = state->spellCostData(77, 1, RuledCastSource::Graveyard);
    ASSERT_EQ(zoneCosts.choices.size(), 1);
    EXPECT_EQ(zoneCosts.choices.first().zone, RuledCostChoiceZone::Battlefield);
    EXPECT_EQ(zoneCosts.choices.first().candidateIds, QSet<quint32>({900}));
}

TEST_F(RuledClientTest, ActivatedAbilityMenuLabelsDoNotDuplicateStructuredCosts)
{
    ruled::v1::RuledEventBatch batch;
    auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
    view->set_player_id(kLocalPlayer);
    auto *object = view->add_battlefield_objects();
    object->set_object_id(100);

    auto *manaAbility = object->add_activated_abilities();
    manaAbility->set_text("{2}{R}: Ability text.");
    manaAbility->set_mana_cost("2R");
    manaAbility->set_cost_label("{2}{R}");

    auto *compositeAbility = object->add_activated_abilities();
    compositeAbility->set_text("{2}, {T}, Sacrifice a creature: Draw a card.");
    compositeAbility->set_mana_cost("2");
    compositeAbility->set_cost_label("{2}, {T}, Sacrifice a creature");
    apply(batch);

    EXPECT_EQ(state->activatedAbilityMenuLabel(100, 0), QStringLiteral("{2}{R}: Ability text."));
    EXPECT_EQ(state->activatedAbilityMenuLabel(100, 1), QStringLiteral("{2}, {T}, Sacrifice a creature: Draw a card."));
}

TEST_F(RuledClientTest, ActivatedAbilityAvailabilityTracksTheEngineAcrossFullZoneViews)
{
    auto availabilityBatch = [](bool activatable) {
        ruled::v1::RuledEventBatch batch;
        auto *view = batch.add_events()->mutable_zone_view()->add_per_player();
        view->set_player_id(kLocalPlayer);
        auto *object = view->add_battlefield_objects();
        object->set_object_id(100);
        auto *ability = object->add_activated_abilities();
        ability->set_text("{1}{W}, {T}: Tap target creature. Activate only if you control a creature with flying.");
        ability->set_activatable(activatable);
        return batch;
    };

    apply(availabilityBatch(false));
    EXPECT_FALSE(state->abilityActivatable(100, 0));
    EXPECT_EQ(state->activatedAbilityMenuLabel(100, 0),
              QStringLiteral("{1}{W}, {T}: Tap target creature. Activate only if you control a creature with flying."));

    apply(availabilityBatch(true));
    EXPECT_TRUE(state->abilityActivatable(100, 0));
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
    EXPECT_TRUE(state->abilityActivatable(100, 0));
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

TEST_F(RuledClientTest, ManaPaymentChoiceCreatesRefreshesAndSerializesDecisions)
{
    QSignalSpy paymentUiSpy(state, &RuledClientState::resolutionPaymentUiChanged);
    auto paymentBatch = [](bool legal) {
        ruled::v1::RuledEventBatch batch;
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
        rcr->set_prompt_text("Pay {4} or decline.");
        rcr->set_generic_mana_cost(4);
        rcr->set_payment_currently_legal(legal);
        return batch;
    };

    apply(paymentBatch(false));
    ASSERT_EQ(paymentUiSpy.count(), 1);
    EXPECT_TRUE(paymentUiSpy.at(0).at(0).toBool());
    ASSERT_TRUE(state->isResolutionPaymentActive());
    EXPECT_EQ(state->resolutionPaymentGenericCost(), 4);
    EXPECT_EQ(state->resolutionPaymentRemainingCost(), QStringLiteral("{4}"));
    EXPECT_FALSE(state->resolutionPaymentCurrentlyLegal());
    state->payResolutionMana();
    EXPECT_TRUE(host.sentCommands.isEmpty());

    state->payment.begin(true);
    ruled::v1::RuledCommand paymentCommand;
    paymentCommand.mutable_submit_resolution_choice()->set_decision(
        ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
    const auto previewRequest = state->payment.requestAction(paymentCommand);
    ruled::v1::RuledEventBatch previewBatch;
    auto *preview = previewBatch.mutable_payment_preview();
    preview->set_transaction_id(previewRequest.transaction_id());
    preview->set_revision(previewRequest.revision());
    preview->set_valid(true);
    preview->set_remaining_cost("{3}");
    apply(previewBatch);
    EXPECT_EQ(state->resolutionPaymentRemainingCost(), QStringLiteral("{3}"));

    apply(paymentBatch(true));
    ASSERT_EQ(paymentUiSpy.count(), 2);
    EXPECT_TRUE(paymentUiSpy.at(1).at(0).toBool());
    ASSERT_TRUE(state->resolutionPaymentCurrentlyLegal());
    state->payResolutionMana();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().decision(),
              ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().chosen_object_ids_size(), 0);
    EXPECT_FALSE(state->isResolutionPaymentActive());
    host.answerPendingAck(true);
}

TEST_F(RuledClientTest, RejectedManaPaymentRestoresThePrompt)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    rcr->set_prompt_text("Pay {4} or decline.");
    rcr->set_generic_mana_cost(4);
    rcr->set_payment_currently_legal(true);
    apply(batch);

    state->declineResolutionMana();
    ASSERT_FALSE(state->isResolutionPaymentActive());
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands.last().submit_resolution_choice().decision(),
              ruled::v1::RESOLUTION_CHOICE_DECISION_DECLINE);
    host.answerPendingAck(false);
    EXPECT_TRUE(state->isResolutionPaymentActive());
    EXPECT_TRUE(state->resolutionPaymentCurrentlyLegal());
}

TEST_F(RuledClientTest, ManaPaymentPromptsOnlyTheDecidingPlayer)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer + 1);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    rcr->set_generic_mana_cost(4);
    rcr->set_payment_currently_legal(true);
    apply(batch);
    EXPECT_FALSE(state->isResolutionPaymentActive());
    EXPECT_TRUE(state->isWaitingForResolutionChoice());
    EXPECT_EQ(state->resolutionChoiceWaitingPlayer(), kOpponent);

    ruled::v1::RuledEventBatch completed;
    apply(completed);
    EXPECT_FALSE(state->isWaitingForResolutionChoice());
}

TEST_F(RuledClientTest, FaceDownObjectMapProvidesPrivateIdentityAndPrunesOnReplacement)
{
    ruled::v1::RuledEventBatch batch;
    auto *entry = batch.add_events()->mutable_face_down_object_map()->add_entries();
    entry->set_controller_player_id(kLocalPlayer);
    entry->set_engine_object_id(9801u);
    entry->set_zone_change_generation(4u);
    entry->set_server_card_id(17);
    entry->set_card_name("Grizzly Bears");
    apply(batch);

    EXPECT_EQ(state->privateFaceDownNameForCard(kLocalPlayer, 17), QString("Grizzly Bears"));
    EXPECT_EQ(state->privateFaceDownGenerationByOid.value(9801u), 4u);
    EXPECT_TRUE(state->privateFaceDownNameForCard(kOpponent, 17).isEmpty());

    ruled::v1::RuledEventBatch cleared;
    cleared.add_events()->mutable_face_down_object_map();
    apply(cleared);
    EXPECT_TRUE(state->privateFaceDownNameForCard(kLocalPlayer, 17).isEmpty());
    EXPECT_FALSE(state->privateFaceDownGenerationByOid.contains(9801u));
}

TEST_F(RuledClientTest, MultiplePermanentActionsAreEngineAuthoredAndClearWhenNoLongerPublished)
{
    ruled::v1::RuledEventBatch batch;
    auto *action = (*batch.mutable_legal_by_player())[kLocalPlayer].add_permanent_actions();
    action->set_kind(ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP);
    action->set_object_id(9802u);
    action->set_zone_change_generation(9u);
    action->set_label("Turn face up — {1}{G}");
    action->set_mana_cost("{1}{G}");
    action->add_eligible_restricted_mana_group_ids(7u);
    auto *unlock = (*batch.mutable_legal_by_player())[kLocalPlayer].add_permanent_actions();
    unlock->set_kind(ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR);
    unlock->set_object_id(9802u);
    unlock->set_zone_change_generation(9u);
    unlock->set_label("Unlock Tunnel of Hate — {4}{R}{R}");
    unlock->set_mana_cost("{4}{R}{R}");
    unlock->set_face_index(1u);
    unlock->add_eligible_restricted_mana_group_ids(8u);
    apply(batch);

    const auto parsed = state->permanentActionsForOid(9802u);
    ASSERT_EQ(parsed.size(), 2);
    EXPECT_EQ(parsed[0].kind, ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP);
    EXPECT_EQ(parsed[0].zoneChangeGeneration, 9u);
    EXPECT_EQ(parsed[0].label, QString("Turn face up — {1}{G}"));
    EXPECT_EQ(parsed[0].manaCost, QString("{1}{G}"));
    EXPECT_EQ(parsed[0].eligibleRestrictedManaGroupIds, QSet<quint32>({7u}));
    EXPECT_FALSE(parsed[0].faceIndex.has_value());
    EXPECT_EQ(parsed[1].kind, ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR);
    ASSERT_TRUE(parsed[1].faceIndex.has_value());
    EXPECT_EQ(*parsed[1].faceIndex, 1u);
    EXPECT_EQ(parsed[1].eligibleRestrictedManaGroupIds, QSet<quint32>({8u}));
    const auto current = state->permanentActionFor(9802u, 9u, ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR,
                                                   std::optional<quint32>{1u});
    ASSERT_TRUE(current.has_value());
    EXPECT_EQ(current->label, QString("Unlock Tunnel of Hate — {4}{R}{R}"));
    EXPECT_FALSE(state->permanentActionFor(9802u, 10u, ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR,
                                           std::optional<quint32>{1u}));
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.permanentAction = true;
    pending.permanentActionKind = ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR;
    pending.permanentActionFaceIndex = 1u;
    pending.permanentOid = 9802u;
    pending.expectedZoneChangeGeneration = 9u;
    pending.abilityIndex = -1;
    EXPECT_TRUE(ruledPendingAbilitySourceStillCurrent(*state, pending));
    pending.expectedZoneChangeGeneration = 10u;
    EXPECT_FALSE(ruledPendingAbilitySourceStillCurrent(*state, pending));

    action->clear_eligible_restricted_mana_group_ids();
    action->add_eligible_restricted_mana_group_ids(9u);
    unlock->clear_eligible_restricted_mana_group_ids();
    apply(batch);
    const auto refreshed = state->permanentActionsForOid(9802u);
    ASSERT_EQ(refreshed.size(), 2);
    EXPECT_EQ(refreshed[0].eligibleRestrictedManaGroupIds, QSet<quint32>({9u}));
    EXPECT_TRUE(refreshed[1].eligibleRestrictedManaGroupIds.isEmpty());

    apply(ruled::v1::RuledEventBatch{});
    EXPECT_TRUE(state->permanentActionsForOid(9802u).isEmpty());
}

TEST_F(RuledClientTest, ZoneAbilityActionsBindHandSlotsAndGraveyardObjectsWithoutLeakingAcrossBatches)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *hand = actions.add_zone_ability_actions();
    hand->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    hand->set_object_id(7101u);
    hand->set_zone_change_generation(4u);
    hand->set_hand_index(3u);
    hand->set_ability_index(0u);
    hand->set_card_name("Shepherding Spirits");
    hand->mutable_ability()->set_text("Plainscycling {2}");
    hand->mutable_ability()->set_mana_cost("{2}");
    hand->mutable_ability()->set_cost_label("{2}, Discard this card");
    hand->mutable_ability()->set_activatable(true);

    auto *graveyard = actions.add_zone_ability_actions();
    graveyard->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    graveyard->set_object_id(7201u);
    graveyard->set_zone_change_generation(8u);
    graveyard->set_ability_index(2u);
    graveyard->set_card_name("Sagu Pummeler");
    graveyard->mutable_ability()->set_text("Renew {4}{G}");
    graveyard->mutable_ability()->set_mana_cost("{4}{G}");
    graveyard->mutable_ability()->set_activatable(false);
    apply(batch);

    EXPECT_EQ(state->zoneAbilityOidForHandSlot(3), 7101u);
    EXPECT_EQ(state->abilitySourceZone(7101u), ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    EXPECT_EQ(state->abilitySourceGeneration(7101u), 4u);
    EXPECT_EQ(state->activatedAbilitiesForOid(7101u).value(0), QString("Plainscycling {2}"));
    EXPECT_EQ(state->activatedAbilityManaCostsForOid(7101u).value(0), QString("{2}"));
    EXPECT_TRUE(state->abilityActivatable(7101u, 0));
    EXPECT_EQ(state->abilitySourceZone(7201u), ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    EXPECT_EQ(state->activatedAbilityIndicesForOid(7201u), QList<int>({2}));
    EXPECT_FALSE(state->abilityActivatable(7201u, 2));

    apply(ruled::v1::RuledEventBatch{});
    EXPECT_EQ(state->zoneAbilityOidForHandSlot(3), 0u);
    EXPECT_TRUE(state->activatedAbilitiesForOid(7101u).isEmpty());
    EXPECT_TRUE(state->activatedAbilityIndicesForOid(7201u).isEmpty());
}

TEST_F(RuledClientTest, MerchantGraveyardActionSubmitsItsExactPublicZoneIdentity)
{
    ruled::v1::RuledEventBatch batch;
    auto &localActions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *merchant = localActions.add_zone_ability_actions();
    merchant->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    merchant->set_object_id(7301u);
    merchant->set_zone_change_generation(12u);
    merchant->set_ability_index(0u);
    merchant->set_card_name("Merchant of Many Hats");
    merchant->mutable_ability()->set_text("{2}{B}: Return this card from your graveyard to your hand.");
    merchant->mutable_ability()->set_mana_cost("{2}{B}");
    merchant->mutable_ability()->set_activatable(true);
    auto *opponentOnly = (*batch.mutable_legal_by_player())[kLocalPlayer + 1].add_zone_ability_actions();
    opponentOnly->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    opponentOnly->set_object_id(9999u);
    opponentOnly->set_zone_change_generation(1u);
    opponentOnly->set_ability_index(0u);
    opponentOnly->mutable_ability()->set_activatable(true);
    apply(batch);

    EXPECT_EQ(state->abilitySourceZone(7301u), ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    EXPECT_EQ(state->abilitySourceGeneration(7301u), 12u);
    EXPECT_TRUE(state->abilityActivatable(7301u, 0));
    EXPECT_TRUE(state->activatedAbilityIndicesForOid(9999u).isEmpty());

    PendingActivatedAbility pending;
    pending.permanentOid = 7301u;
    pending.abilityIndex = 0;
    pending.sourceZone = state->abilitySourceZone(7301u);
    pending.expectedZoneChangeGeneration = state->abilitySourceGeneration(7301u);
    ruled::v1::ActivateAbility command;
    pending.writeActivationHeader(command);
    EXPECT_EQ(command.source_object_id(), 7301u);
    EXPECT_EQ(command.source_zone(), ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    EXPECT_EQ(command.expected_zone_change_generation(), 12u);
}

TEST_F(RuledClientTest, CastAndCycleAreCombinedIntoOneCardActionMenuModel)
{
    RuledFaceOption face;
    face.faceIndex = 0;
    face.faceName = QStringLiteral("Shepherding Spirits");
    const auto options = RuledPendingCast::cardActionMenuOptions(
        {face}, {0}, {QStringLiteral("{2}, Discard this card: Plainscycling {2}")}, {{0, true}});

    ASSERT_EQ(options.size(), 2);
    EXPECT_EQ(options.at(0).kind, RuledCardActionMenuOption::Kind::CastFace);
    EXPECT_EQ(options.at(0).index, 0);
    EXPECT_EQ(options.at(0).label, QStringLiteral("Cast Shepherding Spirits"));
    EXPECT_TRUE(options.at(0).enabled);
    EXPECT_EQ(options.at(1).kind, RuledCardActionMenuOption::Kind::ActivateAbility);
    EXPECT_EQ(options.at(1).index, 0);
    EXPECT_EQ(options.at(1).label, QStringLiteral("{2}, Discard this card: Plainscycling {2}"));
    EXPECT_TRUE(options.at(1).enabled);
}

TEST(RuledPendingCastTest, ManaPaymentMenuRetainsEveryEngineOptionAndItsIndex)
{
    const auto options = RuledPendingCast::cardActionMenuOptions(
        {}, {0, 2}, {"Tap another permanent: Add one mana of any color.", "", "Add blue or green."},
        {{0, true}, {2, false}}, {"W/U/B/R/G", "", "U/G"});
    ASSERT_EQ(options.size(), 7);
    for (int i = 0; i < 5; ++i) {
        EXPECT_EQ(options.at(i).index, 0);
        EXPECT_EQ(options.at(i).manaOptionIndex, i);
        EXPECT_TRUE(options.at(i).enabled);
    }
    EXPECT_TRUE(options.at(3).label.contains("{R}"));
    EXPECT_EQ(options.at(6).index, 2);
    EXPECT_EQ(options.at(6).manaOptionIndex, 1);
    EXPECT_FALSE(options.at(6).enabled);
}

TEST(RuledPendingCastTest, NestedPaymentMenuOffersOnlyEnginePublishedManaAbilities)
{
    RuledFaceOption face;
    face.faceName = QStringLiteral("Test spell");
    const auto options = RuledPendingCast::cardActionMenuOptions(
        {face}, {0, 1, 2}, {"Waterbend {5}: Put a counter.", "{T}: Add {G}.", "{T}: Add {U} or {G}."},
        {{0, true}, {1, true}, {2, false}}, {"", "G", "U/G"}, true);
    ASSERT_EQ(options.size(), 3);
    EXPECT_EQ(options.at(0).kind, RuledCardActionMenuOption::Kind::ActivateAbility);
    EXPECT_EQ(options.at(0).index, 1);
    EXPECT_TRUE(options.at(0).enabled);
    EXPECT_EQ(options.at(2).index, 2);
    EXPECT_EQ(options.at(2).manaOptionIndex, 1);
    EXPECT_FALSE(options.at(2).enabled);
}

TEST(RuledPendingCastTest, PaymentCandidateAndManaAbilityShareOneCardMenu)
{
    const QVector<QPair<int, QString>> waterbend = {
        {ruled::v1::OBJECT_PAYMENT_KIND_WATERBEND, QStringLiteral("Waterbend — pay {1}")}};
    const auto waterbendOptions = RuledPendingCast::cardActionMenuOptions(
        {}, {0}, {QStringLiteral("{T}: Add {G}.")}, {{0, true}}, {QStringLiteral("G")}, true, waterbend);
    ASSERT_EQ(waterbendOptions.size(), 2);
    EXPECT_EQ(waterbendOptions.at(0).kind, RuledCardActionMenuOption::Kind::PaymentContribution);
    EXPECT_EQ(waterbendOptions.at(0).index, ruled::v1::OBJECT_PAYMENT_KIND_WATERBEND);
    EXPECT_EQ(waterbendOptions.at(1).kind, RuledCardActionMenuOption::Kind::ActivateAbility);

    const QVector<QPair<int, QString>> convoke = {
        {ruled::v1::OBJECT_PAYMENT_KIND_GREEN, QStringLiteral("Convoke — pay {G}")},
        {ruled::v1::OBJECT_PAYMENT_KIND_GENERIC, QStringLiteral("Convoke — pay {1}")}};
    const auto convokeOptions = RuledPendingCast::cardActionMenuOptions(
        {}, {0}, {QStringLiteral("{T}: Add {G}.")}, {{0, true}}, {QStringLiteral("G")}, true, convoke);
    ASSERT_EQ(convokeOptions.size(), 3);
    EXPECT_EQ(convokeOptions.at(0).kind, RuledCardActionMenuOption::Kind::PaymentContribution);
    EXPECT_EQ(convokeOptions.at(1).kind, RuledCardActionMenuOption::Kind::PaymentContribution);
    EXPECT_EQ(convokeOptions.at(2).kind, RuledCardActionMenuOption::Kind::ActivateAbility);
}

TEST(RuledPendingCastTest, ActivationHeaderPreservesSelectedManaOptionAndSourceIdentity)
{
    PendingActivatedAbility pending;
    pending.permanentOid = 501;
    pending.abilityIndex = 2;
    pending.manaOptionIndex = 3;
    pending.expectedZoneChangeGeneration = 7;
    ruled::v1::ActivateAbility command;
    pending.writeActivationHeader(command);
    EXPECT_EQ(command.source_object_id(), 501u);
    EXPECT_EQ(command.ability_index(), 2u);
    EXPECT_EQ(command.expected_zone_change_generation(), 7u);
    EXPECT_EQ(command.mana_option_index(), 3u);
}

TEST_F(RuledClientTest, TriggerModesBecomePromptOptionsAndSubmitTheChosenMode)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(2);
    tnt->set_ability_text("Choose one.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *life = tnt->add_modes();
    life->set_mode_index(0);
    life->set_label("Gain 4 life");
    life->set_selectable(true);
    auto *counter = tnt->add_modes();
    counter->set_mode_index(1);
    counter->set_label("Put a counter on it");
    counter->set_selectable(true);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::TriggerMode));
    ASSERT_EQ(state->pendingChoiceOptions().size(), 2);
    state->submitPendingChoiceOption(1);
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_TRUE(host.sentCommands[0].has_choose_trigger_target());
    ASSERT_EQ(host.sentCommands[0].choose_trigger_target().selected_modes_size(), 1);
    EXPECT_EQ(host.sentCommands[0].choose_trigger_target().selected_modes(0).mode_index(), 1u);
}

TEST_F(RuledClientTest, TargetedTriggerModeCarriesItsModeIntoTheTargetCommand)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(2);
    tnt->set_ability_text("Choose one.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *mode = tnt->add_modes();
    mode->set_mode_index(3);
    mode->set_label("Target creature gets +2/+2");
    mode->set_selectable(true);
    mode->set_needs_target(true);
    mode->mutable_targets()->add_groups()->add_valid_permanent_ids(101);
    apply(batch);

    state->submitPendingChoiceOption(3);
    ASSERT_TRUE(state->hasPendingTriggerTarget());
    EXPECT_TRUE(state->abilityTargetData(100, 2).validPermanentIds.contains(101));
    ruled::v1::ChooseTriggerTarget command;
    state->appendPendingTriggerMode(&command);
    ASSERT_EQ(command.selected_modes_size(), 1);
    EXPECT_EQ(command.selected_modes(0).mode_index(), 3u);
}

TEST_F(RuledClientTest, NonModalTriggerPublishesItsClickTargets)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(2);
    tnt->set_ability_text("Deal 3 damage to any target.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *group = tnt->mutable_targets()->add_groups();
    group->set_group_index(0);
    group->set_min(1);
    group->set_max(1);
    group->set_prompt_text("Choose a target");
    group->add_valid_permanent_ids(101);
    group->set_can_target_opponent(true);
    apply(batch);

    ASSERT_TRUE(state->hasPendingTriggerTarget());
    EXPECT_EQ(state->pendingTriggerTargetPrompt(),
              QString::fromUtf8("Choose a target for “Deal 3 damage to any target.”"));
    EXPECT_EQ(state->pendingTriggerTargetDisplayText(),
              QString::fromUtf8("Choose a target for “Deal 3 damage to any target.”"));
    const auto targets = state->abilityTargetData(100, 2);
    EXPECT_TRUE(targets.validPermanentIds.contains(101));
    EXPECT_TRUE(targets.canTargetOpponent);
}

TEST_F(RuledClientTest, TriggerTargetPromptPreservesSpecificGuidance)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Exile up to two target cards from a single graveyard.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *group = tnt->mutable_targets()->add_groups();
    group->set_group_index(0);
    group->set_prompt_text("Choose up to two target cards from a single graveyard.");
    group->set_min(0);
    group->set_max(2);
    apply(batch);

    EXPECT_EQ(state->pendingTriggerTargetPrompt(),
              QString::fromUtf8("Choose targets for “Exile up to two target cards from a single graveyard.”\n"
                                "Choose up to two target cards from a single graveyard."));
    EXPECT_EQ(state->pendingTriggerTargetDisplayText(),
              QString::fromUtf8("Choose targets for “Exile up to two target cards from a single graveyard.”\n"
                                "Choose up to two target cards from a single graveyard.\n"
                                "Selected: 0 (0–2)."));
}

TEST_F(RuledClientTest, TriggerTargetPromptAdvancesSequentialGroupGuidance)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Choose one artifact and one creature.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *artifact = tnt->mutable_targets()->add_groups();
    artifact->set_group_index(3);
    artifact->set_prompt_text("Choose target artifact.");
    artifact->set_min(1);
    artifact->set_max(1);
    artifact->add_valid_permanent_ids(701);
    auto *creature = tnt->mutable_targets()->add_groups();
    creature->set_group_index(7);
    creature->set_prompt_text("Choose target creature.");
    creature->set_min(1);
    creature->set_max(1);
    creature->add_valid_permanent_ids(702);
    apply(batch);

    EXPECT_EQ(state->pendingTriggerTargetPrompt(),
              QString::fromUtf8("Choose targets for “Choose one artifact and one creature.”\n"
                                "Target 1 of 2: Choose target artifact."));
    ASSERT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 701, kLocalPlayer));
    EXPECT_EQ(state->pendingTriggerTargetPrompt(),
              QString::fromUtf8("Choose targets for “Choose one artifact and one creature.”\n"
                                "Target 2 of 2: Choose target creature."));
}

TEST_F(RuledClientTest, ResolutionBranchesSubmitOpaqueIndexWithoutOpeningADialog)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    choice->set_prompt_text("Choose a payment.");
    choice->set_min(0);
    auto *sacrifice = choice->add_resolution_branches();
    sacrifice->set_branch_index(0);
    sacrifice->set_label("Sacrifice a creature");
    sacrifice->set_selectable(true);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::ResolutionBranch));
    EXPECT_EQ(host.dialogRequests, 0);
    state->submitPendingChoiceOption(0);
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submission = host.sentCommands[0].submit_resolution_choice();
    EXPECT_EQ(submission.decision(), ruled::v1::RESOLUTION_CHOICE_DECISION_SELECT_BRANCH);
    EXPECT_EQ(submission.selected_branch_index(), 0u);
}

TEST_F(RuledClientTest, SiegeCastPromptSubmitsTransformedAnnouncementOrDecline)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_SIEGE_CAST);
    choice->set_prompt_text("Cast Grandmother Ravi Sengir transformed?");
    choice->add_candidate_object_ids(700);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::SiegeCast));
    ASSERT_EQ(state->pendingChoiceOptions().size(), 2);
    host.sentCommands.clear();
    state->submitPendingChoiceOption(1);
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submission = host.sentCommands[0].submit_resolution_choice();
    EXPECT_EQ(submission.decision(), ruled::v1::RESOLUTION_CHOICE_DECISION_CAST_TRANSFORMED);
    ASSERT_TRUE(submission.has_cast_spell());
    EXPECT_EQ(submission.cast_spell().cast_method(), ruled::v1::CAST_METHOD_SIEGE_DEFEAT);
    EXPECT_EQ(submission.cast_spell().face_index(), 1u);
    EXPECT_EQ(submission.cast_spell().source().exile_object_id(), 700u);

    apply(batch);
    host.sentCommands.clear();
    state->submitPendingChoiceOption(0);
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().decision(),
              ruled::v1::RESOLUTION_CHOICE_DECISION_DECLINE);
    EXPECT_FALSE(host.sentCommands[0].submit_resolution_choice().has_cast_spell());
}

TEST_F(RuledClientTest, VariableTriggerTargetsStageOneGraveyardCohortAndRestoreOnRejection)
{
    QSignalSpy selectionChanged(state, &RuledClientState::triggerTargetSelectionChanged);
    ruled::v1::RuledEventBatch batch;
    auto *mapping = batch.add_events()->mutable_graveyard_object_map();
    auto *mineOne = mapping->add_entries();
    mineOne->set_player_id(kLocalPlayer);
    mineOne->set_engine_object_id(501);
    mineOne->set_server_card_id(11);
    auto *mineTwo = mapping->add_entries();
    mineTwo->set_player_id(kLocalPlayer);
    mineTwo->set_engine_object_id(502);
    mineTwo->set_server_card_id(12);
    auto *theirs = mapping->add_entries();
    theirs->set_player_id(kOpponent);
    theirs->set_engine_object_id(601);
    theirs->set_server_card_id(21);
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Exile up to two target cards from a single graveyard.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *group = tnt->mutable_targets()->add_groups();
    group->set_group_index(0);
    group->set_prompt_text("Choose up to two target cards from a single graveyard");
    group->set_min(0);
    group->set_max(2);
    group->set_same_graveyard(true);
    group->add_valid_graveyard_ids(501);
    group->add_valid_graveyard_ids(502);
    group->add_valid_graveyard_ids(601);
    apply(batch);

    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_GRAVEYARD, 501, kLocalPlayer));
    EXPECT_EQ(selectionChanged.count(), 1);
    EXPECT_FALSE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_GRAVEYARD, 601, kLocalPlayer));
    EXPECT_EQ(selectionChanged.count(), 1);
    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_GRAVEYARD, 502, kLocalPlayer));
    EXPECT_EQ(selectionChanged.count(), 2);
    EXPECT_EQ(state->pendingTriggerSelectedCount(), 2);
    EXPECT_TRUE(state->isPendingTriggerTargetSelected(501));
    EXPECT_TRUE(state->isPendingTriggerTargetSelected(502));

    state->confirmPendingTriggerTargets();
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_EQ(host.sentCommands[0].choose_trigger_target().targets_size(), 2);
    EXPECT_TRUE(state->hasPendingTriggerTarget());
    host.answerPendingAck(false);
    EXPECT_TRUE(state->hasPendingTriggerTarget());
    EXPECT_EQ(state->pendingTriggerSelectedCount(), 2);
}

TEST_F(RuledClientTest, ExactOneTriggerTargetSubmitsImmediately)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Choose target creature.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *group = tnt->mutable_targets()->add_groups();
    group->set_group_index(0);
    group->set_min(1);
    group->set_max(1);
    group->add_valid_permanent_ids(701);
    apply(batch);

    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 701, kLocalPlayer));
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].choose_trigger_target().targets(0).object_id(), 701u);
}

TEST_F(RuledClientTest, TriggerTargetGroupsAdvanceInOrderAndPreserveAuthoredIndices)
{
    ruled::v1::RuledEventBatch batch;
    auto *tnt = batch.add_events()->mutable_trigger_needs_target();
    tnt->set_source_permanent_id(100);
    tnt->set_ability_index(0);
    tnt->set_ability_text("Choose two targets.");
    tnt->set_controller_player_id(kLocalPlayer);
    auto *first = tnt->mutable_targets()->add_groups();
    first->set_group_index(3);
    first->set_min(1);
    first->set_max(1);
    first->add_valid_permanent_ids(701);
    auto *second = tnt->mutable_targets()->add_groups();
    second->set_group_index(7);
    second->set_min(1);
    second->set_max(1);
    second->add_valid_permanent_ids(702);
    apply(batch);

    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 701, kLocalPlayer));
    EXPECT_TRUE(host.sentCommands.isEmpty());
    EXPECT_TRUE(state->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PERMANENT, 702, kLocalPlayer));
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &targets = host.sentCommands[0].choose_trigger_target().targets();
    ASSERT_EQ(targets.size(), 2);
    EXPECT_EQ(targets.Get(0).group_index(), 3u);
    EXPECT_EQ(targets.Get(1).group_index(), 7u);
}

TEST_F(RuledClientTest, ParsesMethodAwareHarmonizeActionAndAuthoritativeCreatureReductions)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *flashback = actions.add_zone_cast_actions();
    flashback->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_GRAVEYARD);
    flashback->set_object_id(77);
    flashback->set_face_index(0);
    flashback->set_card_name("Unending Whisper");
    flashback->set_cost("{6}{U}");
    flashback->set_cast_method(ruled::v1::CAST_METHOD_FLASHBACK);

    auto *harmonize = actions.add_zone_cast_actions();
    harmonize->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_GRAVEYARD);
    harmonize->set_object_id(77);
    harmonize->set_face_index(0);
    harmonize->set_card_name("Unending Whisper");
    harmonize->set_cost("{5}{U}");
    harmonize->set_cast_method(ruled::v1::CAST_METHOD_HARMONIZE);
    auto *group = harmonize->mutable_cost_choices()->add_cast_cost_groups();
    group->set_group_index(0);
    group->set_min(0);
    group->set_max(1);
    group->set_prompt("Choose a creature for Harmonize.");
    group->set_skip_label("Pay full Harmonize cost");
    auto *option = group->add_options();
    option->set_option_index(0);
    option->set_label("Tap a creature");
    option->set_kind(ruled::v1::CAST_COST_OPTION_KIND_TAP_PERMANENT_FOR_GENERIC_REDUCTION);
    option->set_selectable(true);
    option->add_valid_permanent_ids(900);
    option->add_valid_permanent_generations(12);
    option->add_valid_permanent_generic_reductions(4);
    apply(batch);

    const auto faces = state->zoneActionFaceOptions(77, RuledCastSource::Graveyard);
    ASSERT_EQ(faces.size(), 2);
    EXPECT_EQ(faces.at(0).castMethod, ruled::v1::CAST_METHOD_FLASHBACK);
    EXPECT_EQ(faces.at(1).castMethod, ruled::v1::CAST_METHOD_HARMONIZE);
    EXPECT_EQ(state->zoneActionCost(77, 0, RuledCastSource::Graveyard, ruled::v1::CAST_METHOD_FLASHBACK),
              QStringLiteral("{6}{U}"));
    EXPECT_EQ(state->zoneActionCost(77, 0, RuledCastSource::Graveyard, ruled::v1::CAST_METHOD_HARMONIZE),
              QStringLiteral("{5}{U}"));
    const auto costs = state->spellCostData(77, 0, RuledCastSource::Graveyard, ruled::v1::CAST_METHOD_HARMONIZE);
    ASSERT_EQ(costs.castCostGroups.size(), 1);
    EXPECT_EQ(costs.castCostGroups[0].skipLabel, QStringLiteral("Pay full Harmonize cost"));
    ASSERT_EQ(costs.castCostGroups[0].options.size(), 1);
    EXPECT_EQ(costs.castCostGroups[0].options[0].kind, RuledCastCostOptionKind::TapPermanentForGenericReduction);
    EXPECT_EQ(costs.castCostGroups[0].options[0].validPermanentGenerations.value(900), 12u);
    EXPECT_EQ(costs.castCostGroups[0].options[0].validPermanentGenericReductions.value(900), 4);
}

TEST_F(RuledClientTest, WarpAndNormalHandOffersKeepDistinctMethodsAndCostChoices)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    for (const auto method : {ruled::v1::CAST_METHOD_NORMAL, ruled::v1::CAST_METHOD_WARP}) {
        auto *action = actions.add_hand_actions();
        action->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
        action->set_hand_index(3);
        action->set_card_name("Knight Luminary");
        action->set_cast_method(method);
        action->set_cost(method == ruled::v1::CAST_METHOD_WARP ? "{1}{W}" : "{3}{W}");
        action->add_eligible_restricted_mana_group_ids(static_cast<quint32>(method));
    }
    apply(batch);
    const auto options = state->handActionFaceOptions(ruled::v1::HAND_ACTION_CAST_SPELL, 3);
    ASSERT_EQ(options.size(), 2);
    EXPECT_EQ(options[0].castMethod, ruled::v1::CAST_METHOD_NORMAL);
    EXPECT_EQ(options[1].castMethod, ruled::v1::CAST_METHOD_WARP);
    EXPECT_EQ(options[1].manaCost, QStringLiteral("{1}{W}"));
    const auto menu = RuledPendingCast::cardActionMenuOptions(options, {}, {}, {});
    EXPECT_EQ(menu[1].label, QStringLiteral("Warp Knight Luminary ({1}{W})"));
    EXPECT_EQ(menu[1].castMethod, ruled::v1::CAST_METHOD_WARP);
    EXPECT_EQ(state->eligibleRestrictedManaForCast(3, 0, RuledCastSource::Hand, ruled::v1::CAST_METHOD_NORMAL),
              QSet<quint32>{static_cast<quint32>(ruled::v1::CAST_METHOD_NORMAL)});
    EXPECT_EQ(state->eligibleRestrictedManaForCast(3, 0, RuledCastSource::Hand, ruled::v1::CAST_METHOD_WARP),
              QSet<quint32>{static_cast<quint32>(ruled::v1::CAST_METHOD_WARP)});
    EXPECT_NE(RuledClientState::handCastActionKey(3, 0, ruled::v1::CAST_METHOD_NORMAL),
              RuledClientState::handCastActionKey(4, 0, ruled::v1::CAST_METHOD_NORMAL));
}

TEST_F(RuledClientTest, ExilePermissionOffersKeepOpaqueIdentityCostAndSourceLabel)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    for (const auto [id, cost, source] :
         {std::tuple<quint64, const char *, const char *>{41, "{2}", "Airbending Lesson"},
          std::tuple<quint64, const char *, const char *>{42, "", "Release to the Wind"}}) {
        auto *group = actions.add_exile_play_permission_groups();
        group->set_group_id(id);
        group->set_source_label(source);
        group->add_object_ids(77);
        auto *action = actions.add_zone_cast_actions();
        action->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
        action->set_object_id(77);
        action->set_card_name("Grizzly Bears");
        action->set_cost(cost);
        action->set_cast_method(ruled::v1::CAST_METHOD_PERMISSION);
        action->set_zone_change_generation(9);
        action->set_casting_permission_id(id);
    }
    apply(batch);

    const auto faces = state->zoneActionFaceOptions(77, RuledCastSource::Exile);
    ASSERT_EQ(faces.size(), 2);
    EXPECT_EQ(faces[0].castingPermissionId, 41u);
    EXPECT_EQ(faces[0].permissionSourceLabel, QStringLiteral("Airbending Lesson"));
    EXPECT_EQ(faces[1].castingPermissionId, 42u);
    EXPECT_EQ(state->zoneActionCost(77, 0, RuledCastSource::Exile, ruled::v1::CAST_METHOD_PERMISSION, 41),
              QStringLiteral("{2}"));
    EXPECT_TRUE(state->zoneActionCost(77, 0, RuledCastSource::Exile, ruled::v1::CAST_METHOD_PERMISSION, 42).isEmpty());
    const auto menu = RuledPendingCast::cardActionMenuOptions(faces, {}, {}, {});
    ASSERT_EQ(menu.size(), 2);
    EXPECT_EQ(menu[0].label, QStringLiteral("Cast Grizzly Bears — Airbending Lesson ({2})"));
    EXPECT_EQ(menu[1].label, QStringLiteral("Cast Grizzly Bears — Release to the Wind"));
}

TEST_F(RuledClientTest, PublicCastGenerationRejectsAnOldSelectionAfterReexile)
{
    ruled::v1::RuledEventBatch batch;
    auto *action = (*batch.mutable_legal_by_player())[kLocalPlayer].add_zone_cast_actions();
    action->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    action->set_object_id(77);
    action->set_card_name("Knight Luminary");
    action->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    action->set_zone_change_generation(4);
    apply(batch);
    EXPECT_TRUE(state->isZoneCastActionLegal(77, 0, RuledCastSource::Exile, ruled::v1::CAST_METHOD_NORMAL, 0, 4));
    action->set_zone_change_generation(6);
    apply(batch);
    EXPECT_FALSE(state->isZoneCastActionLegal(77, 0, RuledCastSource::Exile, ruled::v1::CAST_METHOD_NORMAL, 0, 4));
    EXPECT_TRUE(state->isZoneCastActionLegal(77, 0, RuledCastSource::Exile, ruled::v1::CAST_METHOD_NORMAL, 0, 6));
}

TEST_F(RuledClientTest, PublicZoneCastActionsRequireTheClickedSourceZone)
{
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *harmonize = actions.add_zone_cast_actions();
    harmonize->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_GRAVEYARD);
    harmonize->set_object_id(77);
    harmonize->set_face_index(0);
    harmonize->set_card_name("Unending Whisper");
    harmonize->set_cost("{5}{U}");
    harmonize->set_cast_method(ruled::v1::CAST_METHOD_HARMONIZE);
    apply(batch);

    EXPECT_TRUE(state->isZoneActionLegal(77, RuledCastSource::Graveyard));
    EXPECT_FALSE(state->isZoneActionLegal(77, RuledCastSource::Exile));
    EXPECT_EQ(state->zoneActionFaceOptions(77, RuledCastSource::Graveyard).size(), 1);
    EXPECT_TRUE(state->zoneActionFaceOptions(77, RuledCastSource::Exile).isEmpty());
    EXPECT_EQ(state->zoneActionCost(77, 0, RuledCastSource::Graveyard, ruled::v1::CAST_METHOD_HARMONIZE),
              QStringLiteral("{5}{U}"));
    EXPECT_TRUE(state->zoneActionCost(77, 0, RuledCastSource::Exile, ruled::v1::CAST_METHOD_HARMONIZE).isEmpty());
}

TEST_F(RuledClientTest, AppliesExileLandActionsAndStablePermissionGroupSnapshots)
{
    constexpr quint32 objectId = 700;
    ruled::v1::RuledEventBatch batch;
    auto &actions = (*batch.mutable_legal_by_player())[kLocalPlayer];
    auto *land = actions.add_zone_land_actions();
    land->set_source_zone(ruled::v1::CAST_SOURCE_ZONE_EXILE);
    land->set_object_id(objectId);
    land->set_face_index(1);
    land->set_card_name("Timbercrown Pathway");
    auto *group = actions.add_exile_play_permission_groups();
    group->set_group_id(42);
    group->set_source_label("Clockwork Percussionist");
    group->add_object_ids(objectId);
    auto *overlap = actions.add_exile_play_permission_groups();
    overlap->set_group_id(43);
    overlap->set_source_label("Second permission");
    overlap->add_object_ids(objectId);

    QSignalSpy changed(state, &RuledClientState::exilePlayPermissionGroupsChanged);
    apply(batch);
    ASSERT_TRUE(state->isZoneLandActionLegal(objectId));
    ASSERT_EQ(state->zoneLandFaceOptions(objectId).size(), 1);
    EXPECT_EQ(state->zoneLandFaceOptions(objectId).first().faceName, QStringLiteral("Timbercrown Pathway"));
    ASSERT_EQ(state->exilePlayPermissionGroups.size(), 2);
    EXPECT_EQ(state->exilePlayPermissionGroups.value(42).sourceLabel, QStringLiteral("Clockwork Percussionist"));
    EXPECT_EQ(state->exilePlayPermissionGroups.value(42).objectIds, QVector<quint32>{objectId});
    EXPECT_EQ(state->exilePlayPermissionGroups.value(43).objectIds, QVector<quint32>{objectId});
    EXPECT_EQ(changed.count(), 1);

    apply(batch);
    EXPECT_EQ(changed.count(), 1) << "an identical snapshot must not reopen a dismissed popup";

    auto cleared = phaseBatch(ruled::v1::PHASE_ID_MAIN1, kLocalPlayer);
    (*cleared.mutable_legal_by_player())[kLocalPlayer];
    apply(cleared);
    EXPECT_FALSE(state->isZoneLandActionLegal(objectId));
    EXPECT_TRUE(state->exilePlayPermissionGroups.isEmpty());
    EXPECT_EQ(changed.count(), 2);
}

TEST_F(RuledClientTest, CounterRemovalCostsAreRecognized)
{
    ruled::v1::RuledEventBatch batch;
    auto *hand = (*batch.mutable_legal_by_player())[kLocalPlayer].add_hand_actions();
    hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    hand->set_hand_index(3);
    auto *choice = hand->mutable_cost_choices()->add_choices();
    choice->set_kind(ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS);
    choice->set_cost_index(1);
    choice->set_min(1);
    choice->set_max(1);
    auto *removal = choice->mutable_counter_removal();
    removal->mutable_source()->set_object_id(900);
    removal->mutable_source()->set_zone_change_generation(12);
    removal->set_count(1);
    auto *option = removal->add_options();
    option->set_option_id(3);
    option->set_label("stun");
    option->set_available_count(2);
    apply(batch);
    const auto costs = state->spellCostData(3, 0, RuledCastSource::Hand);
    ASSERT_EQ(costs.choices.size(), 1);
    EXPECT_NE(costs.choices[0].kind, RuledCostChoiceKind::Unspecified);
    const auto &parsed = costs.choices.front();
    ASSERT_EQ(parsed.counterOptions.size(), 1);
    EXPECT_EQ(parsed.counterOptions.front().label, QStringLiteral("stun"));
    PendingActivatedAbility pending;
    pending.valid = true;
    pending.permanentOid = 900;
    pending.expectedZoneChangeGeneration = 12;
    pending.costChoices = costs.choices;
    EXPECT_TRUE(RuledPendingCast::chooseCounterCosts(nullptr, pending));
    ASSERT_EQ(pending.costSelections.size(), 1);
    EXPECT_EQ(pending.nextCostChoice, 1);
    EXPECT_TRUE(ruledCounterSelectionStillLegal(pending.costSelections.front(), parsed));
    ruled::v1::CostSelection command;
    ruledWriteCounterRemoval(pending.costSelections.front(), command);
    EXPECT_EQ(command.counter_removal().option_id(), 3u);
    EXPECT_EQ(command.counter_removal().source().object_id(), 900u);
    EXPECT_EQ(command.counter_removal().source().zone_change_generation(), 12u);
    auto stale = parsed;
    ++stale.counterSourceGeneration;
    EXPECT_FALSE(ruledCounterSelectionStillLegal(pending.costSelections.front(), stale));
    stale = parsed;
    stale.counterOptions.clear();
    EXPECT_FALSE(ruledCounterSelectionStillLegal(pending.costSelections.front(), stale));
}

TEST_F(RuledClientTest, BlightCostsUseAnAuthoritativeCreaturePicker)
{
    ruled::v1::RuledEventBatch batch;
    auto *hand = (*batch.mutable_legal_by_player())[kLocalPlayer].add_hand_actions();
    hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    hand->set_hand_index(3);
    auto *choice = hand->mutable_cost_choices()->add_choices();
    choice->set_cost_index(1);
    choice->set_zone(ruled::v1::COST_CHOICE_ZONE_BATTLEFIELD);
    choice->set_kind(ruled::v1::COST_CHOICE_KIND_BLIGHT);
    choice->set_blight_count(2);
    choice->set_min(1);
    choice->set_max(1);
    choice->add_candidate_ids(900);
    auto *ref = choice->add_candidate_objects();
    ref->mutable_object()->set_object_id(900);
    ref->mutable_object()->set_zone_change_generation(12);
    auto *group = hand->mutable_cost_choices()->add_cast_cost_groups();
    group->set_group_index(0);
    group->set_max(1);
    auto *option = group->add_options();
    option->set_kind(ruled::v1::CAST_COST_OPTION_KIND_BLIGHT);
    option->set_label("Blight 2");
    option->set_selectable(true);
    option->add_valid_permanent_ids(900);
    option->add_valid_permanent_generations(12);
    apply(batch);
    const auto costs = state->spellCostData(3, 0, RuledCastSource::Hand);
    ASSERT_EQ(costs.choices.size(), 1);
    EXPECT_EQ(costs.choices[0].kind, RuledCostChoiceKind::Blight);
    EXPECT_EQ(costs.choices[0].blightCount, 2u);
    EXPECT_TRUE(ruledCostSelectionPrompt(costs.choices[0], QStringLiteral("Wild Unraveling")).contains("Blight 2"));
    PendingRuledSpellCast pending;
    pending.valid = true;
    pending.waitingForCost = true;
    pending.costChoices = costs.choices;
    EXPECT_TRUE(ruledPendingGraveyardCostSelectionProgress(pending).has_value());
    pending.costSelections = {{1, RuledCostChoiceZone::Battlefield, {900}, {12}}};
    EXPECT_TRUE(ruledPendingGraveyardCostSelectionProgress(pending)->confirmable);
    ruled::v1::CostSelection command;
    ruledWriteCostObjectRefs(pending.costSelections[0], command);
    ASSERT_EQ(command.battlefield_objects().objects_size(), 1);
    EXPECT_EQ(command.battlefield_objects().objects(0).object_id(), 900u);
    EXPECT_EQ(command.battlefield_objects().objects(0).zone_change_generation(), 12u);
    pending.costSelections[0].selectedGenerations[0] = 11;
    EXPECT_FALSE(ruledPendingGraveyardCostSelectionProgress(pending)->confirmable);
    pending.waitingForCastCostObject = true;
    pending.activeCastCostOption = 0;
    pending.castCostGroups = costs.castCostGroups;
    EXPECT_EQ(ruledCastCostObjectEligibility(pending, RuledCastCostCandidateKind::Permanent, 900),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledCastCostObjectEligibility(pending, RuledCastCostCandidateKind::Permanent, 901),
              RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledCastCostObjectEligibility(pending, RuledCastCostCandidateKind::Hand, 900),
              RuledTargetClickEligibility::Illegal);
}

TEST(RuledPendingCostSelectionTest, BlightCanShareACreatureWithTapAndSacrifice)
{
    RuledCostChoice blight;
    blight.costIndex = 1;
    blight.kind = RuledCostChoiceKind::Blight;
    RuledCostChoice sacrifice;
    sacrifice.costIndex = 2;
    sacrifice.kind = RuledCostChoiceKind::Sacrifice;
    const QVector<RuledCostChoice> choices{blight, sacrifice};
    EXPECT_FALSE(
        ruledCostSelectionConflicts(sacrifice, choices, {1, RuledCostChoiceZone::Battlefield, {900}, {12}}, 900));
    EXPECT_FALSE(ruledCostSelectionConflicts(blight, choices, {2, RuledCostChoiceZone::Battlefield, {900}}, 900));
}

TEST_F(RuledClientTest, ParsesEngineAuthoredOptionalCastCostGroups)
{
    ruled::v1::RuledEventBatch batch;
    auto *hand = (*batch.mutable_legal_by_player())[kLocalPlayer].add_hand_actions();
    hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    hand->set_hand_index(3);
    auto *group = hand->mutable_cost_choices()->add_cast_cost_groups();
    group->set_group_index(0);
    group->set_prompt("Choose an additional cost.");
    group->set_min(0);
    group->set_max(1);
    auto *kicker = group->add_options();
    kicker->set_option_index(0);
    kicker->set_label("Kicker {2}");
    kicker->set_kind(ruled::v1::CAST_COST_OPTION_KIND_MANA);
    kicker->set_additional_mana_cost("{2}");
    kicker->set_selectable(true);
    auto *behold = group->add_options();
    behold->set_option_index(1);
    behold->set_label("Behold a Dragon");
    behold->set_kind(ruled::v1::CAST_COST_OPTION_KIND_BEHOLD);
    behold->add_valid_hand_indices(7);
    behold->add_valid_permanent_ids(900);
    behold->add_valid_permanent_generations(12);
    behold->set_selectable(true);
    apply(batch);

    const auto costs = state->spellCostData(3, 0, RuledCastSource::Hand);
    ASSERT_EQ(costs.castCostGroups.size(), 1);
    EXPECT_EQ(costs.castCostGroups.first().prompt, QStringLiteral("Choose an additional cost."));
    ASSERT_EQ(costs.castCostGroups.first().options.size(), 2);
    EXPECT_EQ(costs.castCostGroups.first().options.at(0).additionalManaCost, QStringLiteral("{2}"));
    EXPECT_EQ(costs.castCostGroups.first().options.at(1).validHandIndices, QSet<quint32>({7}));
    EXPECT_EQ(costs.castCostGroups.first().options.at(1).validPermanentIds, QSet<quint32>({900}));
    EXPECT_EQ(costs.castCostGroups.first().options.at(1).validPermanentGenerations.value(900), 12u);
}

TEST_F(RuledClientTest, ParsesModeLinkedCastCostCoordinates)
{
    ruled::v1::RuledEventBatch batch;
    auto *hand = (*batch.mutable_legal_by_player())[kLocalPlayer].add_hand_actions();
    hand->set_kind(ruled::v1::HAND_ACTION_CAST_SPELL);
    hand->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    hand->set_hand_index(3);
    auto *mode = hand->add_modes();
    mode->set_mode_index(7);
    mode->set_label("Create a Spirit token.");
    mode->set_selectable(true);
    mode->mutable_linked_cast_cost()->set_group_index(2);
    mode->mutable_linked_cast_cost()->set_option_index(5);
    apply(batch);

    const RuledCastActionKey key{3, 0, RuledCastSource::Hand, ruled::v1::CAST_METHOD_NORMAL, 0};
    const auto modes = state->handActions.value(ruled::v1::HAND_ACTION_CAST_SPELL).modalOptionsByCastKey.value(key);
    ASSERT_EQ(modes.size(), 1);
    EXPECT_EQ(modes.first().linkedCastCostGroupIndex, 2);
    EXPECT_EQ(modes.first().linkedCastCostOptionIndex, 5);
}

TEST_F(RuledClientTest, ParsesPublicPermanentResolutionChoiceAsUntargetedBoardClick)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
    choice->set_min(1);
    choice->set_max(1);
    choice->set_prompt_text("Choose a creature you control.");
    choice->add_candidate_object_ids(901);
    apply(batch);

    EXPECT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::PermanentChoice));
    EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::PermanentChoice, 901));
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, *state, RuledTargetCandidateKind::Battlefield, 901, kLocalPlayer),
              RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledTargetClickEligibility({}, {}, *state, RuledTargetCandidateKind::Stack, 901, kLocalPlayer),
              RuledTargetClickEligibility::Illegal);
}

TEST_F(RuledClientTest, ActiveCastRevealsReplaceAsExactSnapshotsAndClear)
{
    QSignalSpy changed(state, &RuledClientState::activePublicRevealsChanged);
    ruled::v1::RuledEventBatch first;
    auto *snapshot = first.add_events()->mutable_active_public_reveal_snapshot();
    auto *caustic = snapshot->add_reveals();
    caustic->set_source_stack_object_id(700);
    caustic->set_group_index(0);
    caustic->set_revealing_player_id(kLocalPlayer);
    caustic->set_source_description("Caustic Exhale");
    caustic->set_card_id("adult_gold_dragon");
    caustic->set_card_name("Adult Gold Dragon");
    auto *osseous = snapshot->add_reveals();
    osseous->set_source_stack_object_id(701);
    osseous->set_group_index(0);
    osseous->set_revealing_player_id(kOpponent);
    osseous->set_source_description("Osseous Exhale");
    osseous->set_card_id("shivan_dragon");
    osseous->set_card_name("Shivan Dragon");
    apply(first);

    ASSERT_EQ(state->getActivePublicReveals().size(), 2);
    ASSERT_EQ(changed.count(), 1);
    EXPECT_EQ(changed.at(0).at(0).toStringList(),
              QStringList({QStringLiteral("Adult Gold Dragon"), QStringLiteral("Shivan Dragon")}));
    EXPECT_EQ(changed.at(0).at(1).value<QVector<int>>(), QVector<int>({kLocalPlayer, kOpponent}));

    ruled::v1::RuledEventBatch second;
    auto *replacement = second.add_events()->mutable_active_public_reveal_snapshot()->add_reveals();
    replacement->set_source_stack_object_id(701);
    replacement->set_group_index(0);
    replacement->set_revealing_player_id(kOpponent);
    replacement->set_source_description("Osseous Exhale");
    replacement->set_card_id("shivan_dragon");
    replacement->set_card_name("Shivan Dragon");
    apply(second);
    ASSERT_EQ(state->getActivePublicReveals().size(), 1);
    EXPECT_EQ(state->getActivePublicReveals().first().sourceStackObjectId, 701u);

    ruled::v1::RuledEventBatch cleared;
    cleared.add_events()->mutable_active_public_reveal_snapshot();
    apply(cleared);
    EXPECT_TRUE(state->getActivePublicReveals().isEmpty());
    ASSERT_EQ(changed.count(), 3);
    EXPECT_TRUE(changed.at(2).at(0).toStringList().isEmpty());
}

TEST_F(RuledClientTest, MandatoryResolutionBranchesCannotSubmitDecline)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    choice->set_prompt_text("Put the permanent on top or bottom.");
    choice->set_min(1);
    choice->set_max(1);
    for (int index = 0; index < 2; ++index) {
        auto *branch = choice->add_resolution_branches();
        branch->set_branch_index(index);
        branch->set_label(index == 0 ? "Top" : "Bottom");
        branch->set_selectable(true);
    }
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::ResolutionBranch));
    EXPECT_FALSE(state->pendingClickChoiceMayDecline());
    state->declinePendingClickChoice();
    EXPECT_TRUE(host.sentCommands.isEmpty());
    EXPECT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::ResolutionBranch));
}

TEST_F(RuledClientTest, SearchZoneBranchesRemainStructuredAndSubmitTheirOpaqueCombinationIndex)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    choice->set_prompt_text("Choose which zones to search.");
    for (int index = 0; index < 7; ++index) {
        auto *branch = choice->add_resolution_branches();
        branch->set_branch_index(index);
        branch->set_label(QStringLiteral("branch %1").arg(index).toStdString());
        branch->set_selectable(true);
        if (index == 0 || index == 3 || index == 4 || index == 6) {
            branch->add_search_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
        }
        if (index == 1 || index == 3 || index == 5 || index == 6) {
            branch->add_search_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_GRAVEYARD);
        }
        if (index == 2 || index == 4 || index == 5 || index == 6) {
            branch->add_search_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
        }
    }
    apply(batch);

    ASSERT_TRUE(state->hasPendingZoneScopeChoice());
    ASSERT_EQ(state->pendingChoiceOptions().size(), 7);
    EXPECT_EQ(
        state->pendingChoiceOptions().at(4).searchZones,
        QSet<int>({ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND, ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY}));
    state->submitPendingChoiceOption(4);
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().selected_branch_index(), 4u);
}

TEST_F(RuledClientTest, MixedStructuredAndOrdinaryResolutionBranchesFailClosed)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    auto *structured = choice->add_resolution_branches();
    structured->set_branch_index(0);
    structured->set_selectable(true);
    structured->add_search_zones(ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND);
    auto *missing = choice->add_resolution_branches();
    missing->set_branch_index(1);
    missing->set_selectable(true);
    apply(batch);

    EXPECT_FALSE(state->hasPendingChoiceOptions());
}

TEST_F(RuledClientTest, PrivateHandChoiceMakesTheNonDecidingPlayerWait)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer + 1);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    rcr->set_prompt_text("Opponent is making a resolution choice.");
    apply(batch);

    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_TRUE(state->isWaitingForResolutionChoice());
    EXPECT_EQ(state->resolutionChoiceWaitingPlayer(), kOpponent);

    ruled::v1::RuledEventBatch completed;
    apply(completed);
    EXPECT_FALSE(state->isWaitingForResolutionChoice());
}

TEST_F(RuledClientTest, SequentialPlayerSetDiscardReplacesPickWithWaitAndNextPrivatePick)
{
    ruled::v1::RuledEventBatch first;
    auto *firstChoice = first.add_events()->mutable_resolution_choice_required();
    firstChoice->set_deciding_player_id(kLocalPlayer);
    firstChoice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    firstChoice->set_prompt_text("Choose one card to discard.");
    firstChoice->set_min(1);
    firstChoice->set_max(1);
    firstChoice->add_candidate_object_ids(501u);
    firstChoice->add_candidate_server_card_ids(1);
    apply(first);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    state->toggleResolutionHandPickCard(1);
    EXPECT_EQ(state->resolutionHandPickSelected(), 1);

    ruled::v1::RuledEventBatch waiting;
    auto *waitChoice = waiting.add_events()->mutable_resolution_choice_required();
    waitChoice->set_deciding_player_id(kOpponent);
    waitChoice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    waitChoice->set_prompt_text("Opponent is making a resolution choice.");
    apply(waiting);
    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickSelected(), 0);
    EXPECT_TRUE(state->isWaitingForResolutionChoice());
    EXPECT_EQ(state->resolutionChoiceWaitingPlayer(), kOpponent);

    ruled::v1::RuledEventBatch nextPrivate;
    auto *nextChoice = nextPrivate.add_events()->mutable_resolution_choice_required();
    nextChoice->set_deciding_player_id(kLocalPlayer);
    nextChoice->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    nextChoice->set_prompt_text("Choose one card to discard.");
    nextChoice->set_min(1);
    nextChoice->set_max(1);
    nextChoice->add_candidate_object_ids(601u);
    nextChoice->add_candidate_server_card_ids(2);
    apply(nextPrivate);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_FALSE(state->isWaitingForResolutionChoice());
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(1));
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(2));

    state->toggleResolutionHandPickCard(2);
    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids_size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 601u);
}

TEST(RuledManaPoolTrackerTest, OptimisticStagingDoesNotTurnOldManaIntoNewProduction)
{
    RuledManaPoolTracker tracker;

    const auto first = tracker.observe(7, 0, 1, 0);
    EXPECT_EQ(first.newlyProduced, 1);
    EXPECT_EQ(first.displayedBeforeNewStaging, 1);

    // The first produced pip is staged and hidden locally. The next engine value is 2, but only
    // one pip is new and only that one may be auto-applied.
    const auto second = tracker.observe(7, 0, 2, 1);
    EXPECT_EQ(second.newlyProduced, 1);
    EXPECT_EQ(second.displayedBeforeNewStaging, 1);

    const auto undoSecond = tracker.observe(7, 0, 1, 1);
    EXPECT_EQ(undoSecond.newlyProduced, 0);
    EXPECT_EQ(undoSecond.displayedBeforeNewStaging, 0);
    const auto undoFirst = tracker.observe(7, 0, 0, 0);
    EXPECT_EQ(undoFirst.newlyProduced, 0);
    EXPECT_EQ(undoFirst.displayedBeforeNewStaging, 0);
}

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
    EXPECT_TRUE(state->resolutionHandPickShowViewControls());
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

TEST_F(RuledClientTest, HeterogeneousLibrarySearchUsesTheEngineAuthoredDistinctSlotGraph)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
    rcr->set_prompt_text("Search for up to two cards.");
    rcr->set_min(0);
    rcr->set_max(2);
    for (const quint32 oid : {101u, 102u, 103u, 104u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1, 2, 3}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    for (const char *name : {"Dryad Shrine", "Forest", "Sanctum", "Island"}) {
        rcr->add_candidate_names(name);
    }
    auto *basic = rcr->add_selection_slots();
    basic->set_label("a basic land card");
    for (const uint32_t index : {0u, 1u, 3u}) {
        basic->add_candidate_indices(index);
    }
    auto *shrine = rcr->add_selection_slots();
    shrine->set_label("a Shrine card");
    for (const uint32_t index : {0u, 2u}) {
        shrine->add_candidate_indices(index);
    }
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_TRUE(state->resolutionHandPickPromptText().contains(QStringLiteral("a basic land card")));
    EXPECT_TRUE(state->resolutionHandPickPromptText().contains(QStringLiteral("a Shrine card")));
    state->toggleResolutionHandPickCard(1);                     // Basic-only candidate occupies the first slot.
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(3)); // Another basic-only card has no slot.
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(0));  // Overlap can occupy the Shrine slot.
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(2));  // Shrine-only candidate is also valid.
    state->toggleResolutionHandPickCard(2);

    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submitted = host.sentCommands.first().submit_resolution_choice();
    ASSERT_EQ(submitted.chosen_object_ids_size(), 2);
    EXPECT_EQ(submitted.chosen_object_ids(0), 102u);
    EXPECT_EQ(submitted.chosen_object_ids(1), 103u);
}

TEST_F(RuledClientTest, MalformedHeterogeneousLibrarySearchIsRejected)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
    rcr->set_max(1);
    rcr->add_candidate_object_ids(101u);
    rcr->add_candidate_server_card_ids(0);
    rcr->add_candidate_names("Forest");
    auto *slot = rcr->add_selection_slots();
    slot->set_label("a basic land card");
    slot->add_candidate_indices(1);
    apply(batch);

    EXPECT_FALSE(state->isResolutionHandPickActive());
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

TEST_F(RuledClientTest, EmptyLibrarySearchOpensABlankDeckViewAndSubmitsFailToFind)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
    rcr->set_prompt_text("Search your library for a Plains card.");
    rcr->set_min(0);
    rcr->set_max(1);
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    EXPECT_TRUE(state->resolutionHandPickCandidateNames().isEmpty());
    ASSERT_EQ(started.count(), 1);
    EXPECT_TRUE(started.at(0).at(0).toStringList().isEmpty());
    EXPECT_TRUE(started.at(0).at(1).value<QVector<int>>().isEmpty());

    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands.first().submit_resolution_choice().chosen_object_ids_size(), 0);
    EXPECT_FALSE(state->isResolutionHandPickActive());
}

TEST_F(RuledClientTest, ManifestDreadChoiceUsesAControlFreeCardImageView)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_MANIFEST_DREAD);
    rcr->set_prompt_text("Choose one of the top two cards to manifest.");
    rcr->set_min(1);
    rcr->set_max(1);
    for (const quint32 oid : {35u, 36u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    rcr->add_candidate_names("Hill Giant");
    rcr->add_candidate_names("Forest");
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Manifest dread"));
    EXPECT_FALSE(state->resolutionHandPickShowViewControls());
    ASSERT_EQ(started.count(), 1);
    EXPECT_EQ(started.at(0).at(0).toStringList(),
              QStringList({QStringLiteral("Hill Giant"), QStringLiteral("Forest")}));
}

TEST_F(RuledClientTest, LibraryLookChoiceShowsEveryCardImageButOnlyMatchingCardsAreClickable)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    rcr->set_prompt_text("Look at the top five cards. Choose a creature card.");
    rcr->set_min(0);
    rcr->set_max(1);
    for (const quint32 oid : {41u, 42u, 43u, 44u, 45u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1, 2, 3, 4}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    for (const char *name : {"Forest", "Grizzly Bears", "Island", "Hill Giant", "Mountain"}) {
        rcr->add_candidate_names(name);
    }
    for (const bool selectable : {false, true, false, true, false}) {
        rcr->add_candidate_selectable(selectable);
    }
    apply(batch);

    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Deck);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Look at cards"));
    EXPECT_FALSE(state->resolutionHandPickShowViewControls());
    ASSERT_EQ(started.count(), 1);
    EXPECT_EQ(started.at(0).at(0).toStringList(),
              QStringList({QStringLiteral("Forest"), QStringLiteral("Grizzly Bears"), QStringLiteral("Island"),
                           QStringLiteral("Hill Giant"), QStringLiteral("Mountain")}));
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(0));
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(1));
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(2));
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(3));

    state->toggleResolutionHandPickCard(0);
    EXPECT_EQ(state->resolutionHandPickSelected(), 0);
    state->toggleResolutionHandPickCard(3);
    EXPECT_EQ(state->resolutionHandPickSelected(), 1);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(3), 1);
}

TEST_F(RuledClientTest, LibraryLookOrderingUsesImageClickOrderAndMalformedEligibilityFailsClosed)
{
    QSignalSpy started(state, &RuledClientState::librarySearchPickStarted);
    ruled::v1::RuledEventBatch malformed;
    auto *bad = malformed.add_events()->mutable_resolution_choice_required();
    bad->set_deciding_player_id(kLocalPlayer);
    bad->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    for (const quint32 oid : {51u, 52u}) {
        bad->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1}) {
        bad->add_candidate_server_card_ids(scid);
    }
    for (const char *name : {"Forest", "Island"}) {
        bad->add_candidate_names(name);
    }
    bad->add_candidate_selectable(true);
    apply(malformed);
    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(started.count(), 0);
    EXPECT_EQ(host.dialogRequests, 0);

    ruled::v1::RuledEventBatch ordered;
    auto *rcr = ordered.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    rcr->set_min(2);
    rcr->set_max(2);
    rcr->set_ordered(true);
    for (const quint32 oid : {61u, 62u}) {
        rcr->add_candidate_object_ids(oid);
    }
    for (const int scid : {0, 1}) {
        rcr->add_candidate_server_card_ids(scid);
    }
    for (const char *name : {"Forest", "Island"}) {
        rcr->add_candidate_names(name);
        rcr->add_candidate_selectable(true);
    }
    apply(ordered);
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Look at cards"));
    EXPECT_EQ(started.count(), 1);

    state->toggleResolutionHandPickCard(1);
    state->toggleResolutionHandPickCard(0);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(1), 1);
    EXPECT_EQ(state->resolutionHandPickClickOrderFor(0), 2);
    host.sentCommands.clear();
    state->submitResolutionHandPick();
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &submission = host.sentCommands[0].submit_resolution_choice();
    ASSERT_EQ(submission.chosen_object_ids_size(), 2);
    EXPECT_EQ(submission.chosen_object_ids(0), 62u);
    EXPECT_EQ(submission.chosen_object_ids(1), 61u);
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
    rcr->add_candidate_selectable(true);
    rcr->add_candidate_selectable(false);
    apply(batch);

    ASSERT_EQ(revealed.count(), 1);
    EXPECT_TRUE(revealed.at(0).at(0).toBool());
    EXPECT_EQ(state->resolutionHandPickZone(), RuledClientState::PickZone::Revealed);
    EXPECT_EQ(state->resolutionHandPickViewTitle(), QStringLiteral("Target player's hand"));
    EXPECT_EQ(revealed.at(0).at(1).toStringList(),
              QStringList({QStringLiteral("Black Lotus"), QStringLiteral("Swamp")}));

    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(0));
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(1));
    state->toggleResolutionHandPickCard(1);
    EXPECT_EQ(state->resolutionHandPickSelected(), 0);
    state->toggleResolutionHandPickCard(0);
    EXPECT_EQ(state->resolutionHandPickSelected(), 1);
}

TEST_F(RuledClientTest, PublicHandRevealIsReadOnlyForObserversAndReconcilesExactSnapshots)
{
    QSignalSpy publicReveal(state, &RuledClientState::publicRevealChanged);
    QSignalSpy timeline(state, &RuledClientState::engineTimeline);
    auto makeReveal = [](quint32 sourceOid, int ownerId, std::initializer_list<const char *> names) {
        ruled::v1::RuledEventBatch batch;
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer + 1);
        rcr->set_source_object_id(sourceOid);
        rcr->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
        rcr->set_reveal_audience(ruled::v1::RESOLUTION_REVEAL_AUDIENCE_ALL_PARTICIPANTS);
        rcr->set_revealed_zone_owner_player_id(ownerId);
        int index = 0;
        for (const char *name : names) {
            rcr->add_candidate_object_ids(100 + index);
            rcr->add_candidate_card_ids(name);
            rcr->add_candidate_names(name);
            rcr->add_candidate_server_card_ids(index);
            ++index;
        }
        return batch;
    };

    apply(makeReveal(700, 2, {"Forest", "Grizzly Bears"}));
    ASSERT_TRUE(state->hasPublicReveal());
    EXPECT_EQ(state->publicRevealSourceObjectId(), 700u);
    EXPECT_EQ(state->publicRevealOwnerPlayerId(), 2);
    EXPECT_EQ(state->publicRevealCandidateNames(),
              QStringList({QStringLiteral("Forest"), QStringLiteral("Grizzly Bears")}));
    EXPECT_FALSE(state->isResolutionHandPickActive());
    ASSERT_EQ(publicReveal.count(), 1);
    EXPECT_TRUE(publicReveal.at(0).at(0).toBool());
    ASSERT_EQ(timeline.count(), 1);
    EXPECT_EQ(timeline.at(0).at(0).toString(), QStringLiteral("P2 reveals: Forest, Grizzly Bears.\n"));

    // An unchanged authoritative repeat (for example after a rejected stale submission) refreshes
    // the popup but must not spam the game log.
    apply(makeReveal(700, 2, {"Forest", "Grizzly Bears"}));
    ASSERT_EQ(timeline.count(), 2);
    EXPECT_TRUE(timeline.at(1).at(0).toString().isEmpty());

    // Same key is a replacement snapshot, not an append.
    apply(makeReveal(700, 2, {"Island"}));
    EXPECT_EQ(state->publicRevealCandidateNames(), QStringList({QStringLiteral("Island")}));
    ASSERT_EQ(publicReveal.count(), 3);
    ASSERT_EQ(timeline.count(), 3);
    EXPECT_EQ(timeline.at(2).at(0).toString(), QStringLiteral("P2 reveals: Island.\n"));

    // A different key replaces the sole reveal rather than creating concurrent state.
    apply(makeReveal(701, 1, {"Swamp", "Thoughtseize"}));
    EXPECT_EQ(state->publicRevealSourceObjectId(), 701u);
    EXPECT_EQ(state->publicRevealOwnerPlayerId(), 1);
    EXPECT_EQ(state->publicRevealCandidateNames(),
              QStringList({QStringLiteral("Swamp"), QStringLiteral("Thoughtseize")}));
    EXPECT_FALSE(state->isResolutionHandPickActive());
    ASSERT_EQ(timeline.count(), 4);
    EXPECT_EQ(timeline.at(3).at(0).toString(), QStringLiteral("P1 reveals: Swamp, Thoughtseize.\n"));

    ruled::v1::RuledEventBatch completed;
    completed.add_events()->mutable_log()->set_text("Choice completed.");
    apply(completed);
    EXPECT_FALSE(state->hasPublicReveal());
    ASSERT_EQ(publicReveal.count(), 5);
    EXPECT_FALSE(publicReveal.at(4).at(0).toBool());
}

TEST_F(RuledClientTest, WardAnnotationAndPaymentUseTheExistingRuledPromptModes)
{
    ruled::v1::RuledEventBatch manaWard;
    auto *pushed = manaWard.add_events()->mutable_stack_pushed();
    pushed->set_object_id(700u);
    pushed->set_description("Dirgur Island Dragon");
    pushed->set_ability_annotation("Ward {2}");
    pushed->set_is_triggered(true);
    auto *payment = manaWard.add_events()->mutable_resolution_choice_required();
    payment->set_deciding_player_id(kLocalPlayer);
    payment->set_choice_kind(ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    payment->set_prompt_text("Ward {2}: Pay {2} or decline.");
    payment->set_generic_mana_cost(2);
    payment->set_payment_currently_legal(false);
    apply(manaWard);

    EXPECT_EQ(state->stackAnnotation(700u), QStringLiteral("Ward {2}"));
    EXPECT_TRUE(state->isResolutionPaymentActive());
    EXPECT_EQ(state->resolutionPaymentGenericCost(), 2);
    EXPECT_FALSE(state->resolutionPaymentCurrentlyLegal());

    ruled::v1::RuledEventBatch discardWardObserver;
    auto *discard = discardWardObserver.add_events()->mutable_resolution_choice_required();
    discard->set_deciding_player_id(kOpponent);
    discard->set_choice_kind(ruled::v1::CHOICE_KIND_HAND_CARDS);
    discard->set_prompt_text("Opponent is making a resolution choice.");
    apply(discardWardObserver);
    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_TRUE(state->isWaitingForResolutionChoice());
    EXPECT_EQ(state->resolutionChoiceWaitingPlayer(), kOpponent);
}

TEST_F(RuledClientTest, CounterEventRemovesOnlyTheExactSyntheticStackObject)
{
    ruled::v1::RuledEventBatch push;
    auto *first = push.add_events()->mutable_stack_pushed();
    first->set_object_id(900);
    first->set_description("Ward target ability");
    first->set_ability_annotation("Target ability");
    auto *second = push.add_events()->mutable_stack_pushed();
    second->set_object_id(901);
    second->set_description("Unrelated ability");
    second->set_ability_annotation("Unrelated ability");
    apply(push);
    ASSERT_EQ(state->getStackOidOrder().size(), 2);

    ruled::v1::RuledEventBatch counter;
    counter.add_events()->mutable_stack_object_countered()->set_object_id(900);
    apply(counter);

    EXPECT_EQ(state->getStackOidOrder(), QVector<quint32>({901}));
    EXPECT_EQ(host.removedSyntheticCards, QVector<quint32>({900}));
    EXPECT_EQ(state->stackAnnotation(901), QStringLiteral("Unrelated ability"));
}

TEST_F(RuledClientTest, PrivateOpponentHandLookDoesNotEnterThePublicGameLog)
{
    QSignalSpy timeline(state, &RuledClientState::engineTimeline);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    rcr->add_candidate_object_ids(21);
    rcr->add_candidate_server_card_ids(0);
    rcr->add_candidate_names("Black Lotus");
    rcr->add_candidate_selectable(true);
    apply(batch);

    ASSERT_EQ(timeline.count(), 1);
    EXPECT_TRUE(timeline.at(0).at(0).toString().isEmpty());
}

TEST_F(RuledClientTest, PublicHandRevealIsInteractiveOnlyForDeciderAndIgnoresPreviewBatches)
{
    QSignalSpy publicReveal(state, &RuledClientState::publicRevealChanged);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_source_object_id(800);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    rcr->set_reveal_audience(ruled::v1::RESOLUTION_REVEAL_AUDIENCE_ALL_PARTICIPANTS);
    rcr->set_revealed_zone_owner_player_id(kLocalPlayer + 1);
    for (int i = 0; i < 2; ++i) {
        rcr->add_candidate_object_ids(200 + i);
        rcr->add_candidate_card_ids(i == 0 ? "forest" : "grizzly_bears");
        rcr->add_candidate_names(i == 0 ? "Forest" : "Grizzly Bears");
        rcr->add_candidate_server_card_ids(i);
    }
    rcr->add_candidate_selectable(false);
    rcr->add_candidate_selectable(true);
    rcr->set_min(1);
    rcr->set_max(1);
    apply(batch);

    ASSERT_TRUE(state->hasPublicReveal());
    ASSERT_TRUE(state->isResolutionHandPickActive());
    EXPECT_FALSE(state->isResolutionHandPickCardSelectable(0));
    EXPECT_TRUE(state->isResolutionHandPickCardSelectable(1));

    ruled::v1::RuledEventBatch preview;
    preview.add_events()->mutable_attackers_preview()->set_declaring_player_id(kLocalPlayer);
    apply(preview);
    EXPECT_TRUE(state->hasPublicReveal());
    EXPECT_EQ(publicReveal.count(), 1);
}

TEST(RuledPickSurfaceTest, PublicRevealAcceptsNonlocalHandViewButOtherPicksRemainLocal)
{
    EXPECT_TRUE(isRuledPickSurfaceCard(RuledPickZone::Revealed, RuledPickScaffoldZone::Hand, true, false));
    EXPECT_TRUE(isRuledPickSurfaceCard(RuledPickZone::Revealed, RuledPickScaffoldZone::Deck, true, false));
    EXPECT_FALSE(isRuledPickSurfaceCard(RuledPickZone::Revealed, RuledPickScaffoldZone::Hand, false, false));

    EXPECT_FALSE(isRuledPickSurfaceCard(RuledPickZone::Hand, RuledPickScaffoldZone::Hand, false, false));
    EXPECT_TRUE(isRuledPickSurfaceCard(RuledPickZone::Hand, RuledPickScaffoldZone::Hand, false, true));
    EXPECT_FALSE(isRuledPickSurfaceCard(RuledPickZone::Deck, RuledPickScaffoldZone::Deck, true, false));
    EXPECT_TRUE(isRuledPickSurfaceCard(RuledPickZone::Deck, RuledPickScaffoldZone::Deck, true, true));
}

TEST_F(RuledClientTest, MalformedPublicHandRevealFailsClosed)
{
    QSignalSpy publicReveal(state, &RuledClientState::publicRevealChanged);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer + 1);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_OPPONENT_HAND);
    rcr->set_reveal_audience(ruled::v1::RESOLUTION_REVEAL_AUDIENCE_ALL_PARTICIPANTS);
    rcr->set_revealed_zone_owner_player_id(2);
    rcr->add_candidate_object_ids(100);
    rcr->add_candidate_card_ids("forest");
    rcr->add_candidate_names("Forest");
    // Missing the parallel transient popup id.
    apply(batch);

    EXPECT_FALSE(state->hasPublicReveal());
    EXPECT_FALSE(state->isResolutionHandPickActive());
    EXPECT_EQ(publicReveal.count(), 0);
    EXPECT_EQ(host.dialogRequests, 0);
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

TEST_F(RuledClientTest, CopySourceChoiceUsesBoardSelectionAndEmptyResolutionChoiceDeclines)
{
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_COPY_SOURCE);
    rcr->set_min(0);
    rcr->set_max(1);
    rcr->set_prompt_text("Choose a creature for Clone to copy, or Decline to enter as Clone.");
    rcr->add_candidate_object_ids(100);
    rcr->add_candidate_object_ids(101);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));
    EXPECT_TRUE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopySource, 100));
    EXPECT_FALSE(state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopySource, 102));
    EXPECT_TRUE(state->pendingClickChoiceMayDecline());
    EXPECT_EQ(state->pendingChoicePromptText(RuledClientState::ChoiceKind::CopySource),
              QStringLiteral("Choose a creature for Clone to copy, or Decline to enter as Clone."));

    host.sentCommands.clear();
    state->declinePendingClickChoice();
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_TRUE(host.sentCommands[0].has_submit_resolution_choice());
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids_size(), 0);
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));
}

TEST_F(RuledClientTest, PopulateUsesMandatoryCopySourceSelection)
{
    ruled::v1::RuledEventBatch batch;
    auto *choice = batch.add_events()->mutable_resolution_choice_required();
    choice->set_deciding_player_id(kLocalPlayer);
    choice->set_choice_kind(ruled::v1::CHOICE_KIND_COPY_SOURCE);
    choice->set_min(1);
    choice->set_max(1);
    choice->set_prompt_text("Choose a creature token you control to populate.");
    choice->add_candidate_object_ids(501);
    choice->add_candidate_object_ids(502);
    apply(batch);
    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));
    EXPECT_FALSE(state->pendingClickChoiceMayDecline());
    host.sentCommands.clear();
    state->declinePendingClickChoice();
    EXPECT_TRUE(host.sentCommands.isEmpty());
    EXPECT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));
    state->submitPendingChoiceObject(502);
    ASSERT_EQ(host.sentCommands.size(), 1);
    ASSERT_TRUE(host.sentCommands[0].has_submit_resolution_choice());
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 502u);
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));

    choice->set_deciding_player_id(kLocalPlayer + 1);
    apply(batch);
    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));
}

TEST_F(RuledClientTest, ResolutionCostObjectsToggleAsABoundedCohortBeforeSubmission)
{
    QSignalSpy progressSpy(state, &RuledClientState::resolutionCostSelectionChanged);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_COST_OBJECTS);
    rcr->set_min(2);
    rcr->set_max(2);
    rcr->set_prompt_text("Choose two untapped permanents.");
    rcr->add_candidate_object_ids(100);
    rcr->add_candidate_object_ids(101);
    apply(batch);

    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CostObjects));
    EXPECT_EQ(progressSpy.count(), 1);
    progressSpy.clear();
    state->toggleResolutionCostObject(999);
    EXPECT_EQ(progressSpy.count(), 0);
    state->toggleResolutionCostObject(100);
    EXPECT_EQ(progressSpy.count(), 1);
    EXPECT_TRUE(state->isResolutionCostObjectSelected(100));
    EXPECT_EQ(state->resolutionCostObjectSelectedCount(), 1);
    host.sentCommands.clear();
    state->submitResolutionCostObjects();
    EXPECT_TRUE(host.sentCommands.isEmpty());

    state->toggleResolutionCostObject(101);
    QSignalSpy repaintSpy(state, &RuledClientState::combatStateChanged);
    state->submitResolutionCostObjects();
    EXPECT_EQ(repaintSpy.count(), 1);
    EXPECT_FALSE(state->isResolutionCostObjectSelected(100));
    ASSERT_EQ(host.sentCommands.size(), 1);
    const auto &chosen = host.sentCommands[0].submit_resolution_choice().chosen_object_ids();
    ASSERT_EQ(chosen.size(), 2);
    EXPECT_EQ(chosen.Get(0), 100u);
    EXPECT_EQ(chosen.Get(1), 101u);
    EXPECT_EQ(progressSpy.count(), 3); // two selections and clearing the submitted choice
}

TEST_F(RuledClientTest, CancellingResolutionCostObjectsRepaintsClearedSelectionImmediately)
{
    RuledClientState::RuledPendingChoice choice;
    choice.kind = RuledClientState::ChoiceKind::CostObjects;
    choice.mayDecline = true;
    choice.min = 0;
    choice.max = 1;
    choice.candidateOids = {100};
    state->setPendingChoice(choice);
    state->toggleResolutionCostObject(100);
    ASSERT_TRUE(state->isResolutionCostObjectSelected(100));

    QSignalSpy repaintSpy(state, &RuledClientState::combatStateChanged);
    QObject::connect(state, &RuledClientState::combatStateChanged, state,
                     [this]() { EXPECT_FALSE(state->isResolutionCostObjectSelected(100)); });
    state->declinePendingClickChoice();

    EXPECT_EQ(repaintSpy.count(), 1);
    ASSERT_EQ(host.sentCommands.size(), 1);
    EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids_size(), 0);
}

TEST_F(RuledClientTest, AuraReturnChoicesUseTypedPermanentAndPlayerClickSurfaces)
{
    for (const auto &[protoKind, stateKind] :
         std::initializer_list<std::pair<ruled::v1::ChoiceKind, RuledClientState::ChoiceKind>>{
             {ruled::v1::CHOICE_KIND_AURA_PERMANENT, RuledClientState::ChoiceKind::AuraPermanent},
             {ruled::v1::CHOICE_KIND_AURA_PLAYER, RuledClientState::ChoiceKind::AuraPlayer}}) {
        ruled::v1::RuledEventBatch batch;
        auto *rcr = batch.add_events()->mutable_resolution_choice_required();
        rcr->set_deciding_player_id(kLocalPlayer);
        rcr->set_choice_kind(protoKind);
        rcr->set_min(1);
        rcr->set_max(1);
        rcr->set_prompt_text("Choose what the Aura will enchant.");
        rcr->add_candidate_object_ids(100);
        apply(batch);

        ASSERT_TRUE(state->hasPendingChoiceOfKind(stateKind));
        EXPECT_TRUE(state->isPendingChoiceCandidate(stateKind, 100));
        EXPECT_FALSE(state->pendingClickChoiceMayDecline());

        host.sentCommands.clear();
        state->submitPendingChoiceObject(100);
        ASSERT_EQ(host.sentCommands.size(), 1);
        EXPECT_EQ(host.sentCommands[0].submit_resolution_choice().chosen_object_ids(0), 100u);
    }
}

TEST_F(RuledClientTest, CopySourceChoiceIsInteractiveOnlyForTheDecidingPlayer)
{
    QSignalSpy promptFeed(state, &RuledClientState::enginePromptFeed);
    ruled::v1::RuledEventBatch batch;
    auto *rcr = batch.add_events()->mutable_resolution_choice_required();
    rcr->set_deciding_player_id(kLocalPlayer + 1);
    rcr->set_choice_kind(ruled::v1::CHOICE_KIND_COPY_SOURCE);
    rcr->set_min(0);
    rcr->set_max(1);
    rcr->set_prompt_text("Choose a creature for Clone to copy, or Decline to enter as Clone.");
    rcr->add_candidate_object_ids(100);
    apply(batch);

    EXPECT_FALSE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource));
    ASSERT_EQ(promptFeed.count(), 1);
    EXPECT_TRUE(promptFeed.at(0).at(0).toString().contains(QStringLiteral("Choose a creature for Clone to copy")));
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
    state->payment.begin();
    const auto obsoletePayment = state->payment.request({});
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
    EXPECT_FALSE(state->payment.active); // a blocking engine choice replaces local staging
    ruled::v1::PaymentPreview obsoleteReply;
    obsoleteReply.set_transaction_id(obsoletePayment.transaction_id());
    obsoleteReply.set_revision(obsoletePayment.revision());
    EXPECT_FALSE(state->payment.apply(obsoleteReply));
    ASSERT_TRUE(state->hasStackItems());
    ASSERT_TRUE(state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));

    QSignalSpy resetSpy(state, &RuledClientState::sessionReset);
    state->payment.begin();
    state->payment.payMana('U');
    state->clearSessionState();

    EXPECT_EQ(resetSpy.count(), 1);
    EXPECT_FALSE(state->payment.active);
    EXPECT_EQ(state->payment.selection.mana().u(), 0u);
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
