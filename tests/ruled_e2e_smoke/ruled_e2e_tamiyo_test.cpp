#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
// Prepared actual-card fixture through a real relay/sidecar and two independent clients.
// Strict dev-disabled legal-session reachability is covered by the server capture pilots.
TEST_F(RuledE2ESmokeTest, TamiyoEmblemHandCastKeepsPrivateMethodAndExactDuplicatePhysicalCard)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << std::string(started.message()).substr(5);
    OpeningDriver p1(true, QStringLiteral("tamiyop1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("tamiyop2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Tamiyo start P1"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Tamiyo start P2"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());
    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId)
            break;
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    auto send = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const auto before1 = p1.stateVersion;
        const auto before2 = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto put = [&](const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        dev->mutable_put_card_in_zone()->set_zone(zone);
        dev->mutable_put_card_in_zone()->set_ready(true);
        return send(p1, command, QStringLiteral("Tamiyo prepared put %1").arg(name));
    };
    auto pass = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("Tamiyo pass"));
    };
    ASSERT_TRUE(put("Doubling Season", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put("Tamiyo, Field Researcher", ruled::v1::DEV_ZONE_BATTLEFIELD));
    const auto &board = p1.battlefieldByPlayer[p1.myId];
    const auto source = std::find_if(board.begin(), board.end(), [](const auto &p) {
        return p.cardId == QStringLiteral("tamiyo,_field_researcher");
    });
    ASSERT_NE(source, board.end());
    ASSERT_EQ(source->loyalty, 8);
    ruled::v1::RuledCommand ultimate;
    p1.setBattlefieldAbilitySource(ultimate.mutable_activate_ability(), source->oid);
    ultimate.mutable_activate_ability()->set_ability_index(2);
    ASSERT_TRUE(send(p1, ultimate, QStringLiteral("Tamiyo prepared -7")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    for (const auto *client : {&p1, &p2}) {
        EXPECT_TRUE(std::any_of(client->physicalCreateTokenEvents.begin(), client->physicalCreateTokenEvents.end(),
                               [](const Event_CreateToken &token) {
                                   return token.card_name() == "Tamiyo, Field Researcher Emblem" &&
                                          token.annotation() == "Emblem";
                               }));
    }
    ASSERT_TRUE(put("Divination", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(put("Divination", ruled::v1::DEV_ZONE_HAND));
    std::vector<int> handSlots;
    for (const auto &action : p1.latestLegal.hand_actions()) {
        if (action.card_name() == "Divination" && action.cast_method() == ruled::v1::CAST_METHOD_WITHOUT_MANA_COST)
            handSlots.push_back(action.hand_index());
    }
    ASSERT_EQ(handSlots.size(), 2u);
    const int slot = handSlots.back();
    const int exactPhysical = p1.handServerCardBySlot.at(slot);
    const int otherPhysical = p1.handServerCardBySlot.at(handSlots.front());
    EXPECT_NE(exactPhysical, otherPhysical);
    EXPECT_EQ(p1.myPool.total(), 0);
    EXPECT_FALSE(p2.latestLegal.has_pending_spell_cast());
    EXPECT_TRUE(std::none_of(p2.latestLegal.hand_actions().begin(), p2.latestLegal.hand_actions().end(), [](const auto &a) {
        return a.cast_method() == ruled::v1::CAST_METHOD_WITHOUT_MANA_COST;
    }));
    const auto handBefore = p1.handServerCardBySlot;
    const int physicalHandBefore = p1.physicalHandCountByPlayer[p1.myId];
    auto begin = [&] {
        ruled::v1::RuledCommand command;
        auto *announcement = command.mutable_begin_spell_cast()->mutable_announcement();
        announcement->mutable_source()->set_hand_index(slot);
        announcement->set_cast_method(ruled::v1::CAST_METHOD_WITHOUT_MANA_COST);
        p1.sendRuled(command, QStringLiteral("Tamiyo begin zero-base hand cast"));
        return p1.pumpUntil([&] {
            p2.pump(25);
            return p1.latestLegal.has_pending_spell_cast();
        }, 10000, "Tamiyo private pending announcement");
    };
    ASSERT_TRUE(begin());
    ASSERT_TRUE(p1.latestLegal.has_pending_spell_cast());
    EXPECT_EQ(p1.latestLegal.pending_spell_cast().announcement().cast_method(), ruled::v1::CAST_METHOD_WITHOUT_MANA_COST);
    // The engine reserves its hand slot during payment; the physical card stays in hand.
    EXPECT_EQ(p1.physicalHandCountByPlayer[p1.myId], physicalHandBefore);
    for (const auto *client : {&p1, &p2}) {
        EXPECT_FALSE(std::any_of(client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(), [&](const auto &move) {
            return move.card_id() == exactPhysical && move.start_zone() == ZoneNames::HAND;
        }));
    }
    ruled::v1::RuledCommand cancel;
    cancel.mutable_cancel_spell_cast()->set_transaction_id(p1.latestLegal.pending_spell_cast().transaction_id());
    p1.sendRuled(cancel, QStringLiteral("Tamiyo cancel first announcement"));
    ASSERT_TRUE(p1.pumpUntil([&] {
        p2.pump(25);
        return !p1.latestLegal.has_pending_spell_cast();
    }, 10000, "Tamiyo private cancellation"));
    EXPECT_EQ(p1.handServerCardBySlot, handBefore);
    ASSERT_TRUE(begin());
    const auto pending = p1.latestLegal.pending_spell_cast();
    EXPECT_FALSE(p2.latestLegal.has_pending_spell_cast());
    const int beforePreview = p1.paymentPreviewCount;
    ruled::v1::RuledCommand preview;
    auto *query = preview.mutable_preview_payment();
    query->set_transaction_id(pending.transaction_id());
    query->set_revision(p1.stateVersion);
    query->mutable_commit_spell_cast()->set_transaction_id(pending.transaction_id());
    p1.sendRuled(preview, QStringLiteral("Tamiyo preview exact zero payment"));
    ASSERT_TRUE(p1.pumpUntil([&] { p2.pump(25); return p1.paymentPreviewCount > beforePreview; }, 10000, "Tamiyo preview"));
    ASSERT_TRUE(p1.paymentPreview.valid()) << p1.paymentPreview.error();
    ASSERT_TRUE(p1.paymentPreview.complete());
    ruled::v1::RuledCommand commit;
    commit.mutable_commit_spell_cast()->set_transaction_id(pending.transaction_id());
    *commit.mutable_commit_spell_cast()->mutable_payment() = p1.paymentPreview.selection();
    ASSERT_TRUE(send(p1, commit, QStringLiteral("Tamiyo commit without mana")));
    EXPECT_EQ(p1.myPool.total(), 0);
    EXPECT_EQ(p1.stackDepth, 1);
    EXPECT_EQ(p2.stackDepth, 1);
    for (const auto *client : {&p1, &p2}) {
        ASSERT_NE(client->serverCardByEngineOid.find(pending.reserved_object_id()), client->serverCardByEngineOid.end());
        EXPECT_EQ(client->serverCardByEngineOid.at(pending.reserved_object_id()), exactPhysical);
        EXPECT_TRUE(std::any_of(client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(), [&](const auto &move) {
            return move.card_id() == exactPhysical && move.start_zone() == ZoneNames::HAND &&
                   move.target_zone() == ZoneNames::STACK;
        }));
        EXPECT_FALSE(std::any_of(client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(), [&](const auto &move) {
            return move.card_id() == otherPhysical && move.start_zone() == ZoneNames::HAND;
        }));
    }
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    EXPECT_EQ(p1.stackDepth, 0);
    EXPECT_EQ(p2.stackDepth, 0);
}
} // namespace
} // namespace ruled_e2e
