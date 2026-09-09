#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
class StormDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    std::vector<ruled::v1::StackPushed> stackPushes;
    int priorityEventCount = 0;

    void onRuledEvent(const ruled::v1::RuledEvent &event) override
    {
        OpeningDriver::onRuledEvent(event);
        if (event.has_stack_pushed()) {
            stackPushes.push_back(event.stack_pushed());
        }
        if (event.has_priority_changed()) {
            ++priorityEventCount;
        }
    }
};

std::optional<ObservedState::Permanent> permanent(const StormDriver &client, int controller, const QString &cardId)
{
    const auto battlefield = client.battlefieldByPlayer.find(controller);
    if (battlefield == client.battlefieldByPlayer.end()) {
        return std::nullopt;
    }
    const auto found = std::find_if(battlefield->second.begin(), battlefield->second.end(),
                                    [&](const auto &object) { return object.cardId == cardId; });
    return found == battlefield->second.end() ? std::nullopt : std::optional(*found);
}
} // namespace

TEST_F(RuledE2ESmokeTest, RalStormCopiesResumeTargetQueueAndPublishStableStackIdentity)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    StormDriver p1(true, QStringLiteral("stormp1"), &transcript);
    StormDriver p2(false, QStringLiteral("stormp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    const auto deck1 = deckXml({{40, QStringLiteral("Island")}});
    const auto deck2 = deckXml({{40, QStringLiteral("Forest")}});
    ASSERT_TRUE(p1.selectDeck(deck1));
    ASSERT_TRUE(p2.selectDeck(deck2));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "storm start p1"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "storm start p2"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());
    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);

    auto send = [&](StormDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const quint64 before1 = p1.stateVersion;
        const quint64 before2 = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto pass = [&](StormDriver &sender) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(sender, command, QStringLiteral("issue 238 pass"));
    };
    auto put = [&](int player, const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(player);
        auto *placement = dev->mutable_put_card_in_zone();
        placement->set_card_name(name);
        placement->set_zone(zone);
        placement->set_ready(true);
        return send(p1, command, QStringLiteral("issue 238 put %1").arg(name));
    };
    auto cast = [&](const QString &name, quint32 target = 0) {
        const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, name);
        if (!action) {
            return false;
        }
        ruled::v1::RuledCommand command;
        auto *spell = command.mutable_cast_spell();
        spell->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        spell->mutable_source()->set_hand_index(action->hand_index());
        if (target != 0) {
            const auto published = p1.latestLegal.valid_targets_by_hand_slot().find(action->hand_index() << 8);
            if (published == p1.latestLegal.valid_targets_by_hand_slot().end() || published->second.groups_size() != 1) {
                return false;
            }
            const auto &group = published->second.groups(0);
            if (std::find(group.valid_permanent_ids().begin(), group.valid_permanent_ids().end(), target) ==
                group.valid_permanent_ids().end()) {
                return false;
            }
            auto *chosen = spell->add_targets();
            chosen->set_object_id(target);
            chosen->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
            chosen->set_group_index(group.group_index());
        }
        return send(p1, command, QStringLiteral("issue 238 cast %1").arg(name));
    };

    ASSERT_TRUE(put(p1.myId, "Ral, Crackling Wit", ruled::v1::DEV_ZONE_BATTLEFIELD));
    auto ral = permanent(p1, p1.myId, QStringLiteral("ral,_crackling_wit"));
    ASSERT_TRUE(ral.has_value());
    EXPECT_EQ(ral->loyalty, 4);
    EXPECT_EQ(ral->abilityIndices, std::vector<quint32>({0, 1, 2}));

    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_u(30);
    ASSERT_TRUE(send(p1, mana, QStringLiteral("issue 238 add mana")));
    for (int castNumber = 0; castNumber < 6; ++castNumber) {
        ASSERT_TRUE(put(p1.myId, "Divination", ruled::v1::DEV_ZONE_HAND));
        ASSERT_TRUE(cast(QStringLiteral("Divination")));
        ASSERT_EQ(p1.stackDepth, 2);
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        ASSERT_EQ(p1.stackDepth, 0);
    }
    ral = permanent(p1, p1.myId, QStringLiteral("ral,_crackling_wit"));
    ASSERT_TRUE(ral.has_value());
    ASSERT_EQ(ral->loyalty, 10);

    ruled::v1::RuledCommand ultimate;
    p1.setBattlefieldAbilitySource(ultimate.mutable_activate_ability(), ral->oid);
    ultimate.mutable_activate_ability()->set_ability_index(2);
    ASSERT_TRUE(send(p1, ultimate, QStringLiteral("issue 238 activate Ral -10")));
    ASSERT_FALSE(permanent(p1, p1.myId, QStringLiteral("ral,_crackling_wit")).has_value());
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    ASSERT_EQ(p1.stackDepth, 0);
    const auto hasRalEmblem = [](const StormDriver &client) {
        return std::any_of(client.physicalCreateTokenEvents.begin(), client.physicalCreateTokenEvents.end(),
                           [](const Event_CreateToken &event) {
                               return event.zone_name() == ZoneNames::TABLE && event.annotation() == "Emblem" &&
                                      event.card_name() == "Ral, Crackling Wit Emblem";
                           });
    };
    EXPECT_TRUE(hasRalEmblem(p1));
    EXPECT_TRUE(hasRalEmblem(p2));

    ASSERT_TRUE(put(p2.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put(p2.myId, "Hill Giant", ruled::v1::DEV_ZONE_BATTLEFIELD));
    const auto bear = permanent(p1, p2.myId, QStringLiteral("grizzly_bears"));
    const auto giant = permanent(p1, p2.myId, QStringLiteral("hill_giant"));
    ASSERT_TRUE(bear.has_value() && giant.has_value());
    ASSERT_TRUE(put(p1.myId, "Unsummon", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(cast(QStringLiteral("Unsummon"), bear->oid));

    const auto original = std::find_if(p1.stackPushes.rbegin(), p1.stackPushes.rend(), [](const auto &pushed) {
        return pushed.card_id() == "unsummon" && !pushed.is_copy() && !pushed.is_triggered();
    });
    ASSERT_NE(original, p1.stackPushes.rend());
    const quint32 originalOid = original->object_id();
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    ASSERT_TRUE(p1.pendingChoice.has_value());
    ASSERT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_TARGET_OBJECTS);
    ASSERT_EQ(p1.pendingChoice->min(), 1u);
    ASSERT_EQ(p1.pendingChoice->max(), 1u);
    ASSERT_TRUE(p2.lastResolutionChoice.has_value());
    EXPECT_FALSE(p2.pendingChoice.has_value());
    EXPECT_EQ(p2.lastResolutionChoice->SerializeAsString(), p1.pendingChoice->SerializeAsString())
        << "public battlefield target candidates stay observable while only the decider can answer";

    const quint32 firstPendingCopy = p1.pendingChoice->source_object_id();
    const int priorityEventsBeforeChoice = p1.priorityEventCount;
    ruled::v1::RuledCommand firstChoice;
    firstChoice.mutable_submit_resolution_choice()->add_chosen_object_ids(bear->oid);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, firstChoice, QStringLiteral("issue 238 first copy target")));
    ASSERT_TRUE(p1.pendingChoice.has_value());
    EXPECT_NE(p1.pendingChoice->source_object_id(), firstPendingCopy);
    EXPECT_EQ(p1.priorityEventCount, priorityEventsBeforeChoice)
        << "priority must remain blocked while another storm copy needs targets";

    const auto firstCopy = std::find_if(p1.stackPushes.rbegin(), p1.stackPushes.rend(),
                                        [&](const auto &pushed) { return pushed.object_id() == firstPendingCopy; });
    ASSERT_NE(firstCopy, p1.stackPushes.rend());
    ASSERT_TRUE(firstCopy->is_copy());
    EXPECT_EQ(firstCopy->copy_source_object_id(), originalOid);
    ASSERT_EQ(firstCopy->targets_size(), 1);
    EXPECT_EQ(firstCopy->targets(0).object_id(), bear->oid);
    const auto observerCopy = std::find_if(p2.stackPushes.rbegin(), p2.stackPushes.rend(),
                                           [&](const auto &pushed) { return pushed.object_id() == firstPendingCopy; });
    ASSERT_NE(observerCopy, p2.stackPushes.rend());
    EXPECT_EQ(firstCopy->SerializeAsString(), observerCopy->SerializeAsString());

    const quint32 secondPendingCopy = p1.pendingChoice->source_object_id();
    const auto expectedChoice = p1.pendingChoice->SerializeAsString();
    const quint64 expectedVersion = p1.stateVersion;

    collectServerLogs();
    servatrice.kill();
    ASSERT_TRUE(servatrice.waitForFinished(5000));
    sidecar.kill();
    ASSERT_TRUE(sidecar.waitForFinished(5000));
    p1.sock.abort();
    p2.sock.abort();

    QString capture;
    QDirIterator manifests(tempDir.filePath("captures"), {"manifest.json"}, QDir::Files,
                           QDirIterator::Subdirectories);
    while (manifests.hasNext()) {
        QFile file(manifests.next());
        ASSERT_TRUE(file.open(QIODevice::ReadOnly));
        if (QJsonDocument::fromJson(file.readAll()).object().value("source") == "server") {
            ASSERT_TRUE(capture.isEmpty());
            capture = QFileInfo(file).absolutePath();
        }
    }
    ASSERT_FALSE(capture.isEmpty());
    const auto planDir = tempDir.filePath("storm-resume");
    QProcess replay;
    replay.start(QStringLiteral(RULED_E2E_REPLAY_PATH),
                 {"--capture", capture, "--resume-plan", "--output", planDir});
    ASSERT_TRUE(replay.waitForStarted(10000));
    ASSERT_TRUE(replay.waitForFinished(60000));
    ASSERT_EQ(replay.exitStatus(), QProcess::NormalExit) << replay.readAllStandardError().constData();
    ASSERT_EQ(replay.exitCode(), 0) << replay.readAllStandardError().constData();
    resumePlanPath = planDir + "/resume-plan.pb";

    ASSERT_TRUE(startServers());
    StormDriver resumed1(true, QStringLiteral("storm-resume1"), &transcript);
    StormDriver resumed2(false, QStringLiteral("storm-resume2"), &transcript);
    ASSERT_TRUE(resumed1.loginAndJoinRoom());
    ASSERT_TRUE(resumed2.loginAndJoinRoom());
    ASSERT_TRUE(resumed1.createRuledGame());
    ASSERT_TRUE(resumed2.joinRuledGame(resumed1.gameId));
    ASSERT_TRUE(resumed1.selectDeck(deck1));
    ASSERT_TRUE(resumed2.selectDeck(deck2));
    resumed1.sendReady();
    resumed2.sendReady();
    ASSERT_TRUE(resumed1.pumpUntil([&] { return resumed1.stateVersion == expectedVersion; }, 20000,
                                      "resumed storm chooser state"));
    ASSERT_TRUE(resumed2.pumpUntil([&] { return resumed2.stateVersion == expectedVersion; }, 20000,
                                      "resumed storm observer state"));
    ASSERT_TRUE(resumed1.pendingChoice.has_value());
    EXPECT_EQ(resumed1.pendingChoice->SerializeAsString(), expectedChoice);
    EXPECT_EQ(resumed1.pendingChoice->source_object_id(), secondPendingCopy);
    ASSERT_TRUE(resumed2.lastResolutionChoice.has_value());
    EXPECT_FALSE(resumed2.pendingChoice.has_value());
    EXPECT_EQ(resumed2.lastResolutionChoice->SerializeAsString(), expectedChoice);

    auto sendResumed = [&](StormDriver &sender, const ruled::v1::RuledCommand &command) {
        const quint64 before1 = resumed1.stateVersion;
        const quint64 before2 = resumed2.stateVersion;
        sender.sendRuled(command, QStringLiteral("issue 238 resumed target"));
        QElapsedTimer wait;
        wait.start();
        while ((resumed1.stateVersion <= before1 || resumed2.stateVersion <= before2) && wait.elapsed() < 10000) {
            resumed1.pump(25);
            resumed2.pump(25);
        }
        return resumed1.stateVersion > before1 && resumed2.stateVersion > before2;
    };
    ruled::v1::RuledCommand secondChoice;
    secondChoice.mutable_submit_resolution_choice()->add_chosen_object_ids(giant->oid);
    resumed1.pendingChoice.reset();
    ASSERT_TRUE(sendResumed(resumed1, secondChoice));
    ASSERT_TRUE(resumed1.pendingChoice.has_value());
    EXPECT_NE(resumed1.pendingChoice->source_object_id(), secondPendingCopy);
    const auto secondCopy = std::find_if(resumed1.stackPushes.rbegin(), resumed1.stackPushes.rend(),
                                         [&](const auto &pushed) { return pushed.object_id() == secondPendingCopy; });
    ASSERT_NE(secondCopy, resumed1.stackPushes.rend());
    EXPECT_NE(secondCopy->object_id(), firstCopy->object_id());
    EXPECT_EQ(secondCopy->copy_source_object_id(), originalOid);
    ASSERT_EQ(secondCopy->targets_size(), 1);
    EXPECT_EQ(secondCopy->targets(0).object_id(), giant->oid);
}
} // namespace ruled_e2e
