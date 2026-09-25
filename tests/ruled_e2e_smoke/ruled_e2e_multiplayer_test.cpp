#include "ruled_e2e_client.h"
#include "ruled_e2e_session.h"
#include <libcockatrice/protocol/pb/command_concede.pb.h>
#include <libcockatrice/protocol/pb/command_leave_game.pb.h>

namespace ruled_e2e
{
namespace
{
class DepartureObserver : public SmokeClient
{
public:
    using SmokeClient::SmokeClient;
    int expectedConceder = -1;
    bool sawEngineConcession = false;
    int expectedWinner = -1;
    bool sawEngineWinner = false;
    void onRuledEvent(const ruled::v1::RuledEvent &event) override
    {
        if (!event.has_log())
            return;
        if (event.log().text() == QStringLiteral("P%1 conceded").arg(expectedConceder).toStdString())
            sawEngineConcession = true;
        if (event.log().text() == QStringLiteral("Game over. Winner: %1").arg(expectedWinner).toStdString())
            sawEngineWinner = true;
    }
};

TEST_F(RuledE2ESmokeTest, ThreeClientsRotateTurnsThenConcedeAndLeave)
{
    const auto started = startServers();
    if (!started)
        FAIL() << started.message();
    if (const std::string message = started.message(); message.rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << message.substr(5);

    DepartureObserver p1(QStringLiteral("threep1"), &transcript);
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

    // Split attacks between both opponents. One defender blocks; the other has no
    // blocker and must still get their own empty declaration turn.
    auto sendAndSettle = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command, const QString &label) {
        const quint64 commandId = sender.nextCmdId;
        const std::array<quint64, 3> before{p1.stateVersion, p2.stateVersion, p3.stateVersion};
        sender.sendRuled(command, label);
        if (!sender.pumpUntil([&] { return sender.responses.count(commandId) > 0; }, 10000, "combat response"))
            return false;
        if (sender.responses[commandId].response_code() != Response::RespOk)
            return false;
        QElapsedTimer sync;
        sync.start();
        while ((p1.stateVersion <= before[0] || p2.stateVersion <= before[1] || p3.stateVersion <= before[2]) &&
               sync.elapsed() < 10000)
            for (auto *client : clients)
                client->pump(20);
        return p1.stateVersion > before[0] && p2.stateVersion > before[1] && p3.stateVersion > before[2];
    };
    auto putReady = [&](int controller, const char *name) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(controller);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(name);
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(true);
        return sendAndSettle(p3, command, QStringLiteral("place combat creature"));
    };
    ASSERT_TRUE(putReady(p3.myId, "Grizzly Bears"));
    ASSERT_TRUE(putReady(p3.myId, "Grizzly Bears"));
    ASSERT_TRUE(putReady(p1.myId, "Grizzly Bears"));
    const auto &attackers = p1.battlefieldByPlayer[p3.myId];
    ASSERT_GE(attackers.size(), 2u);
    const quint32 attackP1 = attackers[0].oid;
    const quint32 attackP2 = attackers[1].oid;
    const auto &blockers = p1.battlefieldByPlayer[p1.myId];
    ASSERT_FALSE(blockers.empty());
    const quint32 blocker = blockers.back().oid;
    // The observer only records LifeChanged events; an untouched starting total
    // has no map entry yet.
    const auto observedLife = [&](int playerId) {
        const auto found = p1.lifeByPlayer.find(playerId);
        return found == p1.lifeByPlayer.end() ? 20 : found->second;
    };
    const int p1LifeBeforeCombat = observedLife(p1.myId);
    const int p2LifeBeforeCombat = observedLife(p2.myId);

    auto pass = [&] {
        SmokeClient *actor = nullptr;
        for (auto *client : clients)
            if (client->myId == p1.priorityPlayer)
                actor = client;
        if (actor == nullptr)
            return false;
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndSettle(*actor, command, QStringLiteral("advance split combat"));
    };
    deadline.restart();
    while (p1.phase != ruled::v1::PHASE_ID_DECLARE_ATTACKERS && deadline.elapsed() < 20000)
        ASSERT_TRUE(pass());
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_DECLARE_ATTACKERS);
    ruled::v1::RuledCommand attack;
    for (const auto &assignment : p3.latestLegal.legal_attack_assignments()) {
        if ((assignment.attacker_object_id() == attackP1 && assignment.defending_player_id() == p1.myId) ||
            (assignment.attacker_object_id() == attackP2 && assignment.defending_player_id() == p2.myId))
            *attack.mutable_declare_attackers()->add_assignments() = assignment;
    }
    ASSERT_EQ(attack.declare_attackers().assignments_size(), 2);
    ASSERT_TRUE(sendAndSettle(p3, attack, QStringLiteral("split attackers across defenders")));
    deadline.restart();
    while (p1.phase != ruled::v1::PHASE_ID_DECLARE_BLOCKERS && deadline.elapsed() < 20000)
        ASSERT_TRUE(pass());
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.latestLegal.legal_block_pairs_size() > 0; }, 10000,
                             "first defender's block offer"));
    EXPECT_EQ(p2.latestLegal.legal_block_pairs_size(), 0);
    ruled::v1::RuledCommand block;
    for (const auto &pair : p1.latestLegal.legal_block_pairs())
        if (pair.blocker_id() == blocker && pair.attacker_id() == attackP1)
            *block.mutable_declare_blockers()->add_block_pairs() = pair;
    ASSERT_EQ(block.declare_blockers().block_pairs_size(), 1);
    ASSERT_TRUE(sendAndSettle(p1, block, QStringLiteral("first defender blocks")));
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_DECLARE_BLOCKERS);
    ASSERT_EQ(p1.priorityPlayer, p2.myId);
    ruled::v1::RuledCommand noBlocks;
    noBlocks.mutable_declare_blockers();
    ASSERT_TRUE(sendAndSettle(p2, noBlocks, QStringLiteral("second defender declares no blocks")));
    deadline.restart();
    while (p1.phase != ruled::v1::PHASE_ID_MAIN2 && deadline.elapsed() < 20000)
        ASSERT_TRUE(pass());
    EXPECT_EQ(observedLife(p1.myId), p1LifeBeforeCombat);
    EXPECT_EQ(observedLife(p2.myId), p2LifeBeforeCombat - 2);

    // A raw ruled Concede would leave Servatrice's physical seat in play.
    // Only the normal client concession path may depart from both systems.
    const quint64 directConcedeCommandId = p2.nextCmdId;
    ruled::v1::RuledCommand directConcede;
    directConcede.mutable_concede();
    p2.sendRuled(directConcede, QStringLiteral("reject direct ruled concession"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.responses.count(directConcedeCommandId) > 0; }, 10000,
                             "direct ruled concession rejection"));
    EXPECT_EQ(p2.responses[directConcedeCommandId].response_code(), Response::RespInvalidCommand);
    EXPECT_FALSE(p1.sawEngineConcession);

    // The normal client UI sends legacy Command_Concede. It must become the
    // engine's logged Concede command before the other two players continue.
    p1.expectedConceder = p2.myId;
    const quint64 concedeCommandId = p2.nextCmdId;
    CommandContainer concede;
    concede.set_game_id(p2.gameId);
    concede.add_game_command()->MutableExtension(Command_Concede::ext);
    p2.sendContainer(concede);
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.responses.count(concedeCommandId) > 0; }, 10000,
                             "legacy concede response"));
    ASSERT_EQ(p2.responses[concedeCommandId].response_code(), Response::RespOk);
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.sawEngineConcession; }, 10000,
                             "engine concession batch"));
    ASSERT_TRUE(p3.pumpUntil([&] { return p3.stateVersion >= p1.stateVersion; }, 10000,
                             "remaining third player catches up"));
    EXPECT_TRUE(p1.gameStarted);
    EXPECT_TRUE(p3.gameStarted);

    const quint64 unconcedeCommandId = p2.nextCmdId;
    CommandContainer unconcede;
    unconcede.set_game_id(p2.gameId);
    unconcede.add_game_command()->MutableExtension(Command_Unconcede::ext);
    p2.sendContainer(unconcede);
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.responses.count(unconcedeCommandId) > 0; }, 10000,
                             "ruled unconcede rejection"));
    EXPECT_EQ(p2.responses[unconcedeCommandId].response_code(), Response::RespInvalidCommand);

    ASSERT_NE(p1.priorityPlayer, p2.myId);
    SmokeClient *remainingActor = p1.priorityPlayer == p1.myId ? static_cast<SmokeClient *>(&p1) : &p3;
    ASSERT_EQ(remainingActor->myId, p1.priorityPlayer);
    const quint64 continuationCommandId = remainingActor->nextCmdId;
    const quint64 beforeContinuation = p1.stateVersion;
    ruled::v1::RuledCommand passAfterDeparture;
    passAfterDeparture.mutable_pass_priority();
    remainingActor->sendRuled(passAfterDeparture, QStringLiteral("continue after concession"));
    ASSERT_TRUE(remainingActor->pumpUntil([&] { return remainingActor->responses.count(continuationCommandId) > 0; },
                                          10000, "post-concession priority response"));
    EXPECT_EQ(remainingActor->responses[continuationCommandId].response_code(), Response::RespOk);
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.stateVersion > beforeContinuation; }, 10000,
                             "post-concession engine turn continuation"));

    // Leaving after a different player conceded also goes through the engine,
    // and the one remaining player receives the final winner event.
    p1.expectedConceder = p3.myId;
    p1.expectedWinner = p1.myId;
    p1.sawEngineConcession = false;
    CommandContainer leave;
    leave.set_game_id(p3.gameId);
    leave.add_game_command()->MutableExtension(Command_LeaveGame::ext);
    p3.sendContainer(leave);
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.sawEngineConcession && p1.sawEngineWinner; }, 10000,
                             "engine final departure and winner"));
    ASSERT_TRUE(p1.pumpUntil([&] { return !p1.gameStarted; }, 10000, "physical game end"));
}

TEST_F(RuledE2ESmokeTest, UnregisteredDisconnectDuringThreePlayerOpeningLetsOtherSeatsContinue)
{
    const auto started = startServers();
    if (!started)
        FAIL() << started.message();
    if (const std::string message = started.message(); message.rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << message.substr(5);

    DepartureObserver p1(QStringLiteral("disconnectp1"), &transcript);
    SmokeClient p2(QStringLiteral("disconnectp2"), &transcript);
    SmokeClient p3(QStringLiteral("disconnectp3"), &transcript);
    std::array<SmokeClient *, 3> clients{&p1, &p2, &p3};
    for (auto *client : clients)
        ASSERT_TRUE(client->loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame(3));
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p3.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p3.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    for (auto *client : clients)
        client->sendReady();
    for (auto *client : clients)
        ASSERT_TRUE(client->pumpUntil([&] { return client->gameStarted && client->stateVersion > 0; }, 20000,
                                      "three-player opening"));

    p1.expectedConceder = p3.myId;
    p3.sock.disconnectFromHost();
    if (p3.sock.state() != QAbstractSocket::UnconnectedState)
        ASSERT_TRUE(p3.sock.waitForDisconnected(10000));
    ASSERT_EQ(p3.sock.state(), QAbstractSocket::UnconnectedState);
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.sawEngineConcession; }, 10000,
                             "disconnected player's engine concession"));
    EXPECT_TRUE(p1.gameStarted);

    QElapsedTimer deadline;
    deadline.start();
    std::map<int, quint64> actedAtVersion;
    std::array<SmokeClient *, 2> remaining{&p1, &p2};
    int keeps = 0;
    while (deadline.elapsed() < 30000 &&
           !(p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1)) {
        for (auto *client : remaining)
            client->pump(20);
        for (auto *client : remaining) {
            if (actedAtVersion[client->myId] == client->stateVersion)
                continue;
            const auto &opening = client->latestLegal.opening();
            ruled::v1::RuledCommand command;
            if (opening.stage() == ruled::v1::OPENING_STAGE_CHOOSE_STARTING_PLAYER &&
                opening.eligible_starting_player_ids_size() > 0) {
                command.mutable_choose_starting_player()->set_starting_player_id(p1.myId);
            } else if (opening.stage() == ruled::v1::OPENING_STAGE_MULLIGAN && opening.can_keep()) {
                command.mutable_mulligan()->set_keep(true);
                ++keeps;
            } else if (client->priorityPlayer == client->myId && client->phase != ruled::v1::PHASE_ID_MAIN1) {
                command.mutable_pass_priority();
            } else {
                continue;
            }
            client->sendRuled(command, QStringLiteral("continue after disconnect"));
            actedAtVersion[client->myId] = client->stateVersion;
        }
    }
    EXPECT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    EXPECT_EQ(p2.phase, ruled::v1::PHASE_ID_MAIN1);
    EXPECT_EQ(keeps, 2);
    EXPECT_TRUE(p1.gameStarted);
    EXPECT_TRUE(p2.gameStarted);
}

TEST_F(RuledE2ESmokeTest, FailedThreePlayerConcessionFreezesRuledSession)
{
    const auto started = startServers();
    if (!started)
        FAIL() << started.message();
    if (const std::string message = started.message(); message.rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << message.substr(5);

    DepartureObserver p1(QStringLiteral("faileddeparturep1"), &transcript);
    SmokeClient p2(QStringLiteral("faileddeparturep2"), &transcript);
    SmokeClient p3(QStringLiteral("faileddeparturep3"), &transcript);
    for (auto *client : std::array<SmokeClient *, 3>{&p1, &p2, &p3})
        ASSERT_TRUE(client->loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame(3));
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p3.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p3.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    p1.sendReady();
    p2.sendReady();
    p3.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "three-player session before engine disconnect"));

    p1.expectedConceder = p2.myId;
    const quint64 versionBefore = p1.stateVersion;
    const int noticesBefore = p1.notifyCustomCount;
    sidecar.kill();
    ASSERT_TRUE(sidecar.waitForFinished(5000));

    const quint64 concedeCommandId = p2.nextCmdId;
    CommandContainer concede;
    concede.set_game_id(p2.gameId);
    concede.add_game_command()->MutableExtension(Command_Concede::ext);
    p2.sendContainer(concede);
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.responses.count(concedeCommandId) > 0; }, 10000,
                             "failed concession response"));
    EXPECT_NE(p2.responses[concedeCommandId].response_code(), Response::RespOk);
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.notifyCustomCount > noticesBefore; }, 10000,
                             "fatal rules engine notice"));
    EXPECT_TRUE(p1.lastNotifyContent.contains(QStringLiteral("can no longer continue")));
    EXPECT_FALSE(p1.sawEngineConcession);
    EXPECT_EQ(p1.stateVersion, versionBefore);
}
} // namespace
} // namespace ruled_e2e
