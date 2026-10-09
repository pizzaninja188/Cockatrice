#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
class ArenaDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    std::vector<ruled::v1::StackPushed> activations;
    bool legalActionsStayedPrivate = true;

    void onRuledEvent(const ruled::v1::RuledEvent &event) override
    {
        OpeningDriver::onRuledEvent(event);
        if (event.has_stack_pushed() && event.stack_pushed().card_id().empty() &&
            !event.stack_pushed().is_triggered()) {
            activations.push_back(event.stack_pushed());
        }
    }
    void onLegalActions(const ruled::v1::RuledEventBatch &batch) override
    {
        for (const auto &entry : batch.legal_by_player()) {
            legalActionsStayedPrivate &= entry.first == myId;
        }
    }
};

std::optional<ObservedState::Permanent> permanent(const ArenaDriver &client, int player, quint32 oid)
{
    const auto found = client.battlefieldByPlayer.find(player);
    if (found == client.battlefieldByPlayer.end()) {
        return std::nullopt;
    }
    for (const auto &card : found->second) {
        if (card.oid == oid) {
            return card;
        }
    }
    return std::nullopt;
}
} // namespace

TEST_F(RuledE2ESmokeTest, ArenaOpponentChoosesBeforePaymentAndDuplicateCreaturesKeepPhysicalIdentity)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    ArenaDriver p1(true, QStringLiteral("arenap1"), &transcript);
    ArenaDriver p2(false, QStringLiteral("arenap2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Arena game start"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Arena game start"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());
    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000 && p1.phase != ruled::v1::PHASE_ID_MAIN1) {
        p1.pump(25);
        p2.pump(25);
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    auto send = [&](ArenaDriver &sender, const ruled::v1::RuledCommand &command, const QString &label) {
        const auto v1 = p1.stateVersion;
        const auto v2 = p2.stateVersion;
        sender.sendRuled(command, label);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= v1 || p2.stateVersion <= v2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > v1 && p2.stateVersion > v2;
    };
    auto reject = [&](ArenaDriver &sender, const ruled::v1::RuledCommand &command, const QString &label) {
        const auto v1 = p1.stateVersion;
        const auto v2 = p2.stateVersion;
        const auto legal1 = p1.latestLegal.SerializeAsString();
        const auto legal2 = p2.latestLegal.SerializeAsString();
        const auto id = sender.nextCmdId;
        sender.sendRuled(command, label);
        QElapsedTimer wait;
        wait.start();
        while (!sender.responses.count(id) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        if (!sender.responses.count(id)) {
            return false;
        }
        EXPECT_NE(sender.responses.at(id).response_code(), Response::RespOk);
        EXPECT_EQ(p1.stateVersion, v1);
        EXPECT_EQ(p2.stateVersion, v2);
        EXPECT_EQ(p1.latestLegal.SerializeAsString(), legal1);
        EXPECT_EQ(p2.latestLegal.SerializeAsString(), legal2);
        return sender.responses.at(id).response_code() != Response::RespOk;
    };
    auto put = [&](int player, const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        command.mutable_dev_command()->set_target_player_id(player);
        auto *placement = command.mutable_dev_command()->mutable_put_card_in_zone();
        placement->set_card_name(name);
        placement->set_zone(zone);
        placement->set_ready(true);
        return send(p1, command, QStringLiteral("Arena put %1").arg(QString::fromLatin1(name)));
    };
    ASSERT_TRUE(put(p1.myId, "Arena", ruled::v1::DEV_ZONE_HAND));
    const auto land = p1.handAction(ruled::v1::HAND_ACTION_PLAY_LAND, QStringLiteral("Arena"));
    ASSERT_NE(land, nullptr);
    ruled::v1::RuledCommand play;
    play.mutable_play_land()->mutable_source()->set_hand_index(land->hand_index());
    ASSERT_TRUE(send(p1, play, QStringLiteral("Play Arena")));
    ASSERT_TRUE(put(p1.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put(p2.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put(p2.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD));
    for (int i = 0; i < 3; ++i) {
        ASSERT_TRUE(put(p1.myId, "Forest", ruled::v1::DEV_ZONE_BATTLEFIELD));
    }
    ObservedState::Permanent arena;
    ObservedState::Permanent own;
    std::vector<ObservedState::Permanent> forests;
    for (const auto &card : p1.battlefieldByPlayer.at(p1.myId)) {
        if (card.cardId == QLatin1String("arena")) {
            arena = card;
        } else if (card.cardId == QLatin1String("grizzly_bears")) {
            own = card;
        } else if (card.cardId == QLatin1String("forest")) {
            forests.push_back(card);
        }
    }
    ASSERT_NE(arena.oid, 0U);
    ASSERT_NE(own.oid, 0U);
    ASSERT_EQ(forests.size(), 3U);
    const auto opponents = p1.battlefieldByPlayer.at(p2.myId);
    ASSERT_EQ(opponents.size(), 2U);
    const auto chosen = opponents.back();
    const auto untouched = opponents.front();
    ASSERT_NE(chosen.oid, untouched.oid);
    for (const auto &card : {arena, own, chosen, untouched}) {
        ASSERT_TRUE(p1.serverCardByEngineOid.count(card.oid));
        ASSERT_TRUE(p2.serverCardByEngineOid.count(card.oid));
        EXPECT_EQ(p1.serverCardByEngineOid.at(card.oid), p2.serverCardByEngineOid.at(card.oid));
    }
    const int chosenPhysical = p1.serverCardByEngineOid.at(chosen.oid);
    const int untouchedPhysical = p1.serverCardByEngineOid.at(untouched.oid);
    ASSERT_NE(chosenPhysical, untouchedPhysical);
    auto begin = [&] {
        ruled::v1::RuledCommand command;
        auto *activation = command.mutable_begin_ability_activation();
        activation->set_source_object_id(arena.oid);
        activation->set_expected_zone_change_generation(arena.generation);
        activation->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD);
        ASSERT_FALSE(arena.abilityIndices.empty());
        activation->set_ability_index(arena.abilityIndices.front());
        auto *target = activation->mutable_own_target();
        target->set_object_id(own.oid);
        target->set_zone_change_generation(own.generation);
        target->set_group_index(0);
        ASSERT_TRUE(send(p1, command, QStringLiteral("Announce Arena own target")));
    };
    begin();
    ASSERT_TRUE(p1.latestLegal.has_pending_ability_activation());
    ASSERT_TRUE(p2.latestLegal.has_pending_ability_activation());
    auto actorView = p1.latestLegal.pending_ability_activation();
    auto chooserView = p2.latestLegal.pending_ability_activation();
    EXPECT_EQ(actorView.stage(), ruled::v1::ABILITY_ACTIVATION_STAGE_OPPONENT_TARGET);
    EXPECT_EQ(actorView.deciding_player_id(), p2.myId);
    EXPECT_EQ(actorView.announced_targets_size(), 1);
    EXPECT_EQ(actorView.target_candidates_size(), 0);
    EXPECT_FALSE(actorView.has_payment_preview());
    EXPECT_EQ(chooserView.target_candidates_size(), 2);
    EXPECT_FALSE(chooserView.has_payment_preview());
    EXPECT_FALSE(permanent(p1, p1.myId, arena.oid)->tapped);
    EXPECT_EQ(p1.stackDepth, 0);
    EXPECT_EQ(p1.myPool.total(), 0);
    ruled::v1::RuledCommand reply;
    auto *answer = reply.mutable_submit_ability_activation_choice();
    answer->set_transaction_id(chooserView.transaction_id());
    answer->set_expected_revision(chooserView.revision());
    const auto offered = std::find_if(chooserView.target_candidates().begin(), chooserView.target_candidates().end(),
                                    [&](const auto &candidate) { return candidate.object_id() == chosen.oid; });
    ASSERT_NE(offered, chooserView.target_candidates().end());
    answer->mutable_target()->CopyFrom(*offered);
    ASSERT_TRUE(reject(p1, reply, QStringLiteral("Reject actor spoof of opponent target")));
    auto stale = reply;
    stale.mutable_submit_ability_activation_choice()->set_expected_revision(chooserView.revision() + 1);
    ASSERT_TRUE(reject(p2, stale, QStringLiteral("Reject stale opponent revision")));
    ASSERT_TRUE(send(p2, reply, QStringLiteral("Opponent selects second physical Bears")));
    ASSERT_EQ(p1.latestLegal.pending_ability_activation().announced_targets_size(), 2);
    ASSERT_TRUE(p1.latestLegal.pending_ability_activation().has_payment_preview());
    EXPECT_FALSE(p2.latestLegal.pending_ability_activation().has_payment_preview());
    EXPECT_TRUE(p2.latestLegal.pending_ability_activation().locked_total_cost().empty());
    EXPECT_EQ(p1.latestLegal.pending_ability_activation().locked_total_cost(), "{3}");
    for (const auto &forest : forests) {
        ruled::v1::RuledCommand mana;
        p1.setBattlefieldAbilitySource(mana.mutable_activate_ability(), forest.oid);
        mana.mutable_activate_ability()->set_ability_index(0);
        ASSERT_TRUE(send(p1, mana, QStringLiteral("Float Forest mana during Arena payment")));
    }
    EXPECT_EQ(p1.myPool.g, 3);
    const auto pending = p1.latestLegal.pending_ability_activation();
    ruled::v1::RuledCommand cancel;
    cancel.mutable_cancel_ability_activation()->set_transaction_id(pending.transaction_id());
    cancel.mutable_cancel_ability_activation()->set_expected_revision(pending.revision());
    ASSERT_TRUE(send(p1, cancel, QStringLiteral("Cancel Arena while retaining completed mana")));
    EXPECT_FALSE(p1.latestLegal.has_pending_ability_activation());
    EXPECT_FALSE(p2.latestLegal.has_pending_ability_activation());
    EXPECT_EQ(p1.myPool.g, 3);
    EXPECT_FALSE(permanent(p2, p1.myId, arena.oid)->tapped);
    begin();
    chooserView = p2.latestLegal.pending_ability_activation();
    answer->set_transaction_id(chooserView.transaction_id());
    answer->set_expected_revision(chooserView.revision());
    ASSERT_TRUE(send(p2, reply, QStringLiteral("Opponent repeats exact target after cancel")));
    actorView = p1.latestLegal.pending_ability_activation();
    ruled::v1::RuledCommand query;
    auto *preview = query.mutable_preview_payment();
    preview->set_transaction_id(89);
    preview->set_revision(1);
    auto *commit = preview->mutable_commit_ability_activation();
    commit->set_transaction_id(actorView.transaction_id());
    commit->set_expected_revision(actorView.revision());
    commit->mutable_payment()->CopyFrom(actorView.payment_preview().selection());
    commit->mutable_payment()->mutable_mana()->set_g(3);
    const auto previewCount = p1.paymentPreviewCount;
    const auto otherPreviewCount = p2.paymentPreviewCount;
    const auto beforePreview = p1.stateVersion;
    p1.sendRuled(query, QStringLiteral("Private Arena payment preview"));
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.paymentPreviewCount > previewCount; }, 10000, "Arena payment preview"));
    p2.pump(100);
    ASSERT_TRUE(p1.paymentPreview.valid()) << p1.paymentPreview.error();
    ASSERT_TRUE(p1.paymentPreview.complete());
    EXPECT_EQ(p1.stateVersion, beforePreview);
    EXPECT_EQ(p2.paymentPreviewCount, otherPreviewCount);
    ruled::v1::RuledCommand paid;
    paid.mutable_commit_ability_activation()->CopyFrom(*commit);
    paid.mutable_commit_ability_activation()->mutable_payment()->CopyFrom(p1.paymentPreview.selection());
    ASSERT_TRUE(send(p1, paid, QStringLiteral("Pay Arena three green mana and tap source")));
    EXPECT_EQ(p1.myPool.total(), 0);
    EXPECT_FALSE(p1.latestLegal.has_pending_ability_activation());
    EXPECT_FALSE(p2.latestLegal.has_pending_ability_activation());
    EXPECT_TRUE(permanent(p1, p1.myId, arena.oid)->tapped);
    EXPECT_TRUE(permanent(p2, p1.myId, arena.oid)->tapped);
    EXPECT_EQ(p1.stackDepth, 1);
    for (ArenaDriver *client : {&p1, &p2}) {
        ASSERT_EQ(client->activations.size(), 1U);
        const auto &activation = client->activations.front();
        ASSERT_EQ(activation.targets_size(), 2);
        EXPECT_EQ(activation.targets(0).object_id(), own.oid);
        EXPECT_EQ(activation.targets(0).group_index(), 0U);
        EXPECT_EQ(activation.targets(1).object_id(), chosen.oid);
        EXPECT_EQ(activation.targets(1).group_index(), 1U);
        EXPECT_TRUE(client->physicallyTappedCardIds.count({p1.myId, client->serverCardByEngineOid.at(arena.oid)}));
        EXPECT_TRUE(client->legalActionsStayedPrivate);
    }
    ruled::v1::RuledCommand pass;
    pass.mutable_pass_priority();
    ASSERT_TRUE(send(p1, pass, QStringLiteral("Arena actor passes")));
    ASSERT_TRUE(send(p2, pass, QStringLiteral("Arena opponent passes; fight resolves")));
    for (ArenaDriver *client : {&p1, &p2}) {
        EXPECT_EQ(client->stackDepth, 0);
        EXPECT_EQ(client->graveyardOwnerByEngineOid.at(own.oid), p1.myId);
        EXPECT_EQ(client->graveyardOwnerByEngineOid.at(chosen.oid), p2.myId);
        ASSERT_TRUE(permanent(*client, p2.myId, untouched.oid));
        EXPECT_FALSE(permanent(*client, p2.myId, untouched.oid)->tapped);
        EXPECT_EQ(client->serverCardByEngineOid.at(untouched.oid), untouchedPhysical);
        const auto moved = std::find_if(client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(),
                                        [&](const auto &move) {
                                            return move.start_player_id() == p2.myId && move.card_id() == chosenPhysical &&
                                                   move.target_zone() == ZoneNames::GRAVE;
                                        });
        EXPECT_NE(moved, client->physicalMoveEvents.end());
    }
}
} // namespace ruled_e2e
