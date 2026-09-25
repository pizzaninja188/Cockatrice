#include "ruled_e2e_client.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
TEST_F(RuledE2ESmokeTest, ThreeClientsCompleteOpeningAndReachFirstMain)
{
    const auto started = startServers();
    if (!started)
        FAIL() << started.message();
    if (const std::string message = started.message(); message.rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << message.substr(5);

    SmokeClient p1(QStringLiteral("threep1"), &transcript);
    SmokeClient p2(QStringLiteral("threep2"), &transcript);
    SmokeClient p3(QStringLiteral("threep3"), &transcript);
    std::array<SmokeClient *, 3> clients{&p1, &p2, &p3};
    for (auto *client : clients)
        ASSERT_TRUE(client->loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame(3));
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p3.joinRuledGame(p1.gameId));
    ASSERT_NE(p1.myId, p2.myId);
    ASSERT_NE(p1.myId, p3.myId);
    ASSERT_NE(p2.myId, p3.myId);

    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p3.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    for (auto *client : clients)
        client->sendReady();

    QElapsedTimer deadline;
    deadline.start();
    std::map<int, quint64> actedAtVersion;
    int keeps = 0;
    bool choseFirst = false;
    const auto allAtMain = [&] {
        return std::all_of(clients.begin(), clients.end(), [](const auto *client) {
            return client->gameStarted && client->stateVersion > 0 &&
                   client->phase == ruled::v1::PHASE_ID_MAIN1;
        });
    };
    while (!allAtMain() && deadline.elapsed() < 30000) {
        for (auto *client : clients)
            client->pump(20);
        for (auto *client : clients) {
            if (!client->gameStarted || client->stateVersion == 0 ||
                actedAtVersion[client->myId] == client->stateVersion)
                continue;
            const auto &opening = client->latestLegal.opening();
            ruled::v1::RuledCommand command;
            if (!choseFirst && opening.stage() == ruled::v1::OPENING_STAGE_CHOOSE_STARTING_PLAYER &&
                opening.eligible_starting_player_ids_size() > 0) {
                command.mutable_choose_starting_player()->set_starting_player_id(p1.myId);
                choseFirst = true;
                client->sendRuled(command, QStringLiteral("choose p1 to start"));
            } else if (opening.stage() == ruled::v1::OPENING_STAGE_MULLIGAN && opening.can_keep()) {
                command.mutable_mulligan()->set_keep(true);
                ++keeps;
                client->sendRuled(command, QStringLiteral("keep opening hand"));
            } else if (client->priorityPlayer == client->myId &&
                       client->phase != ruled::v1::PHASE_ID_MAIN1 &&
                       opening.stage() != ruled::v1::OPENING_STAGE_CHOOSE_STARTING_PLAYER &&
                       opening.stage() != ruled::v1::OPENING_STAGE_MULLIGAN) {
                command.mutable_pass_priority();
                client->sendRuled(command, QStringLiteral("pass to first main"));
            } else {
                continue;
            }
            actedAtVersion[client->myId] = client->stateVersion;
        }
    }
    EXPECT_TRUE(allAtMain());
    EXPECT_TRUE(choseFirst);
    EXPECT_EQ(keeps, 3);
    for (auto *client : clients) {
        EXPECT_EQ(client->gameId, p1.gameId);
        EXPECT_EQ(client->activePlayer, p1.myId);
        EXPECT_EQ(client->handSizeByPlayer[client->myId], client == &p1 ? 8 : 7);
    }

    // Advance two complete turns. Every seat must retain the same authoritative
    // turn order, and the third seat must receive an ordinary main phase.
    const auto thirdAtMain = [&] {
        return std::all_of(clients.begin(), clients.end(), [&](const auto *client) {
            return client->activePlayer == p3.myId && client->phase == ruled::v1::PHASE_ID_MAIN1;
        });
    };
    deadline.restart();
    while (!thirdAtMain() && deadline.elapsed() < 45000) {
        for (auto *client : clients)
            client->pump(20);
        SmokeClient *active = nullptr;
        SmokeClient *priority = nullptr;
        for (auto *client : clients) {
            if (client->myId == p1.activePlayer)
                active = client;
            if (client->myId == p1.priorityPlayer)
                priority = client;
        }
        if (active == nullptr)
            continue;
        ruled::v1::RuledCommand command;
        SmokeClient *sender = nullptr;
        const auto cleanup = active->handActions(ruled::v1::HAND_ACTION_CLEANUP_DISCARD);
        if (cleanup.size() > 7) {
            for (int i = 0; i < cleanup.size() - 7; ++i)
                command.mutable_discard_to_hand_size()->add_hand_card_indices(cleanup.at(i)->hand_index());
            sender = active;
        } else if (p1.phase == ruled::v1::PHASE_ID_DECLARE_ATTACKERS) {
            command.mutable_declare_attackers();
            sender = active;
        } else if (p1.phase == ruled::v1::PHASE_ID_DECLARE_BLOCKERS) {
            command.mutable_declare_blockers();
            sender = active == &p1 ? &p2 : &p1;
        } else if (priority != nullptr) {
            command.mutable_pass_priority();
            sender = priority;
        }
        if (sender != nullptr && sender->stateVersion > actedAtVersion[sender->myId]) {
            sender->sendRuled(command, QStringLiteral("advance multiplayer turn"));
            actedAtVersion[sender->myId] = sender->stateVersion;
        }
    }
    EXPECT_TRUE(thirdAtMain());
    EXPECT_GE(p3.handSizeByPlayer[p3.myId], 8);
}
} // namespace
} // namespace ruled_e2e
