#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

#include <QFileInfo>
#include <memory>
#include <libcockatrice/protocol/pb/event_create_counter.pb.h>
#include <libcockatrice/protocol/pb/event_set_counter.pb.h>

namespace ruled_e2e
{
namespace
{
class AjaniDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    std::optional<ruled::v1::TriggerNeedsTarget> currentTrigger;
    std::map<int, std::vector<quint32>> emblemIds;
    std::map<int, int> snapshotEmblems;
    std::map<int, std::map<std::string, int>> physicalCounters;
    std::map<std::pair<int, int>, std::string> counterNames;
    std::optional<ruled::v1::StackPushed> latestAbility;

    void onBatchBegin(const ruled::v1::RuledEventBatch &batch) override
    {
        if (batch.replace_stack_presentation() ||
            std::any_of(batch.events().begin(), batch.events().end(), [](const auto &event) { return event.has_zone_view(); }))
            currentTrigger.reset();
    }
    void onRuledEvent(const ruled::v1::RuledEvent &event) override
    {
        OpeningDriver::onRuledEvent(event);
        if (event.has_trigger_needs_target())
            currentTrigger = event.trigger_needs_target();
        if (event.has_stack_pushed() && !event.stack_pushed().ability_annotation().empty())
            latestAbility = event.stack_pushed();
        if (event.has_zone_view()) {
            for (const auto &player : event.zone_view().per_player()) {
                auto &markers = emblemIds[player.player_id()];
                markers.clear();
                for (const auto &marker : player.static_emblems()) {
                    EXPECT_EQ(marker.display_name(), "Ajani, Sleeper Agent Emblem");
                    markers.push_back(marker.object_id());
                }
            }
        }
    }
    void onPhysicalEvent(const GameEvent &event) override
    {
        if (event.HasExtension(Event_CreateCounter::ext)) {
            const auto &counter = event.GetExtension(Event_CreateCounter::ext).counter_info();
            counterNames[{event.player_id(), counter.id()}] = counter.name();
            physicalCounters[event.player_id()][counter.name()] = counter.count();
        }
        if (event.HasExtension(Event_SetCounter::ext)) {
            const auto &counter = event.GetExtension(Event_SetCounter::ext);
            const auto name = counterNames.find({event.player_id(), counter.counter_id()});
            if (name != counterNames.end())
                physicalCounters[event.player_id()][name->second] = counter.value();
        }
        if (event.HasExtension(Event_GameStateChanged::ext)) {
            for (const auto &player : event.GetExtension(Event_GameStateChanged::ext).player_list()) {
                const int owner = player.properties().player_id();
                snapshotEmblems[owner] = 0;
                for (const auto &zone : player.zone_list()) {
                    if (zone.name() == ZoneNames::TABLE) {
                        for (const auto &card : zone.card_list()) {
                            if (card.name() == "Ajani, Sleeper Agent Emblem" && card.annotation() == "Emblem")
                                ++snapshotEmblems[owner];
                        }
                    }
                }
                for (const auto &counter : player.counter_list()) {
                    counterNames[{owner, counter.id()}] = counter.name();
                    physicalCounters[owner][counter.name()] = counter.count();
                }
            }
        }
    }
};

// Prepared actual-card transport through real Servatrice/sidecar and two independent recipients.
// Capture/resume restores the same seats to fresh clients; this is not live TCP reconnect or GUI.
TEST_F(RuledE2ESmokeTest, ActualAjaniEmblemTargetAndPoisonSurviveSameSeatCaptureResume)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << std::string(started.message()).substr(5);
    auto first = std::make_unique<AjaniDriver>(true, QStringLiteral("ajanip1"), &transcript);
    auto second = std::make_unique<AjaniDriver>(false, QStringLiteral("ajanip2"), &transcript);
    const auto deck1 = deckXml({{40, QStringLiteral("Island")}});
    const auto deck2 = deckXml({{40, QStringLiteral("Forest")}});
    const auto join = [&]() -> ::testing::AssertionResult {
        if (!first->loginAndJoinRoom() || !second->loginAndJoinRoom() || !first->createRuledGame() ||
            !second->joinRuledGame(first->gameId) || !first->selectDeck(deck1) || !second->selectDeck(deck2))
            return ::testing::AssertionFailure() << "Ajani two-seat setup failed";
        first->sendReady();
        second->sendReady();
        return ::testing::AssertionSuccess();
    };
    ASSERT_TRUE(join());
    const auto wait = [&](const std::function<bool()> &done, const char *description) -> ::testing::AssertionResult {
        QElapsedTimer timer;
        timer.start();
        while (!done() && timer.elapsed() < 20000) {
            first->pump(20);
            second->pump(20);
        }
        if (!done())
            return ::testing::AssertionFailure() << description;
        return ::testing::AssertionSuccess();
    };
    ASSERT_TRUE(wait([&] { return first->gameStarted && second->gameStarted; }, "Ajani startup"));
    ASSERT_TRUE(first->publishMain1Stops());
    ASSERT_TRUE(second->publishMain1Stops());
    QElapsedTimer opening;
    opening.start();
    while ((first->phase != ruled::v1::PHASE_ID_MAIN1 || second->phase != ruled::v1::PHASE_ID_MAIN1 ||
            first->priorityPlayer != first->myId || second->priorityPlayer != first->myId) && opening.elapsed() < 30000) {
        first->pump(20);
        second->pump(20);
        first->act();
        second->act();
    }
    ASSERT_EQ(first->phase, ruled::v1::PHASE_ID_MAIN1);
    const int actor = first->myId;
    const int opponent = second->myId;
    const auto send = [&](AjaniDriver &sender, const ruled::v1::RuledCommand &command, const char *description) {
        const auto version1 = first->stateVersion;
        const auto version2 = second->stateVersion;
        sender.sendRuled(command, QString::fromUtf8(description));
        return wait([&] { return first->stateVersion > version1 && second->stateVersion > version2; }, description);
    };
    const auto put = [&](const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(actor);
        auto *object = dev->mutable_put_card_in_zone();
        object->set_card_name(name);
        object->set_zone(zone);
        object->set_ready(true);
        return send(*first, command, "Ajani prepared card");
    };
    const auto mana = [&](int c, int g, int w) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(actor);
        dev->mutable_add_mana()->set_c(c);
        dev->mutable_add_mana()->set_g(g);
        dev->mutable_add_mana()->set_w(w);
        return send(*first, command, "Ajani prepared mana");
    };
    const auto pass = [&](AjaniDriver &sender) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(sender, command, "Ajani priority pass");
    };
    const auto cast = [&](const char *name, bool life) {
        const auto *offer = first->handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QString::fromUtf8(name));
        if (!offer)
            return ::testing::AssertionFailure() << "missing offered " << name;
        ruled::v1::RuledCommand command;
        auto *spell = command.mutable_cast_spell();
        spell->mutable_source()->set_hand_index(offer->hand_index());
        spell->set_cast_method(offer->cast_method());
        if (life) {
            auto *choice = spell->add_flex_payments();
            choice->set_pip_index(2u);
            choice->set_pay_life(true);
        }
        if (!send(*first, command, "Ajani actual cast"))
            return ::testing::AssertionFailure() << "actual cast submission";
        return wait([&] { return !first->latestLegal.has_pending_spell_cast() && first->stackDepth > 0; }, "actual cast commit");
    };
    ASSERT_TRUE(put("Doubling Season", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put("Ajani, Sleeper Agent", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(mana(1, 1, 1));
    ASSERT_TRUE(cast("Ajani, Sleeper Agent", true));
    ASSERT_TRUE(pass(*first));
    ASSERT_TRUE(pass(*second));
    ASSERT_TRUE(first->pendingChoice);
    ASSERT_EQ(first->pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT);
    const auto &options = first->pendingChoice->replacement_options();
    const auto season = std::find_if(options.begin(), options.end(), [](const auto &option) {
        return option.source_card_name() == "Doubling Season";
    });
    ASSERT_NE(season, options.end());
    ruled::v1::RuledCommand replacement;
    replacement.mutable_submit_resolution_choice()->set_resolution_prompt_id(first->currentResolutionPromptId);
    replacement.mutable_submit_resolution_choice()->add_chosen_object_ids(season->application_id());
    ASSERT_TRUE(send(*first, replacement, "Ajani Season before Compleated"));
    const auto &board = first->battlefieldByPlayer[actor];
    const auto walker = std::find_if(board.begin(), board.end(), [](const auto &object) {
        return object.cardId == QStringLiteral("ajani,_sleeper_agent");
    });
    ASSERT_NE(walker, board.end());
    ASSERT_EQ(walker->loyalty, 6);
    const quint32 source = walker->oid;
    EXPECT_EQ(first->lifeByPlayer[actor], 18);
    EXPECT_EQ(second->lifeByPlayer[actor], 18);
    ruled::v1::RuledCommand ultimate;
    first->setBattlefieldAbilitySource(ultimate.mutable_activate_ability(), source);
    ultimate.mutable_activate_ability()->set_ability_index(2u);
    ASSERT_TRUE(send(*first, ultimate, "Ajani actual -6"));
    ASSERT_TRUE(pass(*first));
    ASSERT_TRUE(pass(*second));
    for (const auto *recipient : {first.get(), second.get()}) {
        EXPECT_EQ(recipient->graveyardOwnerByEngineOid.at(source), actor);
        ASSERT_EQ(recipient->emblemIds.at(actor).size(), 1u);
        EXPECT_TRUE(std::any_of(recipient->physicalCreateTokenEvents.begin(), recipient->physicalCreateTokenEvents.end(),
                               [](const auto &token) {
                                   return token.card_name() == "Ajani, Sleeper Agent Emblem" && token.annotation() == "Emblem";
                               }));
    }
    const quint32 marker = first->emblemIds[actor][0];
    ASSERT_TRUE(put("Grizzly Bears", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(mana(1, 1, 0));
    ASSERT_TRUE(cast("Grizzly Bears", false));
    const auto expectOffer = [&]() {
        for (const auto *recipient : {first.get(), second.get()}) {
            ASSERT_TRUE(recipient->currentTrigger);
            const auto &offer = *recipient->currentTrigger;
            EXPECT_EQ(offer.source_permanent_id(), marker);
            EXPECT_EQ(offer.controller_player_id(), actor);
            EXPECT_EQ(offer.has_targets(), recipient == first.get());
            EXPECT_EQ(recipient->emblemIds.at(actor), std::vector<quint32>{marker});
        }
        const auto &targets = first->currentTrigger->targets();
        ASSERT_EQ(targets.groups_size(), 1);
        EXPECT_TRUE(targets.groups(0).can_target_opponent());
        EXPECT_FALSE(targets.groups(0).can_target_self());
        EXPECT_EQ(targets.groups(0).valid_permanent_ids_size(), 0);
    };
    ASSERT_NO_FATAL_FAILURE(expectOffer());
    std::set<QString> retainedCaptures;
    int resumeNumber = 0;
    const auto resume = [&]() -> ::testing::AssertionResult {
        const auto version1 = first->stateVersion;
        const auto version2 = second->stateVersion;
        collectServerLogs();
        servatrice.kill();
        if (!servatrice.waitForFinished(5000))
            return ::testing::AssertionFailure() << "relay stop";
        sidecar.kill();
        if (!sidecar.waitForFinished(5000))
            return ::testing::AssertionFailure() << "sidecar stop";
        first->sock.abort();
        second->sock.abort();
        QString capture;
        QDirIterator manifests(tempDir.filePath("captures"), {"manifest.json"}, QDir::Files, QDirIterator::Subdirectories);
        while (manifests.hasNext()) {
            QFile file(manifests.next());
            if (!file.open(QIODevice::ReadOnly))
                return ::testing::AssertionFailure() << "capture manifest read";
            const auto path = QFileInfo(file).absolutePath();
            if (QJsonDocument::fromJson(file.readAll()).object().value("source") == "server" && !retainedCaptures.count(path)) {
                if (!capture.isEmpty())
                    return ::testing::AssertionFailure() << "ambiguous fresh server captures";
                capture = path;
            }
        }
        if (capture.isEmpty())
            return ::testing::AssertionFailure() << "no fresh server capture";
        retainedCaptures.insert(capture);
        const auto plan = tempDir.filePath(QStringLiteral("ajani-resume-%1").arg(++resumeNumber));
        QProcess replay;
        replay.start(QStringLiteral(RULED_E2E_REPLAY_PATH), {"--capture", capture, "--resume-plan", "--output", plan});
        if (!replay.waitForStarted(10000) || !replay.waitForFinished(60000) ||
            replay.exitStatus() != QProcess::NormalExit || replay.exitCode() != 0)
            return ::testing::AssertionFailure() << "resume plan: " << replay.readAllStandardError().constData();
        resumePlanPath = plan + "/resume-plan.pb";
        const auto restarted = startServers();
        if (!restarted)
            return restarted;
        first = std::make_unique<AjaniDriver>(true, QStringLiteral("ajaniresume1-%1").arg(resumeNumber), &transcript);
        second = std::make_unique<AjaniDriver>(false, QStringLiteral("ajaniresume2-%1").arg(resumeNumber), &transcript);
        if (!join())
            return ::testing::AssertionFailure() << "resumed join";
        if (!wait([&] {
                return first->stateVersion == version1 && second->stateVersion == version2 &&
                       first->snapshotEmblems[actor] == 1 && second->snapshotEmblems[actor] == 1 &&
                       (resumeNumber == 1 || (first->physicalCounters[opponent]["poison"] == 2 &&
                                              second->physicalCounters[opponent]["poison"] == 2));
            }, "captured Ajani versions and physical snapshot"))
            return ::testing::AssertionFailure() << "captured versions";
        if (first->myId != actor || second->myId != opponent)
            return ::testing::AssertionFailure() << "seat identity changed";
        return ::testing::AssertionSuccess();
    };
    ASSERT_TRUE(resume());
    ASSERT_NO_FATAL_FAILURE(expectOffer());
    for (const auto *recipient : {first.get(), second.get()})
        EXPECT_EQ(recipient->snapshotEmblems.at(actor), 1);
    ruled::v1::RuledCommand choose;
    auto *target = choose.mutable_choose_trigger_target()->add_targets();
    target->set_object_id(opponent);
    target->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
    target->set_group_index(first->currentTrigger->targets().groups(0).group_index());
    ASSERT_TRUE(send(*first, choose, "Ajani chooses actual opponent"));
    for (const auto *recipient : {first.get(), second.get()}) {
        ASSERT_TRUE(recipient->latestAbility);
        EXPECT_FALSE(recipient->latestAbility->has_ability_source());
        ASSERT_EQ(recipient->latestAbility->targets_size(), 1);
        EXPECT_EQ(recipient->latestAbility->targets(0).object_id(), opponent);
        EXPECT_EQ(recipient->latestAbility->targets(0).kind(), ruled::v1::TARGET_REF_KIND_PLAYER);
        EXPECT_FALSE(recipient->currentTrigger);
    }
    ASSERT_TRUE(pass(*first));
    ASSERT_TRUE(pass(*second));
    for (const auto *recipient : {first.get(), second.get()}) {
        EXPECT_EQ(recipient->playerCountersByPlayer.at(opponent).at("poison"), 2);
        EXPECT_EQ(recipient->physicalCounters.at(opponent).at("poison"), 2);
    }
    ASSERT_TRUE(pass(*first));
    ASSERT_TRUE(pass(*second));
    ASSERT_EQ(first->stackDepth, 0);
    ASSERT_EQ(second->stackDepth, 0);
    ASSERT_TRUE(resume());
    for (const auto *recipient : {first.get(), second.get()}) {
        EXPECT_FALSE(recipient->currentTrigger);
        EXPECT_EQ(recipient->snapshotEmblems.at(actor), 1);
        EXPECT_EQ(recipient->emblemIds.at(actor), std::vector<quint32>{marker});
        EXPECT_EQ(recipient->playerCountersByPlayer.at(opponent).at("poison"), 2);
        EXPECT_EQ(recipient->physicalCounters.at(opponent).at("poison"), 2);
        EXPECT_EQ(recipient->stackDepth, 0);
    }
}
} // namespace
} // namespace ruled_e2e
