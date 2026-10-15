#include "ruled_e2e_client.h"
#include "ruled_e2e_session.h"
#include <libcockatrice/protocol/pb/command_concede.pb.h>
#include <libcockatrice/protocol/pb/command_leave_game.pb.h>
#include <libcockatrice/protocol/pb/event_destroy_card.pb.h>

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
    std::vector<std::pair<int, int>> destroyedStackCards;
    std::vector<quint32> lightningBoltStackOids;
    std::vector<quint32> removedStackObjectOids;
    std::vector<quint32> pushedAndRemovedInSameBatch;
    std::vector<quint32> pushedInCurrentBatch;
    std::vector<quint32> removedInCurrentBatch;
    void onBatchBegin(const ruled::v1::RuledEventBatch &) override
    {
        pushedInCurrentBatch.clear();
        removedInCurrentBatch.clear();
    }
    void onPhysicalEvent(const GameEvent &event) override
    {
        if (event.HasExtension(Event_DestroyCard::ext)) {
            const auto &destroyed = event.GetExtension(Event_DestroyCard::ext);
            if (destroyed.zone_name() == ZoneNames::STACK) {
                destroyedStackCards.emplace_back(event.player_id(), static_cast<int>(destroyed.card_id()));
            }
        }
    }
    void onRuledEvent(const ruled::v1::RuledEvent &event) override
    {
        if (event.has_stack_pushed() && event.stack_pushed().card_id() == "lightning_bolt") {
            lightningBoltStackOids.push_back(event.stack_pushed().object_id());
        }
        if (event.has_stack_pushed() && event.stack_pushed().card_id() == "bitter_triumph") {
            pushedInCurrentBatch.push_back(event.stack_pushed().object_id());
        }
        if (event.has_stack_object_removed()) {
            removedStackObjectOids.push_back(event.stack_object_removed().object_id());
            removedInCurrentBatch.push_back(event.stack_object_removed().object_id());
        }
        if (!event.has_log())
            return;
        if (event.log().text() == QStringLiteral("P%1 conceded").arg(expectedConceder).toStdString())
            sawEngineConcession = true;
        if (event.log().text() == QStringLiteral("Game over. Winner: %1").arg(expectedWinner).toStdString())
            sawEngineWinner = true;
    }
    void onBatchEventsComplete(const ruled::v1::RuledEventBatch &) override
    {
        for (const quint32 pushed : pushedInCurrentBatch) {
            if (std::find(removedInCurrentBatch.begin(), removedInCurrentBatch.end(), pushed) !=
                removedInCurrentBatch.end()) {
                pushedAndRemovedInSameBatch.push_back(pushed);
            }
        }
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
    // legal blocker and is skipped without an empty declaration command.
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
    ASSERT_EQ(p1.priorityPlayer, p3.myId);
    EXPECT_EQ(p2.latestLegal.legal_block_pairs_size(), 0);
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

TEST_F(RuledE2ESmokeTest, DepartedPayerSpellLeavesCanonicalPhysicalStackBeforeAnyFollowupCommand)
{
    const auto started = startServers();
    if (!started)
        FAIL() << started.message();
    if (const std::string message = started.message(); message.rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << message.substr(5);

    DepartureObserver p1(QStringLiteral("departure-stack-p1"), &transcript);
    SmokeClient p2(QStringLiteral("departure-stack-p2"), &transcript);
    DepartureObserver p3(QStringLiteral("departure-stack-p3"), &transcript);
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
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p3.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    for (auto *client : clients)
        client->sendReady();
    for (auto *client : clients) {
        ASSERT_TRUE(client->pumpUntil([&] { return client->gameStarted && client->stateVersion > 0; }, 20000,
                                      "departure-stack game start"));
        ASSERT_TRUE(client->publishMain1Stops());
    }

    QElapsedTimer openingDeadline;
    openingDeadline.start();
    std::map<int, quint64> actedAtVersion;
    int keeps = 0;
    bool choseFirst = false;
    const auto allAtMain = [&] {
        return std::all_of(clients.begin(), clients.end(), [](const auto *client) {
            return client->gameStarted && client->stateVersion > 0 &&
                   client->phase == ruled::v1::PHASE_ID_MAIN1;
        });
    };
    while (!allAtMain() && openingDeadline.elapsed() < 30000) {
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
                client->sendRuled(command, QStringLiteral("choose p1 as active player"));
            } else if (opening.stage() == ruled::v1::OPENING_STAGE_MULLIGAN && opening.can_keep()) {
                command.mutable_mulligan()->set_keep(true);
                ++keeps;
                client->sendRuled(command, QStringLiteral("keep departure-stack opening hand"));
            } else if (client->priorityPlayer == client->myId && client->phase != ruled::v1::PHASE_ID_MAIN1 &&
                       opening.stage() != ruled::v1::OPENING_STAGE_CHOOSE_STARTING_PLAYER &&
                       opening.stage() != ruled::v1::OPENING_STAGE_MULLIGAN) {
                command.mutable_pass_priority();
                client->sendRuled(command, QStringLiteral("advance to departure-stack main phase"));
            } else {
                continue;
            }
            actedAtVersion[client->myId] = client->stateVersion;
        }
    }
    ASSERT_TRUE(allAtMain());
    ASSERT_TRUE(choseFirst);
    ASSERT_EQ(keeps, 3);
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    auto sendAndSettle = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command, const QString &label) {
        const quint64 commandId = sender.nextCmdId;
        const std::array<quint64, 3> before{p1.stateVersion, p2.stateVersion, p3.stateVersion};
        sender.sendRuled(command, label);
        QElapsedTimer wait;
        wait.start();
        while (sender.responses.count(commandId) == 0 && wait.elapsed() < 10000) {
            for (auto *client : clients)
                client->pump(20);
        }
        if (sender.responses.count(commandId) == 0 || sender.responses.at(commandId).response_code() != Response::RespOk)
            return false;
        wait.restart();
        const auto allUpdated = [&] {
            return p1.stateVersion > before[0] && p2.stateVersion > before[1] && p3.stateVersion > before[2];
        };
        while (!allUpdated() && wait.elapsed() < 10000) {
            for (auto *client : clients)
                client->pump(20);
        }
        return allUpdated();
    };
    auto devPut = [&](int targetPlayer, const char *name) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        dev->mutable_put_card_in_zone()->set_zone(ruled::v1::DEV_ZONE_HAND);
        return sendAndSettle(p1, command, QStringLiteral("dev: put %1 into hand").arg(QString::fromLatin1(name)));
    };
    auto devMana = [&](int targetPlayer, int red, int blue, int colorless) {
        ruled::v1::RuledCommand command;
        auto *mana = command.mutable_dev_command();
        mana->set_target_player_id(targetPlayer);
        mana->mutable_add_mana()->set_r(red);
        mana->mutable_add_mana()->set_u(blue);
        mana->mutable_add_mana()->set_c(colorless);
        return sendAndSettle(p1, command, QStringLiteral("dev: add mana for stack departure"));
    };
    auto passPriority = [&](SmokeClient &sender) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndSettle(sender, command, QStringLiteral("pass priority in stack departure scenario"));
    };

    ASSERT_TRUE(devPut(p2.myId, "Lightning Bolt"));
    ASSERT_TRUE(devMana(p2.myId, 1, 0, 2));
    ASSERT_TRUE(devPut(p3.myId, "Lightning Bolt"));
    ASSERT_TRUE(devMana(p3.myId, 1, 0, 0));
    ASSERT_TRUE(devPut(p1.myId, "Convolute"));
    ASSERT_TRUE(devMana(p1.myId, 0, 1, 2));

    ASSERT_TRUE(passPriority(p1));
    ASSERT_EQ(p2.priorityPlayer, p2.myId);
    const auto *boltForPayer = p2.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Lightning Bolt"));
    ASSERT_NE(boltForPayer, nullptr);
    ruled::v1::RuledCommand castPayerBolt;
    castPayerBolt.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    castPayerBolt.mutable_cast_spell()->mutable_source()->set_hand_index(boltForPayer->hand_index());
    auto *payerBoltTarget = castPayerBolt.mutable_cast_spell()->add_targets();
    payerBoltTarget->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
    payerBoltTarget->set_object_id(p1.myId);
    ASSERT_TRUE(sendAndSettle(p2, castPayerBolt, QStringLiteral("B casts Lightning Bolt")));
    ASSERT_EQ(p2.priorityPlayer, p2.myId);
    ASSERT_TRUE(passPriority(p2));
    ASSERT_EQ(p3.priorityPlayer, p3.myId);
    ASSERT_EQ(p3.lightningBoltStackOids.size(), 1u);
    const quint32 payerBoltOid = p3.lightningBoltStackOids.back();
    ASSERT_TRUE(p3.serverCardByEngineOid.count(payerBoltOid));
    const int payerBoltServerId = p3.serverCardByEngineOid.at(payerBoltOid);

    ASSERT_EQ(p3.priorityPlayer, p3.myId);
    const auto *survivorBolt = p3.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Lightning Bolt"));
    ASSERT_NE(survivorBolt, nullptr);
    ruled::v1::RuledCommand castSurvivorBolt;
    castSurvivorBolt.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    castSurvivorBolt.mutable_cast_spell()->mutable_source()->set_hand_index(survivorBolt->hand_index());
    auto *survivorBoltTarget = castSurvivorBolt.mutable_cast_spell()->add_targets();
    survivorBoltTarget->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
    survivorBoltTarget->set_object_id(p1.myId);
    ASSERT_TRUE(sendAndSettle(p3, castSurvivorBolt, QStringLiteral("C casts same-name Lightning Bolt")));
    ASSERT_EQ(p3.lightningBoltStackOids.size(), 2u);
    const quint32 survivorBoltOid = p3.lightningBoltStackOids.back();
    ASSERT_NE(payerBoltOid, survivorBoltOid);
    ASSERT_TRUE(p3.serverCardByEngineOid.count(survivorBoltOid));
    const int survivorBoltServerId = p3.serverCardByEngineOid.at(survivorBoltOid);
    ASSERT_NE(payerBoltServerId, survivorBoltServerId);
    EXPECT_EQ(p3.serverCardByEngineOid.at(payerBoltOid), payerBoltServerId);
    EXPECT_EQ(p3.serverCardByEngineOid.at(survivorBoltOid), survivorBoltServerId);

    ASSERT_EQ(p3.priorityPlayer, p3.myId);
    ASSERT_TRUE(passPriority(p3));
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    const auto *convolute = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Convolute"));
    ASSERT_NE(convolute, nullptr);
    ruled::v1::RuledCommand castConvolute;
    castConvolute.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    castConvolute.mutable_cast_spell()->mutable_source()->set_hand_index(convolute->hand_index());
    auto *convoluteTarget = castConvolute.mutable_cast_spell()->add_targets();
    convoluteTarget->set_kind(ruled::v1::TARGET_REF_KIND_STACK);
    convoluteTarget->set_object_id(payerBoltOid);
    ASSERT_TRUE(sendAndSettle(p1, castConvolute, QStringLiteral("A casts Convolute at B's Bolt")));
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    ASSERT_TRUE(passPriority(p1));
    ASSERT_EQ(p2.priorityPlayer, p2.myId);
    ASSERT_TRUE(passPriority(p2));
    ASSERT_EQ(p3.priorityPlayer, p3.myId);
    ASSERT_TRUE(passPriority(p3));
    ASSERT_TRUE(p2.pendingChoice.has_value());
    ASSERT_EQ(p2.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    ASSERT_FALSE(p1.pendingChoice.has_value());
    ASSERT_FALSE(p3.pendingChoice.has_value());

    p1.expectedConceder = p2.myId;
    p3.expectedConceder = p2.myId;
    const quint64 p1BeforeConcede = p1.stateVersion;
    const quint64 p3BeforeConcede = p3.stateVersion;
    const quint64 concedeCommandId = p2.nextCmdId;
    CommandContainer concede;
    concede.set_game_id(p2.gameId);
    concede.add_game_command()->MutableExtension(Command_Concede::ext);
    p2.sendContainer(concede);
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.responses.count(concedeCommandId) > 0; }, 10000,
                             "departed payer legacy concede response"));
    ASSERT_EQ(p2.responses[concedeCommandId].response_code(), Response::RespOk);
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.sawEngineConcession && p1.stateVersion > p1BeforeConcede; }, 10000,
                             "departure stack cleanup batch at A"));
    ASSERT_TRUE(p3.pumpUntil([&] { return p3.sawEngineConcession && p3.stateVersion > p3BeforeConcede; }, 10000,
                             "departure stack cleanup batch at C"));

    const int canonicalStackPlayer = std::min({p1.myId, p2.myId, p3.myId});
    const auto hasDestroyed = [&](const DepartureObserver &client, int serverCardId) {
        return std::find(client.destroyedStackCards.begin(), client.destroyedStackCards.end(),
                         std::make_pair(canonicalStackPlayer, serverCardId)) != client.destroyedStackCards.end();
    };
    EXPECT_TRUE(hasDestroyed(p1, payerBoltServerId))
        << "departure response must delete B's exact physical Bolt from the canonical stack, before any later command";
    EXPECT_TRUE(hasDestroyed(p3, payerBoltServerId))
        << "the other survivor must receive the same exact physical stack deletion";
    EXPECT_FALSE(hasDestroyed(p1, survivorBoltServerId))
        << "deleting one same-name spell must not delete C's physical Bolt";
    EXPECT_FALSE(hasDestroyed(p3, survivorBoltServerId))
        << "deleting one same-name spell must not delete C's physical Bolt";
    EXPECT_EQ(std::count(p1.removedStackObjectOids.begin(), p1.removedStackObjectOids.end(), payerBoltOid), 1)
        << "the engine identifies B's exact no-destination stack removal";
    EXPECT_EQ(std::count(p3.removedStackObjectOids.begin(), p3.removedStackObjectOids.end(), payerBoltOid), 1)
        << "the engine removal OID reaches both survivors";
    EXPECT_EQ(std::count(p1.removedStackObjectOids.begin(), p1.removedStackObjectOids.end(), survivorBoltOid), 0);
    EXPECT_EQ(std::count(p3.removedStackObjectOids.begin(), p3.removedStackObjectOids.end(), survivorBoltOid), 0);
    EXPECT_TRUE(p1.gameStarted);
    EXPECT_TRUE(p3.gameStarted);
    EXPECT_FALSE(p2.resolutionPromptActive);
    EXPECT_EQ(p1.stackDepth, 1) << "C's unrelated same-name Bolt remains after B departs";
    EXPECT_EQ(p3.stackDepth, 1) << "C's unrelated same-name Bolt remains after B departs";
    EXPECT_EQ(p1.counteredStackObjectIds.count(payerBoltOid), 0u)
        << "departure removes B's spell without claiming a counter";
}

TEST_F(RuledE2ESmokeTest, BitterTriumphPayingAllLifePushesAndRemovesInOneBatch)
{
    const auto started = startServers();
    if (!started)
        FAIL() << started.message();
    if (const std::string message = started.message(); message.rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << message.substr(5);

    DepartureObserver p1(QStringLiteral("life-zero-p1"), &transcript);
    DepartureObserver p2(QStringLiteral("life-zero-p2"), &transcript);
    DepartureObserver p3(QStringLiteral("life-zero-p3"), &transcript);
    std::array<SmokeClient *, 3> clients{&p1, &p2, &p3};
    for (auto *client : clients)
        ASSERT_TRUE(client->loginAndJoinRoom());

    // Ruled seats use 20 life regardless of the room's casual starting-life setting. Lower the
    // payer to exactly 3 with an ordinary Fireball so the following real additional-cost payment
    // creates the actual cast-then-state-based-loss batch without test-only engine state edits.
    ASSERT_TRUE(p1.createRuledGame(3));
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p3.joinRuledGame(p1.gameId));

    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Swamp")}})));
    ASSERT_TRUE(p3.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    for (auto *client : clients)
        client->sendReady();

    QElapsedTimer deadline;
    deadline.start();
    std::map<int, quint64> actedAtVersion;
    int keeps = 0;
    bool choseP2 = false;
    const auto p2AtMain = [&] {
        return std::all_of(clients.begin(), clients.end(), [&](const auto *client) {
            return client->gameStarted && client->stateVersion > 0 && client->phase == ruled::v1::PHASE_ID_MAIN1 &&
                   client->activePlayer == p2.myId;
        });
    };
    while (!p2AtMain() && deadline.elapsed() < 30000) {
        for (auto *client : clients)
            client->pump(20);
        for (auto *client : clients) {
            if (!client->gameStarted || client->stateVersion == 0 ||
                actedAtVersion[client->myId] == client->stateVersion)
                continue;
            const auto &opening = client->latestLegal.opening();
            ruled::v1::RuledCommand command;
            if (!choseP2 && opening.stage() == ruled::v1::OPENING_STAGE_CHOOSE_STARTING_PLAYER &&
                opening.eligible_starting_player_ids_size() > 0) {
                command.mutable_choose_starting_player()->set_starting_player_id(p2.myId);
                choseP2 = true;
                client->sendRuled(command, QStringLiteral("choose the Bitter Triumph payer to start"));
            } else if (opening.stage() == ruled::v1::OPENING_STAGE_MULLIGAN && opening.can_keep()) {
                command.mutable_mulligan()->set_keep(true);
                ++keeps;
                client->sendRuled(command, QStringLiteral("keep three-player life-zero opening hand"));
            } else if (client->priorityPlayer == client->myId && client->phase != ruled::v1::PHASE_ID_MAIN1 &&
                       opening.stage() != ruled::v1::OPENING_STAGE_CHOOSE_STARTING_PLAYER &&
                       opening.stage() != ruled::v1::OPENING_STAGE_MULLIGAN) {
                command.mutable_pass_priority();
                client->sendRuled(command, QStringLiteral("advance to payer's first main phase"));
            } else {
                continue;
            }
            actedAtVersion[client->myId] = client->stateVersion;
        }
    }
    ASSERT_TRUE(p2AtMain());
    ASSERT_TRUE(choseP2);
    ASSERT_EQ(keeps, 3);
    ASSERT_EQ(p2.priorityPlayer, p2.myId);

    auto sendAndSettle = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command, const QString &label) {
        const quint64 commandId = sender.nextCmdId;
        const std::array<quint64, 3> before{p1.stateVersion, p2.stateVersion, p3.stateVersion};
        sender.sendRuled(command, label);
        QElapsedTimer wait;
        wait.start();
        while (sender.responses.count(commandId) == 0 && wait.elapsed() < 10000) {
            for (auto *client : clients)
                client->pump(20);
        }
        if (sender.responses.count(commandId) == 0 || sender.responses[commandId].response_code() != Response::RespOk)
            return false;
        wait.restart();
        while ((p1.stateVersion <= before[0] || p2.stateVersion <= before[1] || p3.stateVersion <= before[2]) &&
               wait.elapsed() < 10000) {
            for (auto *client : clients)
                client->pump(20);
        }
        return p1.stateVersion > before[0] && p2.stateVersion > before[1] && p3.stateVersion > before[2];
    };
    auto devPut = [&](int targetPlayer, const char *name, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        dev->mutable_put_card_in_zone()->set_zone(zone);
        dev->mutable_put_card_in_zone()->set_ready(ready);
        return sendAndSettle(p1, command,
                             QStringLiteral("dev: put %1 for life-zero cast").arg(QString::fromLatin1(name)));
    };
    ASSERT_TRUE(devPut(p2.myId, "Fireball", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devPut(p2.myId, "Bitter Triumph", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devPut(p3.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD, true));

    ruled::v1::RuledCommand fireballMana;
    fireballMana.mutable_dev_command()->set_target_player_id(p2.myId);
    fireballMana.mutable_dev_command()->mutable_add_mana()->set_r(1);
    fireballMana.mutable_dev_command()->mutable_add_mana()->set_c(17);
    ASSERT_TRUE(sendAndSettle(p1, fireballMana, QStringLiteral("dev: add mana for Fireball life setup")));
    const auto *fireball = p2.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Fireball"));
    ASSERT_NE(fireball, nullptr);
    const quint32 fireballKey = fireball->hand_index() << 8;
    const auto fireballTargets = p2.latestLegal.valid_targets_by_hand_slot().find(fireballKey);
    ASSERT_NE(fireballTargets, p2.latestLegal.valid_targets_by_hand_slot().end());
    ASSERT_GT(fireballTargets->second.groups_size(), 0);
    ASSERT_TRUE(fireballTargets->second.groups(0).can_target_self());
    ruled::v1::RuledCommand lowerLife;
    auto *fireballCast = lowerLife.mutable_cast_spell();
    fireballCast->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    fireballCast->mutable_source()->set_hand_index(fireball->hand_index());
    fireballCast->set_x_value(17);
    auto *selfTarget = fireballCast->add_targets();
    selfTarget->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
    selfTarget->set_object_id(p2.myId);
    ASSERT_TRUE(sendAndSettle(p2, lowerLife, QStringLiteral("cast Fireball at self to reach three life")));
    auto passPriority = [&](SmokeClient &sender) {
        ruled::v1::RuledCommand pass;
        pass.mutable_pass_priority();
        return sendAndSettle(sender, pass, QStringLiteral("pass to resolve life-setup Fireball"));
    };
    ASSERT_TRUE(passPriority(p2));
    ASSERT_TRUE(passPriority(p3));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_EQ(p2.lifeByPlayer[p2.myId], 3);
    ASSERT_EQ(p1.lifeByPlayer[p2.myId], 3);

    ruled::v1::RuledCommand bitterTriumphMana;
    bitterTriumphMana.mutable_dev_command()->set_target_player_id(p2.myId);
    bitterTriumphMana.mutable_dev_command()->mutable_add_mana()->set_b(1);
    bitterTriumphMana.mutable_dev_command()->mutable_add_mana()->set_c(1);
    ASSERT_TRUE(
        sendAndSettle(p1, bitterTriumphMana, QStringLiteral("dev: add black and generic mana for Bitter Triumph")));

    const auto *spell = p2.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Bitter Triumph"));
    ASSERT_NE(spell, nullptr);
    ASSERT_EQ(spell->cost_choices().cast_cost_groups_size(), 1);
    const auto &costGroup = spell->cost_choices().cast_cost_groups(0);
    const auto payLife = std::find_if(costGroup.options().begin(), costGroup.options().end(), [](const auto &option) {
        return option.kind() == ruled::v1::CAST_COST_OPTION_KIND_PAY_LIFE;
    });
    ASSERT_NE(payLife, costGroup.options().end());
    ASSERT_TRUE(payLife->selectable());
    const quint32 targetKey = spell->hand_index() << 8;
    const auto targets = p2.latestLegal.valid_targets_by_hand_slot().find(targetKey);
    ASSERT_NE(targets, p2.latestLegal.valid_targets_by_hand_slot().end());
    ASSERT_GT(targets->second.groups_size(), 0);
    ASSERT_GT(targets->second.groups(0).valid_permanent_ids_size(), 0);
    const quint32 targetOid = targets->second.groups(0).valid_permanent_ids(0);
    ASSERT_TRUE(p2.handServerCardBySlot.count(static_cast<int>(spell->hand_index())));
    const int physicalSpellId = p2.handServerCardBySlot.at(static_cast<int>(spell->hand_index()));

    ruled::v1::RuledCommand cast;
    auto *castSpell = cast.mutable_cast_spell();
    castSpell->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    castSpell->mutable_source()->set_hand_index(spell->hand_index());
    auto *selection = castSpell->add_cast_cost_group_selections();
    selection->set_group_index(costGroup.group_index());
    selection->set_option_index(payLife->option_index());
    auto *target = castSpell->add_targets();
    target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    target->set_object_id(targetOid);

    const quint64 p1BeforeCast = p1.stateVersion;
    const quint64 p3BeforeCast = p3.stateVersion;
    const quint64 castCommandId = p2.nextCmdId;
    p2.sendRuled(cast, QStringLiteral("pay all three life to cast Bitter Triumph"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.responses.count(castCommandId) > 0; }, 10000,
                             "life-zero Bitter Triumph cast response"));
    ASSERT_EQ(p2.responses[castCommandId].response_code(), Response::RespOk);
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.stateVersion > p1BeforeCast && !p1.pushedAndRemovedInSameBatch.empty(); },
                             10000, "same-batch push/removal observed by first survivor"));
    ASSERT_TRUE(p3.pumpUntil([&] { return p3.stateVersion > p3BeforeCast && !p3.pushedAndRemovedInSameBatch.empty(); },
                             10000, "same-batch push/removal observed by second survivor"));

    const quint32 spellOid = p1.pushedAndRemovedInSameBatch.back();
    EXPECT_EQ(p3.pushedAndRemovedInSameBatch.back(), spellOid);
    EXPECT_EQ(p1.lifeByPlayer[p2.myId], 0);
    EXPECT_EQ(p3.lifeByPlayer[p2.myId], 0);
    EXPECT_EQ(p1.counteredStackObjectIds.count(spellOid), 0u)
        << "a player who paid life to zero loses after the cast; the engine does not counter the spell";
    EXPECT_EQ(p3.counteredStackObjectIds.count(spellOid), 0u);
    const int canonicalStackPlayer = std::min({p1.myId, p2.myId, p3.myId});
    EXPECT_NE(std::find(p1.destroyedStackCards.begin(), p1.destroyedStackCards.end(),
                        std::make_pair(canonicalStackPlayer, physicalSpellId)),
              p1.destroyedStackCards.end())
        << "same-batch removal must destroy the exact committed physical spell";
    EXPECT_NE(std::find(p3.destroyedStackCards.begin(), p3.destroyedStackCards.end(),
                        std::make_pair(canonicalStackPlayer, physicalSpellId)),
              p3.destroyedStackCards.end());
    EXPECT_EQ(p1.stackDepth, 0);
    EXPECT_EQ(p3.stackDepth, 0);
    EXPECT_TRUE(p1.gameStarted);
    EXPECT_TRUE(p3.gameStarted);
}

TEST_F(RuledE2ESmokeTest, FourClientsOpenSplitCombatAndDepart)
{
    const auto started = startServers();
    if (!started)
        FAIL() << started.message();
    if (const std::string message = started.message(); message.rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << message.substr(5);

    DepartureObserver p1(QStringLiteral("fourp1"), &transcript);
    SmokeClient p2(QStringLiteral("fourp2"), &transcript);
    SmokeClient p3(QStringLiteral("fourp3"), &transcript);
    SmokeClient p4(QStringLiteral("fourp4"), &transcript);
    std::array<SmokeClient *, 4> clients{&p1, &p2, &p3, &p4};
    for (auto *client : clients)
        ASSERT_TRUE(client->loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame(4));
    for (auto *client : std::array<SmokeClient *, 3>{&p2, &p3, &p4})
        ASSERT_TRUE(client->joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p3.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p4.selectDeck(deckXml({{40, QStringLiteral("Plains")}})));
    for (auto *client : clients)
        client->sendReady();

    QElapsedTimer deadline;
    deadline.start();
    std::map<int, quint64> actedAtVersion;
    int keeps = 0;
    bool choseFirst = false;
    const auto allAtMain = [&] {
        return std::all_of(clients.begin(), clients.end(), [&](const auto *client) {
            return client->gameStarted && client->stateVersion > 0 &&
                   client->phase == ruled::v1::PHASE_ID_MAIN1 && client->activePlayer == p1.myId;
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
            } else if (opening.stage() == ruled::v1::OPENING_STAGE_MULLIGAN && opening.can_keep()) {
                command.mutable_mulligan()->set_keep(true);
                ++keeps;
            } else if (client->priorityPlayer == client->myId && client->phase != ruled::v1::PHASE_ID_MAIN1 &&
                       opening.stage() != ruled::v1::OPENING_STAGE_CHOOSE_STARTING_PLAYER &&
                       opening.stage() != ruled::v1::OPENING_STAGE_MULLIGAN) {
                command.mutable_pass_priority();
            } else {
                continue;
            }
            client->sendRuled(command, QStringLiteral("advance four-player opening"));
            actedAtVersion[client->myId] = client->stateVersion;
        }
    }
    ASSERT_TRUE(allAtMain());
    EXPECT_TRUE(choseFirst);
    EXPECT_EQ(keeps, 4);
    for (auto *client : clients) {
        EXPECT_EQ(client->gameId, p1.gameId);
        EXPECT_EQ(client->handSizeByPlayer[client->myId], client == &p1 ? 8 : 7);
    }

    auto sendAndSettle = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command, const QString &label) {
        const quint64 commandId = sender.nextCmdId;
        const std::array<quint64, 4> before{p1.stateVersion, p2.stateVersion, p3.stateVersion, p4.stateVersion};
        sender.sendRuled(command, label);
        if (!sender.pumpUntil([&] { return sender.responses.count(commandId) > 0; }, 10000, "four-seat response"))
            return false;
        if (sender.responses[commandId].response_code() != Response::RespOk)
            return false;
        QElapsedTimer sync;
        sync.start();
        while (sync.elapsed() < 10000) {
            for (auto *client : clients)
                client->pump(20);
            if (p1.stateVersion > before[0] && p2.stateVersion > before[1] &&
                p3.stateVersion > before[2] && p4.stateVersion > before[3])
                return true;
        }
        return false;
    };
    auto putReady = [&](int controller) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(controller);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name("Grizzly Bears");
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(true);
        return sendAndSettle(p1, command, QStringLiteral("place four-seat combat creature"));
    };
    for (int i = 0; i < 3; ++i)
        ASSERT_TRUE(putReady(p1.myId));
    ASSERT_TRUE(putReady(p2.myId));
    const auto &attackers = p1.battlefieldByPlayer[p1.myId];
    ASSERT_GE(attackers.size(), 3u);
    const auto &blockers = p1.battlefieldByPlayer[p2.myId];
    ASSERT_FALSE(blockers.empty());
    auto pass = [&] {
        SmokeClient *actor = nullptr;
        for (auto *client : clients)
            if (client->myId == p1.priorityPlayer)
                actor = client;
        if (actor == nullptr)
            return false;
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndSettle(*actor, command, QStringLiteral("advance four-seat combat"));
    };
    deadline.restart();
    while (p1.phase != ruled::v1::PHASE_ID_DECLARE_ATTACKERS && deadline.elapsed() < 20000)
        ASSERT_TRUE(pass());
    ruled::v1::RuledCommand attack;
    const std::array<int, 3> defenderIds{p2.myId, p3.myId, p4.myId};
    for (int i = 0; i < 3; ++i)
        for (const auto &assignment : p1.latestLegal.legal_attack_assignments())
            if (assignment.attacker_object_id() == attackers[i].oid &&
                assignment.defending_player_id() == defenderIds[i])
                *attack.mutable_declare_attackers()->add_assignments() = assignment;
    ASSERT_EQ(attack.declare_attackers().assignments_size(), 3);
    ASSERT_TRUE(sendAndSettle(p1, attack, QStringLiteral("attack all three defenders")));
    deadline.restart();
    while (p1.phase != ruled::v1::PHASE_ID_DECLARE_BLOCKERS && deadline.elapsed() < 20000)
        ASSERT_TRUE(pass());
    ASSERT_EQ(p1.priorityPlayer, p2.myId);
    ruled::v1::RuledCommand block;
    for (const auto &pair : p2.latestLegal.legal_block_pairs())
        if (pair.blocker_id() == blockers.back().oid && pair.attacker_id() == attackers[0].oid)
            *block.mutable_declare_blockers()->add_block_pairs() = pair;
    ASSERT_EQ(block.declare_blockers().block_pairs_size(), 1);
    ASSERT_TRUE(sendAndSettle(p2, block, QStringLiteral("first defender blocks")));
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    EXPECT_EQ(p3.latestLegal.legal_block_pairs_size(), 0);
    EXPECT_EQ(p4.latestLegal.legal_block_pairs_size(), 0);
    deadline.restart();
    while (p1.phase != ruled::v1::PHASE_ID_MAIN2 && deadline.elapsed() < 20000)
        ASSERT_TRUE(pass());
    EXPECT_EQ(p1.lifeByPlayer.count(p2.myId), 0u);
    EXPECT_EQ(p1.lifeByPlayer[p3.myId], 18);
    EXPECT_EQ(p1.lifeByPlayer[p4.myId], 18);

    p1.expectedConceder = p2.myId;
    CommandContainer concede;
    concede.set_game_id(p2.gameId);
    concede.add_game_command()->MutableExtension(Command_Concede::ext);
    p2.sendContainer(concede);
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.sawEngineConcession; }, 10000, "first four-seat departure"));
    EXPECT_TRUE(p1.gameStarted);
    p1.expectedConceder = p3.myId;
    p1.sawEngineConcession = false;
    CommandContainer leave;
    leave.set_game_id(p3.gameId);
    leave.add_game_command()->MutableExtension(Command_LeaveGame::ext);
    p3.sendContainer(leave);
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.sawEngineConcession; }, 10000, "second four-seat departure"));
    EXPECT_TRUE(p1.gameStarted);
    p1.expectedConceder = p4.myId;
    p1.expectedWinner = p1.myId;
    p1.sawEngineConcession = false;
    CommandContainer finalConcede;
    finalConcede.set_game_id(p4.gameId);
    finalConcede.add_game_command()->MutableExtension(Command_Concede::ext);
    p4.sendContainer(finalConcede);
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.sawEngineConcession && p1.sawEngineWinner; }, 10000,
                             "four-seat final winner"));
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
