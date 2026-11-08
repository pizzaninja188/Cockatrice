#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
class AutomationDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    bool clearAfterResolution = false;
    int resolved = 0;
    void onRuledEvent(const ruled::v1::RuledEvent &event) override
    {
        OpeningDriver::onRuledEvent(event);
        if (event.has_stack_resolved()) {
            ++resolved;
            if (clearAfterResolution) {
                clearAfterResolution = false;
                ruled::v1::RuledCommand stop;
                stop.mutable_update_automation()->set_clear_all(true);
                sendRuled(stop, "interrupt between stack resolutions");
            }
        }
    }
    void onBatchBegin(const ruled::v1::RuledEventBatch &batch) override
    {
        EXPECT_FALSE(batch.has_automation_continuation()) << "relay continuation is server-only";
        for (const auto &entry : batch.legal_by_player()) {
            EXPECT_EQ(entry.first, myId) << "automation preferences belong only to their seat";
        }
    }
};
} // namespace

TEST_F(RuledE2ESmokeTest, EntireStackAutomationCanBeCancelledBetweenResolutions)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    AutomationDriver p1(true, "autoyield1", &transcript);
    AutomationDriver p2(false, "autoyield2", &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, "Forest"}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, "Island"}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "automation startup"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "automation observer"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());
    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(10);
        p2.pump(10);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    const auto send = [&](AutomationDriver &actor, const ruled::v1::RuledCommand &command) {
        const auto before = actor.stateVersion;
        actor.sendRuled(command, "automation scenario command");
        QElapsedTimer wait;
        wait.start();
        while (actor.stateVersion == before && wait.elapsed() < 10000) {
            p1.pump(5);
            p2.pump(5);
        }
        return actor.stateVersion > before;
    };
    ruled::v1::RuledCommand put;
    put.mutable_dev_command()->set_target_player_id(p1.myId);
    put.mutable_dev_command()->mutable_put_card_in_zone()->set_card_name("Elvish Visionary");
    put.mutable_dev_command()->mutable_put_card_in_zone()->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
    ASSERT_TRUE(send(p1, put));
    ASSERT_TRUE(send(p1, put));
    ASSERT_EQ(p1.stackDepth, 2);
    p1.clearAfterResolution = true;
    ruled::v1::RuledCommand yield;
    yield.mutable_update_automation()->mutable_yield_to()->set_mode(ruled::v1::AUTOMATION_YIELD_MODE_ENTIRE_STACK);
    ASSERT_TRUE(send(p1, yield));
    ASSERT_TRUE(send(p2, yield));
    QElapsedTimer wait;
    wait.start();
    while ((!p1.latestLegal.automation().full_control() || p1.resolved == 0) && wait.elapsed() < 10000) {
        p1.pump(10);
        p2.pump(10);
    }
    ASSERT_TRUE(p1.latestLegal.automation().full_control());
    // Pump longer than several continuation delays to prove the stale timer cannot keep passing.
    wait.restart();
    while (wait.elapsed() < 500) {
        p1.pump(10);
        p2.pump(10);
    }
    EXPECT_EQ(p1.resolved, 1);
    EXPECT_EQ(p1.stackDepth, 1);
    EXPECT_EQ(p2.stackDepth, 1);
    EXPECT_EQ(p1.priorityPlayer, p1.myId);
    EXPECT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_TRUE(send(p1, yield));
    wait.restart();
    while ((p1.stackDepth || p2.stackDepth) && wait.elapsed() < 10000) {
        p1.pump(10);
        p2.pump(10);
    }
    EXPECT_EQ(p1.stackDepth, 0);
    EXPECT_EQ(p2.stackDepth, 0);
    EXPECT_EQ(p1.resolved, 2);
    EXPECT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    EXPECT_TRUE(p1.latestLegal.automation().paused());
    EXPECT_EQ(p1.latestLegal.automation().yield_mode(), ruled::v1::AUTOMATION_YIELD_MODE_NONE);
}
TEST_F(RuledE2ESmokeTest, NoBluffAutoPassPublishesPrivateReasonsAndPassesUnusedMana)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    AutomationDriver p1(true, "nobluff1", &transcript);
    AutomationDriver p2(false, "nobluff2", &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, "Forest"}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, "Island"}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "no-bluff startup"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "no-bluff observer"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());
    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(10);
        p2.pump(10);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p1.priorityPlayer == p1.myId &&
            p2.phase == p1.phase && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ruled::v1::RuledCommand enable;
    enable.mutable_update_automation()->set_auto_pass(true);
    p1.sendRuled(enable, "disable bluffing");
    ASSERT_TRUE(p1.pumpUntil([&] {
        return p1.latestLegal.automation().auto_pass() &&
            p1.latestLegal.automation().stop_reason() == ruled::v1::AUTOMATION_STOP_REASON_PHASE_STOP;
    }, 10000, "explicit phase stop takes precedence"));
    ruled::v1::RuledCommand policy;
    policy.mutable_set_auto_pass_policy();
    p1.sendRuled(policy, "remove own explicit phase stops");
    p1.sendRuled(enable, "refresh no-bluff availability");
    ASSERT_TRUE(p1.pumpUntil([&] {
        return p1.latestLegal.automation().stop_reason() == ruled::v1::AUTOMATION_STOP_REASON_LAND;
    }, 10000, "available land play holds priority"));
    const auto land = std::find_if(p1.latestLegal.hand_actions().begin(), p1.latestLegal.hand_actions().end(),
                                  [](const auto &action) { return action.kind() == ruled::v1::HAND_ACTION_PLAY_LAND; });
    ASSERT_NE(land, p1.latestLegal.hand_actions().end());
    ruled::v1::RuledCommand play;
    play.mutable_play_land()->mutable_source()->set_hand_index(land->hand_index());
    p1.sendRuled(play, "use the land play; remaining action is Forest mana");
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.priorityPlayer == p2.myId; }, 10000, "unused mana auto-passes"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.priorityPlayer == p2.myId; }, 10000, "observer sees automatic pass"));
    EXPECT_TRUE(p1.latestLegal.automation().auto_pass());
    EXPECT_EQ(p1.latestLegal.automation().stop_reason(), ruled::v1::AUTOMATION_STOP_REASON_WAITING_FOR_PRIORITY);
    EXPECT_FALSE(p2.latestLegal.automation().auto_pass());
    EXPECT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
}
} // namespace ruled_e2e
