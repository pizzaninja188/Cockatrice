#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"
namespace ruled_e2e
{
namespace
{
class TemporaryExileDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    bool temporaryExileFlowActive = false;
    bool sawTemporaryExilePhysicalMove = false;
    bool sawTemporaryReturnPhysicalMove = false;
    int temporaryExilePhysicalCardId = -1;
    void onPhysicalEvent(const GameEvent &ev) override
    {
        if (ev.HasExtension(Event_MoveCard::ext)) {
            const auto &mc = ev.GetExtension(Event_MoveCard::ext);
            const QString from = QString::fromStdString(mc.start_zone());
            const QString to = QString::fromStdString(mc.target_zone());
            const QString name = QString::fromStdString(mc.card_name());
            // ZoneNames, not literals: Cockatrice's exile zone is spelled "rfg".

            const QLatin1String exile(ZoneNames::EXILE);
            const QLatin1String table(ZoneNames::TABLE);

            if (temporaryExileFlowActive && name == QLatin1String("Grizzly Bears") &&
                ((from == table && to == exile) || (from == exile && to == table))) {
                const bool identityContinuous = temporaryExilePhysicalCardId >= 0 &&
                                                mc.card_id() == temporaryExilePhysicalCardId &&
                                                mc.new_card_id() == temporaryExilePhysicalCardId;
                EXPECT_TRUE(identityContinuous) << "temporary exile moved a different physical Grizzly Bears card";
                sawTemporaryExilePhysicalMove = sawTemporaryExilePhysicalMove || (from == table && to == exile);
                sawTemporaryReturnPhysicalMove = sawTemporaryReturnPhysicalMove || (from == exile && to == table);
            }
        }
    }
};

TEST_F(RuledE2ESmokeTest, TemporaryExileReturnsTheExactPhysicalCardToBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    TemporaryExileDriver p1(true, QStringLiteral("exilep1"), &transcript);
    TemporaryExileDriver p2(false, QStringLiteral("exilep2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Plains")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "temporary-exile game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "temporary-exile game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer openingDeadline;
    openingDeadline.start();
    while (openingDeadline.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);

    auto sendAndPump = [&](TemporaryExileDriver &sender, const ruled::v1::RuledCommand &command,
                           const QString &description) {
        const quint64 v1 = p1.stateVersion;
        const quint64 v2 = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 && (p1.stateVersion <= v1 || p2.stateVersion <= v2)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > v1 && p2.stateVersion > v2;
    };
    auto devPut = [&](int playerId, const char *name) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(playerId);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(name);
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(true);
        return sendAndPump(p1, command, QStringLiteral("dev put %1").arg(name));
    };
    auto passPriority = [&](TemporaryExileDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("temporary-exile pass"));
    };
    auto findPermanent = [](const TemporaryExileDriver &client, const QString &cardId) -> quint32 {
        for (const auto &[playerId, permanents] : client.battlefieldByPlayer) {
            Q_UNUSED(playerId);
            for (const auto &permanent : permanents) {
                if (permanent.cardId == cardId) {
                    return permanent.oid;
                }
            }
        }
        return 0;
    };

    ASSERT_TRUE(devPut(p2.myId, "Grizzly Bears"));
    const quint32 bearOid = findPermanent(p1, QStringLiteral("grizzly_bears"));
    ASSERT_NE(bearOid, 0u);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(bearOid));
    const int physicalBearId = p1.serverCardByEngineOid[bearOid];
    ASSERT_TRUE(p2.serverCardByEngineOid.count(bearOid));
    ASSERT_EQ(p2.serverCardByEngineOid[bearOid], physicalBearId);
    p1.temporaryExileFlowActive = true;
    p2.temporaryExileFlowActive = true;
    p1.temporaryExilePhysicalCardId = physicalBearId;
    p2.temporaryExilePhysicalCardId = physicalBearId;
    ASSERT_TRUE(devPut(p1.myId, "Banishing Light"));
    const quint32 lightOid = findPermanent(p1, QStringLiteral("banishing_light"));
    ASSERT_NE(lightOid, 0u);
    ASSERT_TRUE(p1.pendingTriggerTarget.has_value());
    ruled::v1::RuledCommand chooseTarget;
    auto *choice = chooseTarget.mutable_choose_trigger_target();
    auto *target = choice->add_targets();
    target->set_object_id(bearOid);
    target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    p1.pendingTriggerTarget.reset();
    ASSERT_TRUE(sendAndPump(p1, chooseTarget, QStringLiteral("choose Banishing Light target")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_EQ(findPermanent(p1, QStringLiteral("grizzly_bears")), 0u);
    EXPECT_EQ(findPermanent(p2, QStringLiteral("grizzly_bears")), 0u);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(bearOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(bearOid));
    EXPECT_EQ(p1.serverCardByEngineOid[bearOid], physicalBearId);
    EXPECT_EQ(p2.serverCardByEngineOid[bearOid], physicalBearId);

    ruled::v1::RuledCommand removeLight;
    auto *dev = removeLight.mutable_dev_command();
    dev->set_target_player_id(p1.myId);
    auto *move = dev->mutable_move_card();
    move->set_card_name("Banishing Light");
    move->set_zone(ruled::v1::DEV_ZONE_GRAVEYARD);
    ASSERT_TRUE(sendAndPump(p1, removeLight, QStringLiteral("remove Banishing Light")));

    EXPECT_EQ(findPermanent(p1, QStringLiteral("grizzly_bears")), bearOid);
    EXPECT_EQ(findPermanent(p2, QStringLiteral("grizzly_bears")), bearOid);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(bearOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(bearOid));
    EXPECT_EQ(p1.serverCardByEngineOid[bearOid], physicalBearId);
    EXPECT_EQ(p2.serverCardByEngineOid[bearOid], physicalBearId);
    EXPECT_TRUE(p1.sawTemporaryExilePhysicalMove);
    EXPECT_TRUE(p2.sawTemporaryExilePhysicalMove);
    EXPECT_TRUE(p1.sawTemporaryReturnPhysicalMove);
    EXPECT_TRUE(p2.sawTemporaryReturnPhysicalMove);
}

TEST_F(RuledE2ESmokeTest, IcetillPlaysTheExactGenerationBoundGraveyardLand)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    OpeningDriver p1(true, QStringLiteral("icetillp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("icetillp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Icetill game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Icetill game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer openingDeadline;
    openingDeadline.start();
    while (openingDeadline.elapsed() < 30000) {
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

    auto sendAndPump = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
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

    ruled::v1::RuledCommand putIcetill;
    auto *put = putIcetill.mutable_dev_command();
    put->set_target_player_id(p1.myId);
    put->mutable_put_card_in_zone()->set_card_name("Icetill Explorer");
    put->mutable_put_card_in_zone()->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
    put->mutable_put_card_in_zone()->set_ready(true);
    ASSERT_TRUE(sendAndPump(p1, putIcetill, QStringLiteral("put Icetill Explorer onto the battlefield")));

    ruled::v1::RuledCommand moveForest;
    auto *move = moveForest.mutable_dev_command();
    move->set_target_player_id(p1.myId);
    move->mutable_move_card()->set_card_name("Forest");
    move->mutable_move_card()->set_zone(ruled::v1::DEV_ZONE_GRAVEYARD);
    ASSERT_TRUE(sendAndPump(p1, moveForest, QStringLiteral("move Forest to the graveyard")));

    const ruled::v1::LegalZoneLandAction *graveyardLand = nullptr;
    for (const auto &action : p1.latestLegal.zone_land_actions()) {
        if (action.source_zone() == ruled::v1::CAST_SOURCE_ZONE_GRAVEYARD && action.card_name() == "Forest") {
            graveyardLand = &action;
            break;
        }
    }
    ASSERT_NE(graveyardLand, nullptr);
    const quint32 forestOid = graveyardLand->object_id();
    ASSERT_GT(graveyardLand->zone_change_generation(), 0u);
    EXPECT_TRUE(p1.graveyardOwnerByEngineOid.count(forestOid));
    EXPECT_EQ(p1.graveyardOwnerByEngineOid[forestOid], p1.myId);
    EXPECT_TRUE(p2.graveyardOwnerByEngineOid.count(forestOid));
    EXPECT_TRUE(std::none_of(p2.latestLegal.zone_land_actions().cbegin(), p2.latestLegal.zone_land_actions().cend(),
                             [&](const auto &action) { return action.object_id() == forestOid; }));
    ASSERT_TRUE(p1.serverCardByEngineOid.count(forestOid));
    const int physicalForestId = p1.serverCardByEngineOid[forestOid];
    ASSERT_TRUE(p2.serverCardByEngineOid.count(forestOid));
    ASSERT_EQ(p2.serverCardByEngineOid[forestOid], physicalForestId);

    ruled::v1::RuledCommand playLand;
    auto *play = playLand.mutable_play_land();
    play->mutable_source()->set_graveyard_object_id(forestOid);
    play->mutable_source()->set_expected_zone_change_generation(graveyardLand->zone_change_generation());
    play->set_face_index(graveyardLand->face_index());
    ASSERT_TRUE(sendAndPump(p1, playLand, QStringLiteral("play the generation-bound graveyard Forest")));

    const auto hasForest = [&](const OpeningDriver &client) {
        const auto found = client.battlefieldByPlayer.find(p1.myId);
        return found != client.battlefieldByPlayer.end() &&
               std::any_of(found->second.cbegin(), found->second.cend(),
                           [&](const auto &permanent) { return permanent.oid == forestOid; });
    };
    EXPECT_TRUE(hasForest(p1));
    EXPECT_TRUE(hasForest(p2));
    EXPECT_EQ(p1.serverCardByEngineOid[forestOid], physicalForestId);
    EXPECT_EQ(p2.serverCardByEngineOid[forestOid], physicalForestId);
    EXPECT_EQ(p1.physicalRowAndPt[std::make_pair(p1.myId, physicalForestId)].first, 2);
    EXPECT_EQ(p2.physicalRowAndPt[std::make_pair(p1.myId, physicalForestId)].first, 2);
}

class PlayerSetDiscardDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    bool playerSetDiscardFlowActive = false;
    bool sawPlayerSetDiscardPrivateCandidates = false;
    bool sawPlayerSetDiscardObserverRedaction = false;
    quint32 playerSetDiscardChosenOid = 0;
    int playerSetDiscardChosenServerCardId = -1;
    void onRuledEvent(const ruled::v1::RuledEvent &ev) override
    {
        OpeningDriver::onRuledEvent(ev);
        if (ev.has_resolution_choice_required()) {
            const auto &rcr = ev.resolution_choice_required();
            if (playerSetDiscardFlowActive && rcr.choice_kind() == ruled::v1::CHOICE_KIND_HAND_CARDS) {
                if (rcr.deciding_player_id() == myId) {
                    int bearIndex = -1;
                    for (int i = 0; i < rcr.candidate_names_size(); ++i) {
                        if (rcr.candidate_names(i) == "Grizzly Bears") {
                            bearIndex = i;
                            break;
                        }
                    }
                    const bool aligned = rcr.candidate_object_ids_size() == rcr.candidate_card_ids_size() &&
                                         rcr.candidate_object_ids_size() == rcr.candidate_names_size() &&
                                         rcr.candidate_object_ids_size() == rcr.candidate_server_card_ids_size() &&
                                         rcr.candidate_object_ids_size() == rcr.candidate_selectable_size();
                    sawPlayerSetDiscardPrivateCandidates = aligned && rcr.min() == 1 && rcr.max() == 1 &&
                                                           bearIndex >= 0 && rcr.candidate_selectable(bearIndex);
                    if (sawPlayerSetDiscardPrivateCandidates) {
                        playerSetDiscardChosenOid = rcr.candidate_object_ids(bearIndex);
                        playerSetDiscardChosenServerCardId = rcr.candidate_server_card_ids(bearIndex);
                    }
                } else {
                    sawPlayerSetDiscardObserverRedaction =
                        rcr.candidate_object_ids_size() == 0 && rcr.candidate_card_ids_size() == 0 &&
                        rcr.candidate_names_size() == 0 && rcr.candidate_server_card_ids_size() == 0 &&
                        rcr.candidate_selectable_size() == 0 &&
                        rcr.prompt_text() == "Opponent is making a resolution choice.";
                }
            }
        }
    }
};

TEST_F(RuledE2ESmokeTest, PlayerSetDiscardCollectsPrivateChoicesBeforeOnePhysicalCommit)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    PlayerSetDiscardDriver p1(true, QStringLiteral("playersetp1"), &transcript);
    PlayerSetDiscardDriver p2(false, QStringLiteral("playersetp2"), &transcript);

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Swamp")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "player-set discard game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "player-set discard game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer openingDeadline;
    openingDeadline.start();
    while (openingDeadline.elapsed() < 30000) {
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
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    auto sendAndPump = [&](PlayerSetDiscardDriver &sender, const ruled::v1::RuledCommand &command,
                           const QString &description) {
        const quint64 p1Version = p1.stateVersion;
        const quint64 p2Version = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 && (p1.stateVersion <= p1Version || p2.stateVersion <= p2Version)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > p1Version && p2.stateVersion > p2Version;
    };
    auto devPut = [&](int targetPlayer, const char *cardName, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(zone);
        put->set_ready(false);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for player-set discard").arg(cardName));
    };
    auto passPriority = [&](PlayerSetDiscardDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in player-set discard"));
    };

    ASSERT_TRUE(devPut(p1.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(devPut(p2.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(devPut(p1.myId, "Fanatic of the Harrowing", ruled::v1::DEV_ZONE_HAND));
    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_b(1);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(3);
    ASSERT_TRUE(sendAndPump(p1, mana, QStringLiteral("dev: add Fanatic mana")));

    const auto *fanatic = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Fanatic of the Harrowing"));
    ASSERT_NE(fanatic, nullptr);
    ruled::v1::RuledCommand cast;
    cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast.mutable_cast_spell()->mutable_source()->set_hand_index(fanatic->hand_index());
    ASSERT_TRUE(sendAndPump(p1, cast, QStringLiteral("cast Fanatic of the Harrowing")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    p1.playerSetDiscardFlowActive = true;
    p2.playerSetDiscardFlowActive = true;
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    ASSERT_TRUE(p1.pendingChoice.has_value());
    EXPECT_FALSE(p2.pendingChoice.has_value());
    EXPECT_TRUE(p1.sawPlayerSetDiscardPrivateCandidates);
    EXPECT_TRUE(p2.sawPlayerSetDiscardObserverRedaction);
    ASSERT_NE(p1.playerSetDiscardChosenOid, 0u);
    const int p1HandBefore = p1.handSizeByPlayer[p1.myId];
    const int p2HandBefore = p2.handSizeByPlayer[p2.myId];

    ruled::v1::RuledCommand firstChoice;
    firstChoice.mutable_submit_resolution_choice()->add_chosen_object_ids(p1.playerSetDiscardChosenOid);
    p1.pendingChoice.reset();
    ASSERT_TRUE(sendAndPump(p1, firstChoice, QStringLiteral("stage first APNAP discard")));
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], p1HandBefore);
    EXPECT_EQ(p2.handSizeByPlayer[p2.myId], p2HandBefore);
    EXPECT_EQ(p1.graveyardOwnerByEngineOid.count(p1.playerSetDiscardChosenOid), 0u);
    ASSERT_TRUE(p2.pendingChoice.has_value());
    EXPECT_FALSE(p1.pendingChoice.has_value());
    EXPECT_TRUE(p2.sawPlayerSetDiscardPrivateCandidates);
    EXPECT_TRUE(p1.sawPlayerSetDiscardObserverRedaction);
    ASSERT_NE(p2.playerSetDiscardChosenOid, 0u);

    ruled::v1::RuledCommand secondChoice;
    secondChoice.mutable_submit_resolution_choice()->add_chosen_object_ids(p2.playerSetDiscardChosenOid);
    p2.pendingChoice.reset();
    ASSERT_TRUE(sendAndPump(p2, secondChoice, QStringLiteral("commit complete APNAP discard")));
    ASSERT_EQ(p1.graveyardOwnerByEngineOid[p1.playerSetDiscardChosenOid], p1.myId);
    ASSERT_EQ(p1.graveyardOwnerByEngineOid[p2.playerSetDiscardChosenOid], p2.myId);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(p1.playerSetDiscardChosenOid));
    ASSERT_TRUE(p1.serverCardByEngineOid.count(p2.playerSetDiscardChosenOid));
    EXPECT_EQ(p1.serverCardByEngineOid[p1.playerSetDiscardChosenOid], p1.playerSetDiscardChosenServerCardId);
    EXPECT_EQ(p1.serverCardByEngineOid[p2.playerSetDiscardChosenOid], p2.playerSetDiscardChosenServerCardId);
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], p1HandBefore);
    EXPECT_EQ(p2.handSizeByPlayer[p2.myId], p2HandBefore - 1);
}

class GraveyardCohortDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    bool graveyardCohortFlowActive = false;
    bool sawOtherTriggerTargetsRedacted = false;
    bool graveyardCohortPhysicalIdentityContinuous = true;
    std::set<int> graveyardCohortExpectedPhysicalIds;
    std::set<int> graveyardCohortMovedPhysicalIds;
    int graveyardLibraryExpectedPhysicalId = -1;
    bool sawGraveyardToLibraryPhysicalMove = false;
    void onPhysicalEvent(const GameEvent &ev) override
    {
        if (ev.HasExtension(Event_MoveCard::ext)) {
            const auto &mc = ev.GetExtension(Event_MoveCard::ext);
            const QString from = QString::fromStdString(mc.start_zone());
            const QString to = QString::fromStdString(mc.target_zone());

            // ZoneNames, not literals: Cockatrice's exile zone is spelled "rfg".
            const QLatin1String grave(ZoneNames::GRAVE);

            const QLatin1String exile(ZoneNames::EXILE);

            const QLatin1String deck(ZoneNames::DECK);
            if (graveyardCohortFlowActive && from == grave && to == exile &&
                graveyardCohortExpectedPhysicalIds.count(mc.card_id()) > 0) {
                graveyardCohortPhysicalIdentityContinuous =
                    graveyardCohortPhysicalIdentityContinuous && mc.new_card_id() == mc.card_id();
                graveyardCohortMovedPhysicalIds.insert(mc.card_id());
            } else if (graveyardCohortFlowActive && from == grave && to == deck &&
                       mc.card_id() == graveyardLibraryExpectedPhysicalId) {
                graveyardCohortPhysicalIdentityContinuous =
                    graveyardCohortPhysicalIdentityContinuous && mc.new_card_id() == mc.card_id();
                sawGraveyardToLibraryPhysicalMove = true;
            }
        }
    }
    void onRuledEvent(const ruled::v1::RuledEvent &ev) override
    {
        OpeningDriver::onRuledEvent(ev);
        if (ev.has_trigger_needs_target()) {
            const auto &trigger = ev.trigger_needs_target();
            if (trigger.controller_player_id() != myId && graveyardCohortFlowActive)
                sawOtherTriggerTargetsRedacted = trigger.targets().groups_size() == 0;
        }
    }
};

TEST_F(RuledE2ESmokeTest, GraveyardTargetCohortIsPrivateAndMovesExactPhysicalCards)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    GraveyardCohortDriver p1(true, QStringLiteral("graveyardp1"), &transcript);
    GraveyardCohortDriver p2(false, QStringLiteral("graveyardp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Plains")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "graveyard cohort game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "graveyard cohort game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer openingDeadline;
    openingDeadline.start();
    while (openingDeadline.elapsed() < 30000) {
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

    auto sendAndPump = [&](GraveyardCohortDriver &sender, const ruled::v1::RuledCommand &command,
                           const QString &description) {
        const quint64 p1Version = p1.stateVersion;
        const quint64 p2Version = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 && (p1.stateVersion <= p1Version || p2.stateVersion <= p2Version)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > p1Version && p2.stateVersion > p2Version;
    };
    auto devPut = [&](int targetPlayer, const char *cardName, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(zone);
        put->set_ready(ready);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for graveyard cohort").arg(cardName));
    };
    auto devMove = [&](int targetPlayer, const char *cardName, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        auto *move = dev->mutable_move_card();
        move->set_card_name(cardName);
        move->set_zone(zone);
        return sendAndPump(p1, command, QStringLiteral("dev: move %1 for graveyard cohort").arg(cardName));
    };
    auto passPriority = [&](GraveyardCohortDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in graveyard cohort"));
    };

    ASSERT_TRUE(devPut(p1.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devMove(p1.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_GRAVEYARD));
    ASSERT_TRUE(devPut(p1.myId, "Storm Crow", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devMove(p1.myId, "Storm Crow", ruled::v1::DEV_ZONE_GRAVEYARD));
    ASSERT_TRUE(devPut(p2.myId, "Forest", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devMove(p2.myId, "Forest", ruled::v1::DEV_ZONE_GRAVEYARD));
    p1.graveyardCohortFlowActive = true;
    p2.graveyardCohortFlowActive = true;
    ASSERT_TRUE(devPut(p1.myId, "Arashin Sunshield", ruled::v1::DEV_ZONE_BATTLEFIELD, true));

    ASSERT_TRUE(p1.pendingTriggerTarget.has_value());
    ASSERT_EQ(p1.pendingTriggerTarget->targets().groups_size(), 1);
    const auto &group = p1.pendingTriggerTarget->targets().groups(0);
    EXPECT_TRUE(group.same_graveyard());
    EXPECT_EQ(group.min(), 0u);
    EXPECT_EQ(group.max(), 2u);
    EXPECT_TRUE(p2.sawOtherTriggerTargetsRedacted);
    std::vector<quint32> ownTargets;
    for (const quint32 oid : group.valid_graveyard_ids()) {
        if (p1.graveyardOwnerByEngineOid[oid] == p1.myId) {
            ownTargets.push_back(oid);
        }
    }
    ASSERT_EQ(ownTargets.size(), 2u);
    for (const quint32 oid : ownTargets) {
        ASSERT_TRUE(p1.serverCardByEngineOid.count(oid));
        const int physicalId = p1.serverCardByEngineOid[oid];
        p1.graveyardCohortExpectedPhysicalIds.insert(physicalId);
        p2.graveyardCohortExpectedPhysicalIds.insert(physicalId);
    }

    ruled::v1::RuledCommand choose;
    for (const quint32 oid : ownTargets) {
        auto *target = choose.mutable_choose_trigger_target()->add_targets();
        target->set_object_id(oid);
        target->set_group_index(group.group_index());
        target->set_kind(ruled::v1::TARGET_REF_KIND_GRAVEYARD);
    }
    ASSERT_TRUE(sendAndPump(p1, choose, QStringLiteral("choose two cards from one graveyard")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    EXPECT_EQ(p1.graveyardCohortMovedPhysicalIds, p1.graveyardCohortExpectedPhysicalIds);
    EXPECT_EQ(p2.graveyardCohortMovedPhysicalIds, p2.graveyardCohortExpectedPhysicalIds);
    EXPECT_TRUE(p1.graveyardCohortPhysicalIdentityContinuous);
    EXPECT_TRUE(p2.graveyardCohortPhysicalIdentityContinuous);

    ASSERT_TRUE(devPut(p1.myId, "Malevolent Chandelier", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_c(2);
    ASSERT_TRUE(sendAndPump(p1, addMana, QStringLiteral("dev: add {2} for Malevolent Chandelier")));
    const auto chandelier = std::find_if(p1.battlefieldByPlayer[p1.myId].begin(), p1.battlefieldByPlayer[p1.myId].end(),
                                         [](const GraveyardCohortDriver::Permanent &permanent) {
                                             return permanent.cardId == QStringLiteral("malevolent_chandelier");
                                         });
    ASSERT_NE(chandelier, p1.battlefieldByPlayer[p1.myId].end());
    const quint64 abilityKey = (static_cast<quint64>(chandelier->oid) << 32);
    const auto targets = p1.latestLegal.valid_targets_by_ability().find(abilityKey);
    ASSERT_NE(targets, p1.latestLegal.valid_targets_by_ability().end());
    ASSERT_EQ(targets->second.groups_size(), 1);
    const auto &libraryGroup = targets->second.groups(0);
    const auto opponentTarget =
        std::find_if(libraryGroup.valid_graveyard_ids().begin(), libraryGroup.valid_graveyard_ids().end(),
                     [&](quint32 oid) { return p1.graveyardOwnerByEngineOid[oid] == p2.myId; });
    ASSERT_NE(opponentTarget, libraryGroup.valid_graveyard_ids().end());
    ASSERT_TRUE(p1.serverCardByEngineOid.count(*opponentTarget));
    const int libraryPhysicalId = p1.serverCardByEngineOid[*opponentTarget];
    p1.graveyardLibraryExpectedPhysicalId = libraryPhysicalId;
    p2.graveyardLibraryExpectedPhysicalId = libraryPhysicalId;
    ruled::v1::RuledCommand activate;
    auto *ability = activate.mutable_activate_ability();
    ability->set_source_object_id(chandelier->oid);
    ability->set_ability_index(0);
    ability->set_expected_zone_change_generation(chandelier->generation);
    auto *libraryTarget = ability->add_targets();
    libraryTarget->set_object_id(*opponentTarget);
    libraryTarget->set_group_index(libraryGroup.group_index());
    libraryTarget->set_kind(ruled::v1::TARGET_REF_KIND_GRAVEYARD);
    ASSERT_TRUE(sendAndPump(p1, activate, QStringLiteral("activate Malevolent Chandelier")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_TRUE(p1.sawGraveyardToLibraryPhysicalMove);
    EXPECT_TRUE(p2.sawGraveyardToLibraryPhysicalMove);
    EXPECT_TRUE(p1.graveyardCohortPhysicalIdentityContinuous);
    EXPECT_TRUE(p2.graveyardCohortPhysicalIdentityContinuous);
}

TEST_F(RuledE2ESmokeTest, WarpCastExilesAtEndStepAndPublishesOwnerOnlyPermission)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    OpeningDriver p1(true, QStringLiteral("warpp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("warpp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Plains")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Warp game start"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Warp game start"));
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
    auto send = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command, const QString &label) {
        const quint64 v1 = p1.stateVersion;
        const quint64 v2 = p2.stateVersion;
        sender.sendRuled(command, label);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= v1 || p2.stateVersion <= v2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > v1 && p2.stateVersion > v2;
    };
    ruled::v1::RuledCommand put;
    put.mutable_dev_command()->set_target_player_id(p1.myId);
    auto *placement = put.mutable_dev_command()->mutable_put_card_in_zone();
    placement->set_card_name("Knight Luminary");
    placement->set_zone(ruled::v1::DEV_ZONE_HAND);
    placement->set_ready(true);
    ASSERT_TRUE(send(p1, put, QStringLiteral("Warp put Knight")));
    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_w(1);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(1);
    ASSERT_TRUE(send(p1, mana, QStringLiteral("Warp mana")));
    const auto actions = p1.handActions(ruled::v1::HAND_ACTION_CAST_SPELL);
    const auto warp = std::find_if(actions.begin(), actions.end(), [](const auto *action) {
        return action->card_name() == "Knight Luminary" && action->cast_method() == ruled::v1::CAST_METHOD_WARP;
    });
    ASSERT_NE(warp, actions.end());
    ruled::v1::RuledCommand cast;
    cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_WARP);
    cast.mutable_cast_spell()->mutable_source()->set_hand_index((*warp)->hand_index());
    ASSERT_TRUE(send(p1, cast, QStringLiteral("Warp cast Knight")));
    auto pass = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("Warp pass"));
    };
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    ASSERT_TRUE(p1.pumpUntil(
        [&] {
            const auto current = p1.battlefieldByPlayer.find(p1.myId);
            return current != p1.battlefieldByPlayer.end() &&
                   std::any_of(current->second.begin(), current->second.end(),
                               [](const auto &object) { return object.cardId == QLatin1String("knight_luminary"); });
        },
        10000, "Warp Knight battlefield projection"));
    p2.pumpUntil([&] { return p2.stateVersion >= p1.stateVersion; }, 10000, "Warp observer projection");
    const auto battlefield = p1.battlefieldByPlayer.find(p1.myId);
    ASSERT_NE(battlefield, p1.battlefieldByPlayer.end());
    const auto knight = std::find_if(battlefield->second.begin(), battlefield->second.end(), [](const auto &object) {
        return object.cardId == QLatin1String("knight_luminary");
    });
    ASSERT_NE(knight, battlefield->second.end());
    const quint32 oid = knight->oid;
    const int physicalId = p1.serverCardByEngineOid.at(oid);
    QElapsedTimer toEnd;
    toEnd.start();
    while (!(p1.phase == ruled::v1::PHASE_ID_END_STEP && p1.stackDepth > 0) && toEnd.elapsed() < 30000) {
        if (p1.priorityPlayer == p1.myId)
            ASSERT_TRUE(pass(p1));
        else if (p1.priorityPlayer == p2.myId)
            ASSERT_TRUE(pass(p2));
        else {
            p1.pump(25);
            p2.pump(25);
        }
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_END_STEP);
    ASSERT_TRUE(pass(p1.priorityPlayer == p1.myId ? p1 : p2));
    ASSERT_TRUE(pass(p1.priorityPlayer == p1.myId ? p1 : p2));
    EXPECT_EQ(p1.serverCardByEngineOid.at(oid), physicalId);
    EXPECT_EQ(p2.serverCardByEngineOid.at(oid), physicalId);
    const auto ownerGroup = std::find_if(
        p1.latestLegal.exile_play_permission_groups().begin(), p1.latestLegal.exile_play_permission_groups().end(),
        [](const auto &group) { return group.source_label().find("Warp") != std::string::npos; });
    EXPECT_NE(ownerGroup, p1.latestLegal.exile_play_permission_groups().end());
    EXPECT_TRUE(std::none_of(p2.latestLegal.exile_play_permission_groups().begin(),
                             p2.latestLegal.exile_play_permission_groups().end(),
                             [](const auto &group) { return group.source_label().find("Warp") != std::string::npos; }));
}

class AirbendDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    int airbendOwnerId = -1;
    bool sawAirbendBattlefieldToExile = false;
    bool sawAirbendExileToStack = false;
    bool sawAirbendStackToBattlefield = false;
    bool airbendPhysicalIdentityContinuous = true;
    int airbendPhysicalCardId = -1;
    void onPhysicalEvent(const GameEvent &ev) override
    {
        if (ev.HasExtension(Event_MoveCard::ext)) {
            const auto &mc = ev.GetExtension(Event_MoveCard::ext);
            const QString from = QString::fromStdString(mc.start_zone());
            const QString to = QString::fromStdString(mc.target_zone());
            const QString name = QString::fromStdString(mc.card_name());
            // ZoneNames, not literals: Cockatrice's exile zone is spelled "rfg".

            const QLatin1String stack(ZoneNames::STACK);

            const QLatin1String exile(ZoneNames::EXILE);
            const QLatin1String table(ZoneNames::TABLE);

            if (name == QLatin1String("Grizzly Bears") && airbendPhysicalCardId >= 0) {
                auto followPhysicalCard = [&] {
                    if (mc.card_id() != airbendPhysicalCardId) {
                        airbendPhysicalIdentityContinuous = false;
                    }
                    airbendPhysicalCardId = mc.new_card_id();
                };
                if (from == table && to == exile && mc.target_player_id() == airbendOwnerId) {
                    followPhysicalCard();
                    sawAirbendBattlefieldToExile = true;
                } else if (from == exile && to == stack && mc.start_player_id() == airbendOwnerId) {
                    followPhysicalCard();
                    sawAirbendExileToStack = true;
                } else if (from == stack && to == table && mc.target_player_id() == airbendOwnerId) {
                    followPhysicalCard();
                    sawAirbendStackToBattlefield = true;
                }
            }
        }
    }
};

TEST_F(RuledE2ESmokeTest, AirbendPublishesOwnerOnlyAlternativeCastAndPreservesPhysicalIdentity)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    AirbendDriver p1(true, QStringLiteral("airbendp1"), &transcript);
    AirbendDriver p2(false, QStringLiteral("airbendp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Plains")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Airbend game start"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Airbend game start"));
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

    auto send = [&](AirbendDriver &sender, const ruled::v1::RuledCommand &command, const QString &label) {
        const quint64 v1 = p1.stateVersion;
        const quint64 v2 = p2.stateVersion;
        sender.sendRuled(command, label);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= v1 || p2.stateVersion <= v2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > v1 && p2.stateVersion > v2;
    };
    auto put = [&](int playerId, const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        command.mutable_dev_command()->set_target_player_id(playerId);
        auto *placement = command.mutable_dev_command()->mutable_put_card_in_zone();
        placement->set_card_name(name);
        placement->set_zone(zone);
        placement->set_ready(true);
        return send(p1, command, QStringLiteral("Airbend put %1").arg(QString::fromLatin1(name)));
    };
    ASSERT_TRUE(put(p2.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put(p1.myId, "Airbending Lesson", ruled::v1::DEV_ZONE_HAND));
    const auto battlefield = p1.battlefieldByPlayer.find(p2.myId);
    ASSERT_NE(battlefield, p1.battlefieldByPlayer.end());
    const auto bear = std::find_if(battlefield->second.begin(), battlefield->second.end(),
                                   [](const auto &object) { return object.cardId == QLatin1String("grizzly_bears"); });
    ASSERT_NE(bear, battlefield->second.end());
    const quint32 bearOid = bear->oid;
    ASSERT_TRUE(p1.serverCardByEngineOid.count(bearOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(bearOid));
    const int physicalId = p1.serverCardByEngineOid[bearOid];
    ASSERT_EQ(p2.serverCardByEngineOid[bearOid], physicalId);
    for (AirbendDriver *client : {&p1, &p2}) {
        client->airbendOwnerId = p2.myId;
        client->airbendPhysicalCardId = physicalId;
    }

    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_w(1);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(2);
    ASSERT_TRUE(send(p1, mana, QStringLiteral("Airbend caster mana")));
    const auto *lesson = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Airbending Lesson"));
    ASSERT_NE(lesson, nullptr);
    ruled::v1::RuledCommand castLesson;
    auto *lessonCast = castLesson.mutable_cast_spell();
    lessonCast->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    lessonCast->mutable_source()->set_hand_index(lesson->hand_index());
    auto *target = lessonCast->add_targets();
    target->set_object_id(bearOid);
    target->set_group_index(0);
    target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(send(p1, castLesson, QStringLiteral("cast Airbending Lesson")));
    QElapsedTimer lessonStack;
    lessonStack.start();
    while ((p1.stackDepth == 0 || p2.stackDepth == 0) && lessonStack.elapsed() < 10000) {
        p1.pump(25);
        p2.pump(25);
    }
    ASSERT_GT(p1.stackDepth, 0);
    ASSERT_GT(p2.stackDepth, 0);
    QElapsedTimer resolveLesson;
    resolveLesson.start();
    while ((p1.stackDepth > 0 || p2.stackDepth > 0) && resolveLesson.elapsed() < 10000) {
        ruled::v1::RuledCommand pass;
        pass.mutable_pass_priority();
        AirbendDriver &priority = p1.priorityPlayer == p1.myId ? p1 : p2;
        ASSERT_TRUE(send(priority, pass, QStringLiteral("resolve Airbending Lesson")));
    }
    ASSERT_EQ(p1.stackDepth, 0);
    ASSERT_TRUE(p2.pumpUntil(
        [&] {
            return std::any_of(p2.latestLegal.exile_play_permission_groups().begin(),
                               p2.latestLegal.exile_play_permission_groups().end(), [&](const auto &group) {
                                   return group.source_label() == "Airbending Lesson" &&
                                          std::find(group.object_ids().begin(), group.object_ids().end(), bearOid) !=
                                              group.object_ids().end();
                               });
        },
        10000, "Airbend owner permission"));
    EXPECT_TRUE(std::none_of(p1.latestLegal.exile_play_permission_groups().begin(),
                             p1.latestLegal.exile_play_permission_groups().end(),
                             [](const auto &group) { return group.source_label() == "Airbending Lesson"; }));
    EXPECT_TRUE(p1.airbendPhysicalIdentityContinuous);
    EXPECT_TRUE(p2.airbendPhysicalIdentityContinuous);
    EXPECT_TRUE(p1.sawAirbendBattlefieldToExile && p2.sawAirbendBattlefieldToExile);
    EXPECT_EQ(p1.serverCardByEngineOid[bearOid], p1.airbendPhysicalCardId);
    EXPECT_EQ(p2.serverCardByEngineOid[bearOid], p2.airbendPhysicalCardId);

    QElapsedTimer nextTurn;
    nextTurn.start();
    while (!(p1.activePlayer == p2.myId && p1.phase == ruled::v1::PHASE_ID_MAIN1) && nextTurn.elapsed() < 45000) {
        p1.pump(25);
        p2.pump(25);
        ruled::v1::RuledCommand advance;
        AirbendDriver *sender = nullptr;
        QString label;
        AirbendDriver &active = p1.activePlayer == p1.myId ? p1 : p2;
        const auto cleanupDiscards = active.handActions(ruled::v1::HAND_ACTION_CLEANUP_DISCARD);
        if (!cleanupDiscards.isEmpty()) {
            const int excess = cleanupDiscards.size() - 7;
            auto *discard = advance.mutable_discard_to_hand_size();
            for (int i = 0; i < excess; ++i) {
                discard->add_hand_card_indices(cleanupDiscards.at(i)->hand_index());
            }
            sender = &active;
            label = QStringLiteral("Airbend cleanup discard");
        } else if (p1.phase == ruled::v1::PHASE_ID_DECLARE_ATTACKERS) {
            advance.mutable_declare_attackers();
            sender = p1.activePlayer == p1.myId ? &p1 : &p2;
            label = QStringLiteral("Airbend declare no attackers");
        } else if (p1.phase == ruled::v1::PHASE_ID_DECLARE_BLOCKERS) {
            advance.mutable_declare_blockers();
            sender = p1.activePlayer == p1.myId ? &p2 : &p1;
            label = QStringLiteral("Airbend declare no blockers");
        } else if (p1.priorityPlayer == p1.myId) {
            advance.mutable_pass_priority();
            sender = &p1;
            label = QStringLiteral("Airbend advance turn");
        } else if (p1.priorityPlayer == p2.myId) {
            advance.mutable_pass_priority();
            sender = &p2;
            label = QStringLiteral("Airbend advance turn");
        }
        if (sender != nullptr) {
            ASSERT_TRUE(send(*sender, advance, label));
        }
    }
    ASSERT_EQ(p1.activePlayer, p2.myId);
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ruled::v1::RuledCommand ownerMana;
    ownerMana.mutable_dev_command()->set_target_player_id(p2.myId);
    ownerMana.mutable_dev_command()->mutable_add_mana()->set_c(2);
    ASSERT_TRUE(send(p2, ownerMana, QStringLiteral("Airbend owner mana")));
    const auto action = std::find_if(p2.latestLegal.zone_cast_actions().begin(),
                                     p2.latestLegal.zone_cast_actions().end(), [&](const auto &candidate) {
                                         return candidate.object_id() == bearOid &&
                                                candidate.cast_method() == ruled::v1::CAST_METHOD_PERMISSION;
                                     });
    ASSERT_NE(action, p2.latestLegal.zone_cast_actions().end());
    ASSERT_TRUE(action->has_casting_permission_id());
    EXPECT_EQ(action->cost(), "{2}");
    ruled::v1::RuledCommand castBear;
    auto *permissionCast = castBear.mutable_cast_spell();
    permissionCast->set_cast_method(action->cast_method());
    permissionCast->set_casting_permission_id(action->casting_permission_id());
    permissionCast->mutable_source()->set_exile_object_id(bearOid);
    permissionCast->mutable_source()->set_expected_zone_change_generation(action->zone_change_generation());
    ASSERT_TRUE(send(p2, castBear, QStringLiteral("cast Airbent Bears for two")));
    QElapsedTimer bearStack;
    bearStack.start();
    while ((p1.stackDepth == 0 || p2.stackDepth == 0) && bearStack.elapsed() < 10000) {
        p1.pump(25);
        p2.pump(25);
    }
    ASSERT_GT(p1.stackDepth, 0);
    ASSERT_GT(p2.stackDepth, 0);
    QElapsedTimer resolveBear;
    resolveBear.start();
    while ((p1.stackDepth > 0 || p2.stackDepth > 0) && resolveBear.elapsed() < 10000) {
        ruled::v1::RuledCommand pass;
        pass.mutable_pass_priority();
        AirbendDriver &priority = p1.priorityPlayer == p1.myId ? p1 : p2;
        ASSERT_TRUE(send(priority, pass, QStringLiteral("resolve Airbent Bears")));
    }
    ASSERT_EQ(p1.stackDepth, 0);
    ASSERT_TRUE(p2.pumpUntil(
        [&] {
            const auto permanents = p2.battlefieldByPlayer.find(p2.myId);
            return permanents != p2.battlefieldByPlayer.end() &&
                   std::any_of(permanents->second.begin(), permanents->second.end(),
                               [&](const auto &object) { return object.oid == bearOid; });
        },
        10000, "Airbent Bears battlefield"));
    EXPECT_TRUE(p1.sawAirbendExileToStack && p2.sawAirbendExileToStack);
    EXPECT_TRUE(p1.sawAirbendStackToBattlefield && p2.sawAirbendStackToBattlefield);
    EXPECT_TRUE(p1.airbendPhysicalIdentityContinuous);
    EXPECT_TRUE(p2.airbendPhysicalIdentityContinuous);
    EXPECT_EQ(p1.serverCardByEngineOid[bearOid], p1.airbendPhysicalCardId);
    EXPECT_EQ(p2.serverCardByEngineOid[bearOid], p2.airbendPhysicalCardId);
}

TEST_F(RuledE2ESmokeTest, FloodpitsDrownerMovesExactPermanentsAndKeepsLibrariesHidden)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    OpeningDriver p1(true, QStringLiteral("drownerp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("drownerp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "Floodpits Drowner game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "Floodpits Drowner game start (p2)"));
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

    auto sendAndPump = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const quint64 p1Version = p1.stateVersion;
        const quint64 p2Version = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 && (p1.stateVersion <= p1Version || p2.stateVersion <= p2Version)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > p1Version && p2.stateVersion > p2Version;
    };
    auto devPut = [&](int player, const char *cardName) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(player);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(true);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for Floodpits Drowner").arg(cardName));
    };
    auto passPriority = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority for Floodpits Drowner"));
    };
    auto findPermanent = [](const OpeningDriver &client, int controller,
                            const QString &cardId) -> const OpeningDriver::Permanent * {
        const auto player = client.battlefieldByPlayer.find(controller);
        if (player == client.battlefieldByPlayer.end()) {
            return nullptr;
        }
        const auto permanent = std::find_if(player->second.begin(), player->second.end(),
                                            [&](const auto &candidate) { return candidate.cardId == cardId; });
        return permanent == player->second.end() ? nullptr : &*permanent;
    };

    // Put the target down first so Drowner's ETB both proves normal trigger targeting and supplies
    // the stun counter required by its activated ability.
    ASSERT_TRUE(devPut(p2.myId, "Grizzly Bears"));
    ASSERT_TRUE(devPut(p1.myId, "Floodpits Drowner"));
    const auto *targetBeforeEtb = findPermanent(p1, p2.myId, QStringLiteral("grizzly_bears"));
    ASSERT_NE(targetBeforeEtb, nullptr);
    ASSERT_TRUE(p1.pendingTriggerTarget.has_value());
    ASSERT_EQ(p1.pendingTriggerTarget->targets().groups_size(), 1);
    const auto &etbGroup = p1.pendingTriggerTarget->targets().groups(0);
    ASSERT_TRUE(std::find(etbGroup.valid_permanent_ids().begin(), etbGroup.valid_permanent_ids().end(),
                          targetBeforeEtb->oid) != etbGroup.valid_permanent_ids().end());
    ruled::v1::RuledCommand chooseEtbTarget;
    auto *etbTarget = chooseEtbTarget.mutable_choose_trigger_target()->add_targets();
    etbTarget->set_object_id(targetBeforeEtb->oid);
    etbTarget->set_group_index(etbGroup.group_index());
    etbTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p1, chooseEtbTarget, QStringLiteral("choose Drowner ETB target")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_u(1);
    addMana.mutable_dev_command()->mutable_add_mana()->set_c(1);
    ASSERT_TRUE(sendAndPump(p1, addMana, QStringLiteral("dev: add {1}{U} for Floodpits Drowner")));

    const auto *source = findPermanent(p1, p1.myId, QStringLiteral("floodpits_drowner"));
    const auto *target = findPermanent(p1, p2.myId, QStringLiteral("grizzly_bears"));
    ASSERT_NE(source, nullptr);
    ASSERT_NE(target, nullptr);
    const quint32 sourceOid = source->oid;
    const quint32 targetOid = target->oid;
    const quint64 sourceGeneration = source->generation;
    ASSERT_TRUE(p1.serverCardByEngineOid.count(sourceOid));
    ASSERT_TRUE(p1.serverCardByEngineOid.count(targetOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(sourceOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(targetOid));
    const int sourcePhysicalId = p1.serverCardByEngineOid.at(sourceOid);
    const int targetPhysicalId = p1.serverCardByEngineOid.at(targetOid);
    ASSERT_EQ(p2.serverCardByEngineOid.at(sourceOid), sourcePhysicalId);
    ASSERT_EQ(p2.serverCardByEngineOid.at(targetOid), targetPhysicalId);

    const quint64 abilityKey = static_cast<quint64>(sourceOid) << 32;
    const auto legal = p1.latestLegal.valid_targets_by_ability().find(abilityKey);
    ASSERT_NE(legal, p1.latestLegal.valid_targets_by_ability().end());
    ASSERT_EQ(legal->second.groups_size(), 1);
    const auto &activationGroup = legal->second.groups(0);
    ASSERT_TRUE(std::find(activationGroup.valid_permanent_ids().begin(), activationGroup.valid_permanent_ids().end(),
                          targetOid) != activationGroup.valid_permanent_ids().end());

    ruled::v1::RuledCommand activate;
    auto *ability = activate.mutable_activate_ability();
    ability->set_source_object_id(sourceOid);
    ability->set_expected_zone_change_generation(sourceGeneration);
    ability->set_ability_index(0);
    auto *abilityTarget = ability->add_targets();
    abilityTarget->set_object_id(targetOid);
    abilityTarget->set_group_index(activationGroup.group_index());
    abilityTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p1, activate, QStringLiteral("activate Floodpits Drowner")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    for (const OpeningDriver *client : {&p1, &p2}) {
        EXPECT_EQ(findPermanent(*client, p1.myId, QStringLiteral("floodpits_drowner")), nullptr);
        EXPECT_EQ(findPermanent(*client, p2.myId, QStringLiteral("grizzly_bears")), nullptr);
        const auto movedExactCard = [&](int physicalId, int owner) {
            return std::any_of(client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(),
                               [&](const Event_MoveCard &move) {
                                   return move.start_zone() == ZoneNames::TABLE &&
                                          move.target_zone() == ZoneNames::DECK && move.card_id() == physicalId &&
                                          move.new_card_id() == physicalId && move.target_player_id() == owner;
                               });
        };
        EXPECT_TRUE(movedExactCard(sourcePhysicalId, p1.myId))
            << "the exact physical Drowner did not enter its owner's library";
        EXPECT_TRUE(movedExactCard(targetPhysicalId, p2.myId))
            << "the exact physical target did not enter its owner's library";
        EXPECT_TRUE(client->libraryDetailsStayedConcealed)
            << "a client received hidden library card identity or ordering";
    }
}

TEST_F(RuledE2ESmokeTest, DemolitionFieldRoutesIndependentPrivateSearchesToBothSeats)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    OpeningDriver p1(true, QStringLiteral("demolitionp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("demolitionp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Demolition Field game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Demolition Field game start (p2)"));
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

    auto sendAndPump = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const quint64 p1Version = p1.stateVersion;
        const quint64 p2Version = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 && (p1.stateVersion <= p1Version || p2.stateVersion <= p2Version)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > p1Version && p2.stateVersion > p2Version;
    };
    auto devPut = [&](int player, const char *cardName) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(player);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(true);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for Demolition Field").arg(cardName));
    };
    auto passPriority = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority for Demolition Field"));
    };
    auto findPermanent = [](const OpeningDriver &client, int controller,
                            const QString &cardId) -> const OpeningDriver::Permanent * {
        const auto player = client.battlefieldByPlayer.find(controller);
        if (player == client.battlefieldByPlayer.end()) {
            return nullptr;
        }
        const auto permanent = std::find_if(player->second.begin(), player->second.end(),
                                            [&](const auto &candidate) { return candidate.cardId == cardId; });
        return permanent == player->second.end() ? nullptr : &*permanent;
    };
    const auto observerChoiceIsRedacted = [](const OpeningDriver &client) {
        return client.lastResolutionChoice.has_value() &&
               client.lastResolutionChoice->candidate_object_ids_size() == 0 &&
               client.lastResolutionChoice->candidate_card_ids_size() == 0 &&
               client.lastResolutionChoice->candidate_names_size() == 0 &&
               client.lastResolutionChoice->candidate_server_card_ids_size() == 0 &&
               client.lastResolutionChoice->resolution_branches_size() == 0 &&
               client.lastResolutionChoice->prompt_text() == "Opponent is making a resolution choice.";
    };

    ASSERT_TRUE(devPut(p1.myId, "Demolition Field"));
    ASSERT_TRUE(devPut(p2.myId, "Taiga"));
    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_c(2);
    ASSERT_TRUE(sendAndPump(p1, addMana, QStringLiteral("dev: add {2} for Demolition Field")));

    const auto *source = findPermanent(p1, p1.myId, QStringLiteral("demolition_field"));
    const auto *target = findPermanent(p1, p2.myId, QStringLiteral("taiga"));
    ASSERT_NE(source, nullptr);
    ASSERT_NE(target, nullptr);
    const quint32 sourceOid = source->oid;
    const quint32 targetOid = target->oid;
    const quint64 sourceGeneration = source->generation;
    ASSERT_TRUE(p1.serverCardByEngineOid.count(sourceOid));
    ASSERT_TRUE(p1.serverCardByEngineOid.count(targetOid));
    const int sourcePhysicalId = p1.serverCardByEngineOid.at(sourceOid);
    const int targetPhysicalId = p1.serverCardByEngineOid.at(targetOid);

    const quint64 abilityKey = (static_cast<quint64>(sourceOid) << 32) | 1u;
    const auto legal = p1.latestLegal.valid_targets_by_ability().find(abilityKey);
    ASSERT_NE(legal, p1.latestLegal.valid_targets_by_ability().end());
    ASSERT_EQ(legal->second.groups_size(), 1);
    const auto &targetGroup = legal->second.groups(0);
    ASSERT_TRUE(std::find(targetGroup.valid_permanent_ids().begin(), targetGroup.valid_permanent_ids().end(),
                          targetOid) != targetGroup.valid_permanent_ids().end());

    p1.pendingChoice.reset();
    p2.pendingChoice.reset();
    p1.lastResolutionChoice.reset();
    p2.lastResolutionChoice.reset();
    ruled::v1::RuledCommand activate;
    auto *ability = activate.mutable_activate_ability();
    ability->set_source_object_id(sourceOid);
    ability->set_expected_zone_change_generation(sourceGeneration);
    ability->set_ability_index(1);
    auto *abilityTarget = ability->add_targets();
    abilityTarget->set_object_id(targetOid);
    abilityTarget->set_group_index(targetGroup.group_index());
    abilityTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p1, activate, QStringLiteral("activate Demolition Field")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    ASSERT_TRUE(p2.pendingChoice.has_value());
    EXPECT_EQ(p2.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    EXPECT_EQ(p2.pendingChoice->deciding_player_id(), p2.myId);
    EXPECT_EQ(p2.pendingChoice->min(), 0u);
    ASSERT_EQ(p2.pendingChoice->resolution_branches_size(), 1);
    EXPECT_EQ(p2.pendingChoice->resolution_branches(0).label(), "Search");
    EXPECT_FALSE(p1.pendingChoice.has_value());
    EXPECT_TRUE(observerChoiceIsRedacted(p1));
    EXPECT_EQ(findPermanent(p1, p1.myId, QStringLiteral("demolition_field")), nullptr);
    EXPECT_EQ(findPermanent(p1, p2.myId, QStringLiteral("taiga")), nullptr);

    ruled::v1::RuledCommand acceptFirstSearch;
    acceptFirstSearch.mutable_submit_resolution_choice()->set_decision(
        ruled::v1::RESOLUTION_CHOICE_DECISION_SELECT_BRANCH);
    acceptFirstSearch.mutable_submit_resolution_choice()->set_selected_branch_index(0);
    p2.pendingChoice.reset();
    p1.lastResolutionChoice.reset();
    ASSERT_TRUE(sendAndPump(p2, acceptFirstSearch, QStringLiteral("target controller elects to search")));

    ASSERT_TRUE(p2.pendingChoice.has_value());
    ASSERT_EQ(p2.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
    EXPECT_EQ(p2.pendingChoice->deciding_player_id(), p2.myId);
    ASSERT_GT(p2.pendingChoice->candidate_object_ids_size(), 0);
    EXPECT_EQ(p2.pendingChoice->candidate_object_ids_size(), p2.pendingChoice->candidate_names_size());
    EXPECT_EQ(p2.pendingChoice->candidate_object_ids_size(), p2.pendingChoice->candidate_server_card_ids_size());
    EXPECT_FALSE(p1.pendingChoice.has_value());
    EXPECT_TRUE(observerChoiceIsRedacted(p1));
    const quint32 chosenIslandOid = p2.pendingChoice->candidate_object_ids(0);

    ruled::v1::RuledCommand chooseIsland;
    chooseIsland.mutable_submit_resolution_choice()->add_chosen_object_ids(chosenIslandOid);
    p2.pendingChoice.reset();
    p1.lastResolutionChoice.reset();
    ASSERT_TRUE(sendAndPump(p2, chooseIsland, QStringLiteral("target controller finds Island")));

    ASSERT_TRUE(p1.pendingChoice.has_value());
    EXPECT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    EXPECT_EQ(p1.pendingChoice->deciding_player_id(), p1.myId);
    EXPECT_FALSE(p2.pendingChoice.has_value());
    EXPECT_TRUE(observerChoiceIsRedacted(p2));
    const auto *island1 = findPermanent(p1, p2.myId, QStringLiteral("island"));
    const auto *island2 = findPermanent(p2, p2.myId, QStringLiteral("island"));
    ASSERT_NE(island1, nullptr);
    ASSERT_NE(island2, nullptr);
    EXPECT_EQ(island1->oid, chosenIslandOid);
    EXPECT_EQ(island2->oid, chosenIslandOid);
    EXPECT_FALSE(island1->tapped);
    EXPECT_FALSE(island2->tapped);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(chosenIslandOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(chosenIslandOid));
    EXPECT_EQ(p1.serverCardByEngineOid.at(chosenIslandOid), p2.serverCardByEngineOid.at(chosenIslandOid));

    ruled::v1::RuledCommand declineSecondSearch;
    declineSecondSearch.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_DECLINE);
    p1.pendingChoice.reset();
    ASSERT_TRUE(sendAndPump(p1, declineSecondSearch, QStringLiteral("activator declines second search")));
    EXPECT_FALSE(p1.pendingChoice.has_value());
    EXPECT_FALSE(p2.pendingChoice.has_value());

    for (const OpeningDriver *client : {&p1, &p2}) {
        const auto movedToGrave = [&](int physicalId) {
            return std::any_of(client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(),
                               [&](const Event_MoveCard &move) {
                                   return move.start_zone() == ZoneNames::TABLE &&
                                          move.target_zone() == ZoneNames::GRAVE && move.card_id() == physicalId;
                               });
        };
        EXPECT_TRUE(movedToGrave(sourcePhysicalId));
        EXPECT_TRUE(movedToGrave(targetPhysicalId));
        EXPECT_TRUE(client->libraryDetailsStayedConcealed);
    }
}

TEST_F(RuledE2ESmokeTest, SpyglassSirenMapExplorePublishesOnePublicLibraryCardToBothSeats)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    OpeningDriver p1(true, QStringLiteral("explorep1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("explorep2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Storm Crow")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Explore game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Explore game start (p2)"));
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

    auto sendAndPump = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const quint64 p1Version = p1.stateVersion;
        const quint64 p2Version = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 && (p1.stateVersion <= p1Version || p2.stateVersion <= p2Version)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > p1Version && p2.stateVersion > p2Version;
    };
    auto passPriority = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority for Explore"));
    };
    auto devPutBattlefield = [&](int player, const char *cardName) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(player);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(true);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for Explore").arg(cardName));
    };
    auto findPermanent = [](const OpeningDriver &client, int controller,
                            const QString &cardId) -> const OpeningDriver::Permanent * {
        const auto player = client.battlefieldByPlayer.find(controller);
        if (player == client.battlefieldByPlayer.end()) {
            return nullptr;
        }
        const auto permanent = std::find_if(player->second.begin(), player->second.end(),
                                            [&](const auto &candidate) { return candidate.cardId == cardId; });
        return permanent == player->second.end() ? nullptr : &*permanent;
    };

    ASSERT_TRUE(devPutBattlefield(p1.myId, "Spyglass Siren"));
    ASSERT_EQ(p1.stackDepth, 1);
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    const auto *siren = findPermanent(p1, p1.myId, QStringLiteral("spyglass_siren"));
    const auto *map = findPermanent(p1, p1.myId, QStringLiteral("map"));
    const auto *observerMap = findPermanent(p2, p1.myId, QStringLiteral("map"));
    ASSERT_NE(siren, nullptr);
    ASSERT_NE(map, nullptr);
    ASSERT_NE(observerMap, nullptr);
    const quint32 sirenOid = siren->oid;
    const quint32 mapOid = map->oid;
    const quint64 mapGeneration = map->generation;
    ASSERT_EQ(observerMap->oid, mapOid);

    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_c(1);
    ASSERT_TRUE(sendAndPump(p1, addMana, QStringLiteral("dev: add {1} for Map")));

    const quint64 abilityKey = static_cast<quint64>(mapOid) << 32;
    const auto legal = p1.latestLegal.valid_targets_by_ability().find(abilityKey);
    ASSERT_NE(legal, p1.latestLegal.valid_targets_by_ability().end());
    ASSERT_EQ(legal->second.groups_size(), 1);
    const auto &group = legal->second.groups(0);
    ASSERT_TRUE(std::find(group.valid_permanent_ids().begin(), group.valid_permanent_ids().end(), sirenOid) !=
                group.valid_permanent_ids().end());

    p1.pendingChoice.reset();
    p2.pendingChoice.reset();
    p1.lastResolutionChoice.reset();
    p2.lastResolutionChoice.reset();
    ruled::v1::RuledCommand activate;
    auto *ability = activate.mutable_activate_ability();
    ability->set_source_object_id(mapOid);
    ability->set_expected_zone_change_generation(mapGeneration);
    ability->set_ability_index(0);
    auto *target = ability->add_targets();
    target->set_object_id(sirenOid);
    target->set_group_index(group.group_index());
    target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p1, activate, QStringLiteral("activate Map targeting Spyglass Siren")));
    EXPECT_EQ(findPermanent(p1, p1.myId, QStringLiteral("map")), nullptr);
    EXPECT_EQ(findPermanent(p2, p1.myId, QStringLiteral("map")), nullptr);
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    ASSERT_TRUE(p1.pendingChoice.has_value());
    ASSERT_TRUE(p2.lastResolutionChoice.has_value());
    const auto &chooser = *p1.pendingChoice;
    const auto &observer = *p2.lastResolutionChoice;
    ASSERT_EQ(chooser.choice_kind(), ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    ASSERT_EQ(observer.choice_kind(), ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    ASSERT_EQ(chooser.candidate_names_size(), 1);
    ASSERT_EQ(observer.candidate_names_size(), 1);
    EXPECT_EQ(chooser.candidate_names(0), "Storm Crow");
    EXPECT_EQ(observer.candidate_names(0), "Storm Crow");
    EXPECT_TRUE(chooser.has_public_reveal());
    EXPECT_TRUE(observer.has_public_reveal());
    ASSERT_EQ(chooser.candidate_selectable_size(), 1);
    EXPECT_TRUE(chooser.candidate_selectable(0));
    EXPECT_EQ(observer.candidate_selectable_size(), 0);
    EXPECT_EQ(observer.prompt_text(), "Opponent is making a resolution choice.");
    ASSERT_EQ(chooser.candidate_server_card_ids_size(), 1);
    ASSERT_EQ(observer.candidate_server_card_ids_size(), 1);
    EXPECT_EQ(chooser.candidate_server_card_ids(0), observer.candidate_server_card_ids(0));
    ruled::v1::RuledCommand graveyard;
    graveyard.mutable_submit_resolution_choice()->add_chosen_object_ids(chooser.candidate_object_ids(0));
    p1.pendingChoice.reset();
    ASSERT_TRUE(sendAndPump(p1, graveyard, QStringLiteral("put explored Storm Crow into graveyard")));
    EXPECT_FALSE(p1.pendingChoice.has_value());
    EXPECT_EQ(p1.stackDepth, 0);
    EXPECT_EQ(p2.stackDepth, 0);

    auto exploredMove = [](const OpeningDriver &client) {
        return std::find_if(client.physicalMoveEvents.begin(), client.physicalMoveEvents.end(),
                            [](const Event_MoveCard &move) {
                                return move.start_zone() == ZoneNames::DECK && move.target_zone() == ZoneNames::GRAVE &&
                                       move.card_name() == "Storm Crow";
                            });
    };
    const auto p1Move = exploredMove(p1);
    const auto p2Move = exploredMove(p2);
    ASSERT_NE(p1Move, p1.physicalMoveEvents.end());
    ASSERT_NE(p2Move, p2.physicalMoveEvents.end());
    EXPECT_EQ(p1Move->card_id(), p2Move->card_id());
    EXPECT_EQ(p1Move->new_card_id(), p2Move->new_card_id());

    for (const OpeningDriver *client : {&p1, &p2}) {
        const auto *updatedSiren = findPermanent(*client, p1.myId, QStringLiteral("spyglass_siren"));
        ASSERT_NE(updatedSiren, nullptr);
        EXPECT_EQ(updatedSiren->power, 2);
        EXPECT_EQ(updatedSiren->toughness, 2);
        EXPECT_TRUE(client->libraryDetailsStayedConcealed);
    }

    // Repeat with a land: no resolution choice is needed, but BOTH seats must receive its
    // immutable reveal even though the physical card moves to hand in the very same batch.
    ASSERT_TRUE(devPutBattlefield(p1.myId, "Forest"));
    ruled::v1::RuledCommand moveForest;
    auto *moveDev = moveForest.mutable_dev_command();
    moveDev->set_target_player_id(p1.myId);
    moveDev->mutable_move_card()->set_card_name("Forest");
    moveDev->mutable_move_card()->set_zone(ruled::v1::DEV_ZONE_LIBRARY);
    ASSERT_TRUE(sendAndPump(p1, moveForest, QStringLiteral("move Forest into library for land explore")));
    // Dev movement appends to the library. Remove every Crow from this fixed 40-card
    // fixture so Forest is the only library card, without relying on a hidden top-card ID.
    ruled::v1::RuledCommand removeCrow;
    auto *removeDev = removeCrow.mutable_dev_command();
    removeDev->set_target_player_id(p1.myId);
    removeDev->mutable_move_card()->set_card_name("Storm Crow");
    removeDev->mutable_move_card()->set_zone(ruled::v1::DEV_ZONE_EXILE);
    for (int i = 0; i < 40; ++i)
        ASSERT_TRUE(sendAndPump(p1, removeCrow, QStringLiteral("prepare land-only library")));
    ASSERT_TRUE(devPutBattlefield(p1.myId, "Spyglass Siren"));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    const auto *nextMap = findPermanent(p1, p1.myId, QStringLiteral("map"));
    ASSERT_NE(nextMap, nullptr);
    ability->set_source_object_id(nextMap->oid);
    ability->set_expected_zone_change_generation(nextMap->generation);
    ASSERT_TRUE(sendAndPump(p1, addMana, QStringLiteral("add mana for land explore")));
    ASSERT_TRUE(sendAndPump(p1, activate, QStringLiteral("activate Map for land explore")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_FALSE(p1.pendingChoice.has_value());
    for (const OpeningDriver *client : {&p1, &p2}) {
        const auto reveal = std::find_if(client->revealEvents.begin(), client->revealEvents.end(),
                                         [](const ruled::v1::CardsRevealed &event) {
                                             return event.cards_size() == 1 && event.cards(0).card_name() == "Forest";
                                         });
        ASSERT_NE(reveal, client->revealEvents.end());
        EXPECT_FALSE(reveal->reveal_id().empty());
        EXPECT_EQ(reveal->source_zone(), ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY);
        EXPECT_EQ(reveal->zone_owner_player_id(), p1.myId);
        EXPECT_EQ(findPermanent(*client, p1.myId, QStringLiteral("spyglass_siren"))->power, 2);
    }
}

TEST_F(RuledE2ESmokeTest, TidebinderCountersTriggeredAbilityThroughExistingStackTargetContract)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    OpeningDriver p1(true, QStringLiteral("tidebinderp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("tidebinderp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Tidebinder game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Tidebinder game start (p2)"));
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

    auto sendAndPump = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const quint64 p1Version = p1.stateVersion;
        const quint64 p2Version = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 && (p1.stateVersion <= p1Version || p2.stateVersion <= p2Version)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > p1Version && p2.stateVersion > p2Version;
    };
    auto devPut = [&](int player, const char *cardName) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(player);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(true);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for Tidebinder").arg(cardName));
    };
    auto passPriority = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority for Tidebinder"));
    };
    auto findPermanent = [](const OpeningDriver &client, int controller,
                            const QString &cardId) -> const OpeningDriver::Permanent * {
        const auto player = client.battlefieldByPlayer.find(controller);
        if (player == client.battlefieldByPlayer.end()) {
            return nullptr;
        }
        const auto permanent = std::find_if(player->second.begin(), player->second.end(),
                                            [&](const auto &candidate) { return candidate.cardId == cardId; });
        return permanent == player->second.end() ? nullptr : &*permanent;
    };

    ASSERT_TRUE(devPut(p1.myId, "Grizzly Bears"));
    ASSERT_TRUE(devPut(p1.myId, "The Wondrous Wasp"));
    const auto *bear = findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"));
    ASSERT_NE(bear, nullptr);
    ASSERT_TRUE(p1.pendingTriggerTarget.has_value());
    ASSERT_EQ(p1.pendingTriggerTarget->targets().groups_size(), 1);
    const auto &waspGroup = p1.pendingTriggerTarget->targets().groups(0);
    ASSERT_TRUE(std::find(waspGroup.valid_permanent_ids().begin(), waspGroup.valid_permanent_ids().end(), bear->oid) !=
                waspGroup.valid_permanent_ids().end());

    ruled::v1::RuledCommand chooseWaspTarget;
    auto *waspTarget = chooseWaspTarget.mutable_choose_trigger_target()->add_targets();
    waspTarget->set_object_id(bear->oid);
    waspTarget->set_group_index(waspGroup.group_index());
    waspTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    p1.pendingTriggerTarget.reset();
    ASSERT_TRUE(sendAndPump(p1, chooseWaspTarget, QStringLiteral("choose Wasp trigger target")));

    ASSERT_TRUE(devPut(p2.myId, "Tishana's Tidebinder"));
    ASSERT_TRUE(p2.pendingTriggerTarget.has_value());
    ASSERT_EQ(p2.pendingTriggerTarget->targets().groups_size(), 1);
    const auto &tidebinderGroup = p2.pendingTriggerTarget->targets().groups(0);
    ASSERT_EQ(tidebinderGroup.valid_stack_ids_size(), 1);
    const quint32 waspAbilityId = tidebinderGroup.valid_stack_ids(0);
    EXPECT_EQ(tidebinderGroup.valid_permanent_ids_size(), 0);

    ruled::v1::RuledCommand chooseAbilityTarget;
    auto *abilityTarget = chooseAbilityTarget.mutable_choose_trigger_target()->add_targets();
    abilityTarget->set_object_id(waspAbilityId);
    abilityTarget->set_group_index(tidebinderGroup.group_index());
    abilityTarget->set_kind(ruled::v1::TARGET_REF_KIND_STACK);
    p2.pendingTriggerTarget.reset();
    ASSERT_TRUE(sendAndPump(p2, chooseAbilityTarget, QStringLiteral("choose Wasp triggered ability")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    for (const OpeningDriver *client : {&p1, &p2}) {
        EXPECT_TRUE(client->counteredStackObjectIds.count(waspAbilityId))
            << "the counter event did not reach both ruled clients";
        const auto *wasp = findPermanent(*client, p1.myId, QStringLiteral("the_wondrous_wasp"));
        const auto *untappedBear = findPermanent(*client, p1.myId, QStringLiteral("grizzly_bears"));
        ASSERT_NE(wasp, nullptr);
        ASSERT_NE(untappedBear, nullptr);
        EXPECT_FALSE(wasp->flying) << "Tidebinder did not remove the triggered ability source's abilities";
        EXPECT_FALSE(untappedBear->tapped) << "the countered Wasp trigger still resolved";
    }
}

TEST_F(RuledE2ESmokeTest, EsperOriginsFlashbackReturnsTransformedRevealsPubliclyAndFinalityExiles)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("esperp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("esperp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Esper Origins game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Esper Origins game start (p2)"));
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
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    auto send = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
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
    auto passCurrentPriority = [&] {
        ruled::v1::RuledCommand pass;
        pass.mutable_pass_priority();
        if (p1.priorityPlayer == p1.myId) {
            return send(p1, pass, QStringLiteral("Esper Origins pass priority"));
        }
        if (p1.priorityPlayer == p2.myId) {
            return send(p2, pass, QStringLiteral("Esper Origins pass priority"));
        }
        return false;
    };

    ruled::v1::RuledCommand conjure;
    auto *put = conjure.mutable_dev_command()->mutable_put_card_in_zone();
    conjure.mutable_dev_command()->set_target_player_id(p1.myId);
    put->set_card_name("Esper Origins // Summon: Esper Maduin");
    put->set_zone(ruled::v1::DEV_ZONE_HAND);
    ASSERT_TRUE(send(p1, conjure, QStringLiteral("put Esper Origins in hand")));

    ruled::v1::RuledCommand bury;
    bury.mutable_dev_command()->set_target_player_id(p1.myId);
    auto *buryMove = bury.mutable_dev_command()->mutable_move_card();
    buryMove->set_card_name("Esper Origins // Summon: Esper Maduin");
    buryMove->set_zone(ruled::v1::DEV_ZONE_GRAVEYARD);
    ASSERT_TRUE(send(p1, bury, QStringLiteral("move Esper Origins to graveyard")));

    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_g(1);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(3);
    ASSERT_TRUE(send(p1, mana, QStringLiteral("add flashback mana for Esper Origins")));

    const auto action = std::find_if(p1.latestLegal.zone_cast_actions().begin(),
                                     p1.latestLegal.zone_cast_actions().end(), [](const auto &candidate) {
                                         return candidate.card_name() == "Esper Origins" &&
                                                candidate.source_zone() == ruled::v1::CAST_SOURCE_ZONE_GRAVEYARD;
                                     });
    ASSERT_NE(action, p1.latestLegal.zone_cast_actions().end());
    const quint32 esperOid = action->object_id();

    ruled::v1::RuledCommand cast;
    auto *spell = cast.mutable_cast_spell();
    spell->set_cast_method(action->cast_method());
    spell->mutable_source()->set_graveyard_object_id(esperOid);
    spell->mutable_source()->set_expected_zone_change_generation(action->zone_change_generation());
    ASSERT_TRUE(send(p1, cast, QStringLiteral("flashback Esper Origins")));
    ASSERT_TRUE(passCurrentPriority());
    ASSERT_TRUE(passCurrentPriority());
    ASSERT_TRUE(p1.pendingChoice.has_value());
    ASSERT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    ASSERT_EQ(p1.pendingChoice->candidate_object_ids_size(), 2);

    ruled::v1::RuledCommand surveil;
    for (const quint32 oid : p1.pendingChoice->candidate_object_ids()) {
        surveil.mutable_submit_resolution_choice()->add_chosen_object_ids(oid);
    }
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, surveil, QStringLiteral("put both surveilled cards into graveyard")));

    const auto transformed = [&]() -> const OpeningDriver::Permanent * {
        const auto battlefield = p1.battlefieldByPlayer.find(p1.myId);
        if (battlefield == p1.battlefieldByPlayer.end()) {
            return nullptr;
        }
        const auto found = std::find_if(battlefield->second.begin(), battlefield->second.end(),
                                        [esperOid](const auto &permanent) { return permanent.oid == esperOid; });
        return found == battlefield->second.end() ? nullptr : &*found;
    };
    ASSERT_NE(transformed(), nullptr);
    EXPECT_EQ(transformed()->faceIndex, 1);
    EXPECT_TRUE(transformed()->creature);
    EXPECT_TRUE(transformed()->countersAnnotation.contains(QStringLiteral("1 finality counter(s)")));
    EXPECT_TRUE(transformed()->countersAnnotation.contains(QStringLiteral("1 lore counter(s)")));

    for (int passes = 0; passes < 6 && p1.stackDepth > 0; ++passes) {
        ASSERT_TRUE(passCurrentPriority());
    }
    ASSERT_EQ(p1.stackDepth, 0);

    const auto sawForestReveal = [](const OpeningDriver &client) {
        return std::any_of(client.revealEvents.begin(), client.revealEvents.end(),
                           [](const ruled::v1::CardsRevealed &event) {
                               return event.source_zone() == ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_LIBRARY &&
                                      event.cards_size() == 1 && event.cards(0).card_name() == "Forest";
                           });
    };
    EXPECT_TRUE(sawForestReveal(p1));
    EXPECT_TRUE(sawForestReveal(p2));

    const auto findMove = [](const OpeningDriver &client, const char *from, const char *to, int cardId) {
        return std::find_if(client.physicalMoveEvents.begin(), client.physicalMoveEvents.end(),
                            [from, to, cardId](const Event_MoveCard &move) {
                                return move.start_zone() == from && move.target_zone() == to &&
                                       (cardId < 0 || move.card_id() == cardId);
                            });
    };
    const auto graveToStack = findMove(p1, ZoneNames::GRAVE, ZoneNames::STACK, -1);
    ASSERT_NE(graveToStack, p1.physicalMoveEvents.end());
    const int physicalId = graveToStack->new_card_id();
    const auto stackToExile = findMove(p1, ZoneNames::STACK, ZoneNames::EXILE, physicalId);
    ASSERT_NE(stackToExile, p1.physicalMoveEvents.end());
    EXPECT_EQ(stackToExile->new_card_id(), physicalId);
    const auto exileToTable = findMove(p1, ZoneNames::EXILE, ZoneNames::TABLE, physicalId);
    ASSERT_NE(exileToTable, p1.physicalMoveEvents.end());
    EXPECT_EQ(exileToTable->new_card_id(), physicalId);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(esperOid));
    EXPECT_EQ(p1.serverCardByEngineOid[esperOid], physicalId);

    ruled::v1::RuledCommand die;
    die.mutable_dev_command()->set_target_player_id(p1.myId);
    auto *move = die.mutable_dev_command()->mutable_move_card();
    move->set_card_name("Esper Origins // Summon: Esper Maduin");
    move->set_zone(ruled::v1::DEV_ZONE_GRAVEYARD);
    ASSERT_TRUE(send(p1, die, QStringLiteral("move finality permanent toward graveyard")));
    const auto tableToExile = findMove(p1, ZoneNames::TABLE, ZoneNames::EXILE, physicalId);
    ASSERT_NE(tableToExile, p1.physicalMoveEvents.end());
    EXPECT_EQ(tableToExile->new_card_id(), physicalId);
    EXPECT_EQ(transformed(), nullptr);
}

TEST_F(RuledE2ESmokeTest, FlowStateSelectsPrivateCardsAndPreservesChosenBottomOrder)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    OpeningDriver p1(true, QStringLiteral("flowp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("flowp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    // Three cards remain after the starting player's opening hand. Duplicate names must
    // retain distinct identities through both the picker and the physical hand moves.
    ASSERT_TRUE(p1.selectDeck(deckXml({{10, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Flow State start p1"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Flow State start p2"));
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
    auto send = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command) {
        const auto before1 = p1.stateVersion;
        const auto before2 = p2.stateVersion;
        sender.sendRuled(command, QStringLiteral("Flow State scenario"));
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto putHand = [&](const char *name) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        dev->mutable_put_card_in_zone()->set_zone(ruled::v1::DEV_ZONE_HAND);
        return send(p1, command);
    };
    auto pass = [&](OpeningDriver &sender) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(sender, command);
    };
    auto choose = [&](const std::vector<quint32> &oids) {
        ruled::v1::RuledCommand command;
        for (quint32 oid : oids) {
            command.mutable_submit_resolution_choice()->add_chosen_object_ids(oid);
        }
        p1.pendingChoice.reset();
        return send(p1, command);
    };
    auto observerIsPrivate = [&] {
        if (!p2.lastResolutionChoice.has_value()) {
            return false;
        }
        const auto &choice = *p2.lastResolutionChoice;
        return choice.candidate_names_size() == 0 && choice.candidate_card_ids_size() == 0 &&
               choice.candidate_object_ids_size() == 0 && choice.candidate_server_card_ids_size() == 0 &&
               choice.candidate_selectable_size() == 0 && !choice.has_public_reveal();
    };
    std::vector<quint32> expectedBottom;
    std::set<int> selectedPhysicalIds;
    for (int required : {1, 2}) {
        ASSERT_TRUE(putHand("Flow State"));
        if (required == 2) {
            // The first Flow State already supplies the sorcery card.
            ASSERT_TRUE(putHand("Lightning Bolt"));
            ruled::v1::RuledCommand move;
            auto *dev = move.mutable_dev_command();
            dev->set_target_player_id(p1.myId);
            dev->mutable_move_card()->set_card_name("Lightning Bolt");
            dev->mutable_move_card()->set_zone(ruled::v1::DEV_ZONE_GRAVEYARD);
            ASSERT_TRUE(send(p1, move));
        }
        ruled::v1::RuledCommand mana;
        auto *dev = mana.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        dev->mutable_add_mana()->set_u(1);
        dev->mutable_add_mana()->set_c(1);
        ASSERT_TRUE(send(p1, mana));
        const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Flow State"));
        ASSERT_NE(action, nullptr);
        ruled::v1::RuledCommand cast;
        cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        cast.mutable_cast_spell()->mutable_source()->set_hand_index(action->hand_index());
        ASSERT_TRUE(send(p1, cast));
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        ASSERT_TRUE(p1.pendingChoice.has_value());
        const auto choice = *p1.pendingChoice;
        ASSERT_EQ(choice.choice_kind(), ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
        EXPECT_EQ(choice.min(), static_cast<unsigned>(required));
        EXPECT_EQ(choice.max(), static_cast<unsigned>(required));
        EXPECT_FALSE(choice.has_public_reveal());
        ASSERT_TRUE(observerIsPrivate());
        std::vector<quint32> selected;
        if (required == 1) {
            ASSERT_EQ(choice.candidate_object_ids_size(), 3);
            selected = {choice.candidate_object_ids(1)};
            expectedBottom = {choice.candidate_object_ids(2), choice.candidate_object_ids(0)};
        } else {
            ASSERT_EQ(choice.candidate_object_ids_size(), 2);
            EXPECT_EQ(choice.candidate_object_ids(0), expectedBottom[0]);
            EXPECT_EQ(choice.candidate_object_ids(1), expectedBottom[1]);
            selected = expectedBottom;
        }
        const int oldHandSize = p1.handSizeByPlayer[p1.myId];
        p1.physicalMoveEvents.clear();
        p2.physicalMoveEvents.clear();
        ASSERT_TRUE(choose(selected));
        EXPECT_EQ(p1.handSizeByPlayer[p1.myId], oldHandSize + required);
        // Hand OIDs and the other seat's hand-slot map are intentionally not published.
        // Verify the physical moves each recipient actually receives instead.
        for (const OpeningDriver *client : {&p1, &p2}) {
            int handMoves = 0;
            for (const auto &move : client->physicalMoveEvents) {
                if (move.start_zone() != ZoneNames::DECK || move.target_zone() != ZoneNames::HAND) {
                    continue;
                }
                ++handMoves;
                if (client == &p1) {
                    EXPECT_EQ(move.card_name(), "Forest");
                    EXPECT_TRUE(selectedPhysicalIds.insert(move.new_card_id()).second);
                    EXPECT_TRUE(std::any_of(p1.handServerCardBySlot.begin(), p1.handServerCardBySlot.end(),
                                            [&](const auto &entry) { return entry.second == move.new_card_id(); }));
                } else {
                    EXPECT_TRUE(move.card_name().empty());
                }
            }
            EXPECT_EQ(handMoves, required);
        }
        EXPECT_TRUE(p1.revealEvents.empty());
        EXPECT_TRUE(p2.revealEvents.empty());
        EXPECT_TRUE(p1.physicalRevealEvents.empty());
        EXPECT_TRUE(p2.physicalRevealEvents.empty());
        if (required == 1) {
            ASSERT_TRUE(p1.pendingChoice.has_value());
            EXPECT_TRUE(p1.pendingChoice->ordered());
            ASSERT_TRUE(observerIsPrivate());
            ASSERT_TRUE(choose(expectedBottom));
        }
        EXPECT_FALSE(p1.pendingChoice.has_value());
        EXPECT_EQ(p1.stackDepth, 0);
        EXPECT_EQ(p2.stackDepth, 0);
    }
    EXPECT_EQ(selectedPhysicalIds.size(), 3u);
    EXPECT_TRUE(p1.libraryDetailsStayedConcealed);
    EXPECT_TRUE(p2.libraryDetailsStayedConcealed);
}

} // namespace
} // namespace ruled_e2e
