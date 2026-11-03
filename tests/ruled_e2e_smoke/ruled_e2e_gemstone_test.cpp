#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
// Prepared duplicate-only decks guarantee the opening interaction, through a real relay/sidecar
// and two independent clients. Legal sixty-card normal startup is proven by the server pilot.
TEST_F(RuledE2ESmokeTest, GemstoneOpeningBindsExactDuplicateSourceAndExileBeforeFirstPriority)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << std::string(started.message()).substr(5);
    OpeningDriver p1(true, QStringLiteral("cavernsp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("cavernsp2"), &transcript, true);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Gemstone Caverns")}})));
    p1.sendReady();
    p2.sendReady();
    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p2.latestLegal.has_opening() && p2.latestLegal.opening().can_finish_hand_actions() &&
            p1.latestLegal.has_opening() &&
            p1.latestLegal.opening().stage() == ruled::v1::OPENING_STAGE_HAND_ACTIONS)
            break;
        p1.act();
        p2.act();
    }
    ASSERT_TRUE(p2.didMulligan && p2.sentBottom);
    ASSERT_EQ(p2.latestLegal.opening().stage(), ruled::v1::OPENING_STAGE_HAND_ACTIONS);
    ASSERT_EQ(p2.latestLegal.opening().gemstone_caverns_size(), 6);
    EXPECT_EQ(p1.latestLegal.opening().gemstone_caverns_size(), 0);
    EXPECT_EQ(p1.latestLegal.opening().exile_candidates_size(), 0);
    ASSERT_TRUE(p2.latestLegal.permanent_actions().empty());
    const auto source = p2.latestLegal.opening().gemstone_caverns(2);
    const auto exile = p2.latestLegal.opening().exile_candidates(4);
    const int exactSource = p2.handServerCardBySlot.at(source.hand_index());
    const int exactExile = p2.handServerCardBySlot.at(exile.hand_index());
    ASSERT_NE(exactSource, exactExile);
    auto send = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const auto before1 = p1.stateVersion, before2 = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    ruled::v1::RuledCommand begin;
    begin.mutable_begin_with_gemstone_caverns()->mutable_source()->CopyFrom(source.object());
    begin.mutable_begin_with_gemstone_caverns()->mutable_exile()->CopyFrom(exile.object());
    ASSERT_TRUE(send(p2, begin, QStringLiteral("Gemstone exact opening source and exile")));
    for (auto *client : {&p1, &p2}) {
        ASSERT_EQ(client->battlefieldByPlayer[p2.myId].size(), 1u);
        const auto &land = client->battlefieldByPlayer[p2.myId][0];
        EXPECT_EQ(land.cardId, QStringLiteral("gemstone_caverns"));
        EXPECT_TRUE(land.countersAnnotation.contains(QStringLiteral("luck")));
        EXPECT_FALSE(land.firstAbilityActivatable);
        EXPECT_EQ(client->serverCardByEngineOid.at(land.oid), exactSource);
        EXPECT_EQ(client->physicalHandCountByPlayer[p2.myId], 4);
        EXPECT_EQ(client->stackDepth, 0);
        EXPECT_TRUE(
            std::any_of(client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(), [&](const auto &move) {
                return move.card_id() == exactSource && move.start_zone() == ZoneNames::HAND &&
                       move.target_zone() == ZoneNames::TABLE;
            }));
        EXPECT_TRUE(
            std::any_of(client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(), [&](const auto &move) {
                return move.card_id() == exactExile && move.start_zone() == ZoneNames::HAND &&
                       move.target_zone() == ZoneNames::EXILE;
            }));
    }
    EXPECT_EQ(p2.latestLegal.opening().gemstone_caverns_size(), 4);
    EXPECT_EQ(p1.latestLegal.opening().exile_candidates_size(), 0);
    ruled::v1::RuledCommand finish;
    finish.mutable_finish_opening_hand_actions();
    ASSERT_TRUE(send(p2, finish, QStringLiteral("Gemstone finish opening actions")));
    EXPECT_EQ(p1.phase, ruled::v1::PHASE_ID_UPKEEP);
    EXPECT_EQ(p2.phase, ruled::v1::PHASE_ID_UPKEEP);
    EXPECT_EQ(p1.priorityPlayer, p1.myId);
    EXPECT_EQ(p2.priorityPlayer, p1.myId);
    EXPECT_FALSE(p2.latestLegal.has_opening());
}
} // namespace
} // namespace ruled_e2e
