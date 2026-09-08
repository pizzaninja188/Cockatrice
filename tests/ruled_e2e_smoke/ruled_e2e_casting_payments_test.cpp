#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"
namespace ruled_e2e
{
namespace
{
class WardDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    bool wardManaFlowActive = false;
    bool wardDiscardFlowActive = false;
    quint32 wardManaSpellOid = 0;
    quint32 wardDiscardSpellOid = 0;
    quint32 wardDiscardSourceOid = 0;
    quint32 wardDiscardChosenOid = 0;
    int wardDiscardChosenServerCardId = -1;
    int wardDiscardMovedServerCardId = -1;
    bool sawWardManaAnnotation = false;
    bool sawWardDiscardAnnotation = false;
    bool sawWardManaCountered = false;
    bool sawWardDiscardPrivateCandidates = false;
    bool sawWardDiscardObserverRedaction = false;
    bool sawWardDiscardCardMoved = false;
    bool sawWardDiscardPhysicalHandToGrave = false;
    bool sawWardDiscardSpellResolved = false;
    bool sawWardDiscardSourceToHand = false;
    void onPhysicalEvent(const GameEvent &ev) override
    {
        if (ev.HasExtension(Event_MoveCard::ext)) {
            const auto &mc = ev.GetExtension(Event_MoveCard::ext);
            const QString from = QString::fromStdString(mc.start_zone());
            const QString to = QString::fromStdString(mc.target_zone());

            // ZoneNames, not literals: Cockatrice's exile zone is spelled "rfg".
            const QLatin1String grave(ZoneNames::GRAVE);

            const QLatin1String hand(ZoneNames::HAND);
            const QLatin1String exile(ZoneNames::EXILE);

            if (wardDiscardFlowActive && from == hand && to == grave) {
                wardDiscardMovedServerCardId = mc.card_id();
                sawWardDiscardPhysicalHandToGrave = true;
            }
        }
    }
    void onRuledEvent(const ruled::v1::RuledEvent &ev) override
    {
        OpeningDriver::onRuledEvent(ev);
        if (ev.has_stack_pushed()) {
            const auto &sp = ev.stack_pushed();
            const QString cardId = QString::fromStdString(sp.card_id());
            if (cardId == QLatin1String("unsummon")) {
                if (wardManaFlowActive) {
                    wardManaSpellOid = sp.object_id();
                } else if (wardDiscardFlowActive) {
                    wardDiscardSpellOid = sp.object_id();
                }
            }
            if (sp.is_triggered() && sp.has_primary_presentation() && sp.primary_presentation().path_size() > 0 &&
                sp.primary_presentation().path(sp.primary_presentation().path_size() - 1).id() == "triggered_01") {
                if (wardManaFlowActive &&
                    sp.primary_presentation().card_id() == "dirgur_island_dragon_skimming_strike") {
                    sawWardManaAnnotation = true;
                }
                if (wardDiscardFlowActive && sp.primary_presentation().card_id() == "spectral_snatcher") {
                    sawWardDiscardAnnotation = true;
                }
            }
        }
        if (ev.has_stack_resolved()) {
            if (wardDiscardSpellOid != 0 && ev.stack_resolved().object_id() == wardDiscardSpellOid) {
                sawWardDiscardSpellResolved = true;
            }
        }
        if (ev.has_stack_object_countered()) {
            if (wardManaSpellOid != 0 && ev.stack_object_countered().object_id() == wardManaSpellOid) {
                sawWardManaCountered = true;
            }
        }
        if (ev.has_resolution_choice_required()) {
            const auto &rcr = ev.resolution_choice_required();
            if (wardDiscardFlowActive && rcr.choice_kind() == ruled::v1::CHOICE_KIND_HAND_CARDS) {
                if (rcr.deciding_player_id() == myId) {
                    bool sawBear = false;
                    for (int i = 0; i < rcr.candidate_names_size(); ++i) {
                        if (rcr.candidate_names(i) == "Grizzly Bears") {
                            sawBear = true;
                            wardDiscardChosenOid = rcr.candidate_object_ids(i);
                            wardDiscardChosenServerCardId = rcr.candidate_server_card_ids(i);
                        }
                    }
                    sawWardDiscardPrivateCandidates =
                        sawBear && rcr.min() == 0 && rcr.max() == 1 &&
                        rcr.candidate_object_ids_size() == rcr.candidate_names_size() &&
                        rcr.candidate_object_ids_size() == rcr.candidate_server_card_ids_size();
                } else {
                    sawWardDiscardObserverRedaction =
                        rcr.candidate_object_ids_size() == 0 && rcr.candidate_card_ids_size() == 0 &&
                        rcr.candidate_names_size() == 0 && rcr.candidate_server_card_ids_size() == 0 &&
                        rcr.prompt_text() == "Opponent is making a resolution choice.";
                }
            }
        }
        if (ev.has_permanent_moved()) {
            const auto &moved = ev.permanent_moved();
            if (wardDiscardFlowActive && moved.card_id() == "grizzly_bears" &&
                moved.destination() == ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD &&
                (wardDiscardChosenOid == 0 || moved.object_id() == wardDiscardChosenOid)) {
                wardDiscardChosenOid = moved.object_id();
                sawWardDiscardCardMoved = true;
            }
            if (wardDiscardSourceOid != 0 && moved.object_id() == wardDiscardSourceOid &&
                moved.destination() == ruled::v1::PermanentMoved::DESTINATION_HAND) {
                sawWardDiscardSourceToHand = true;
            }
        }
    }
};

TEST_F(RuledE2ESmokeTest, WardManaDeclineAndPrivateDiscardPayment)
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

    WardDriver p1(true, QStringLiteral("wardp1"), &transcript);
    WardDriver p2(false, QStringLiteral("wardp2"), &transcript);
    // This focused cohort needs only a kept opening hand before dev setup.

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Ward cohort game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Ward cohort game start (p2)"));
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

    auto sendAndPump = [&](WardDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
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
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for Ward cohort").arg(cardName));
    };
    auto devBlueMana = [&](int targetPlayer) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        dev->mutable_add_mana()->set_u(1);
        return sendAndPump(p1, command, QStringLiteral("dev: add blue mana for Ward cohort"));
    };
    auto passPriority = [&](WardDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in Ward cohort"));
    };
    auto legalPermanentTargets = [](const WardDriver &client, const ruled::v1::LegalHandAction &action) {
        std::vector<quint32> targets;
        const quint32 key = action.hand_index() << 8;
        const auto found = client.latestLegal.valid_targets_by_hand_slot().find(key);
        if (found == client.latestLegal.valid_targets_by_hand_slot().end()) {
            return targets;
        }
        for (const auto &group : found->second.groups()) {
            targets.insert(targets.end(), group.valid_permanent_ids().begin(), group.valid_permanent_ids().end());
        }
        return targets;
    };

    // Ward {2}: the targeting player sees the choice, declines, and both clients receive the
    // exact public counter event for the physical Unsummon.
    ASSERT_TRUE(devPut(p1.myId, "Dirgur Island Dragon // Skimming Strike", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(devPut(p2.myId, "Unsummon", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devBlueMana(p2.myId));
    p1.wardManaFlowActive = true;
    p2.wardManaFlowActive = true;
    ASSERT_TRUE(passPriority(p1));
    const auto *manaUnsummon = p2.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Unsummon"));
    ASSERT_NE(manaUnsummon, nullptr);
    const auto manaTargets = legalPermanentTargets(p2, *manaUnsummon);
    ASSERT_EQ(manaTargets.size(), 1u);
    const quint32 dirgurOid = manaTargets.front();
    ruled::v1::RuledCommand castManaWard;
    castManaWard.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    castManaWard.mutable_cast_spell()->mutable_source()->set_hand_index(manaUnsummon->hand_index());
    auto *manaTarget = castManaWard.mutable_cast_spell()->add_targets();
    manaTarget->set_object_id(dirgurOid);
    manaTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p2, castManaWard, QStringLiteral("cast Unsummon at Dirgur")));
    ASSERT_TRUE(passPriority(p2));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(p2.pendingChoice.has_value());
    EXPECT_EQ(p2.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    EXPECT_FALSE(p1.pendingChoice.has_value());
    ruled::v1::RuledCommand declineManaWard;
    declineManaWard.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_DECLINE);
    p2.pendingChoice.reset();
    ASSERT_TRUE(sendAndPump(p2, declineManaWard, QStringLiteral("decline Ward {2}")));
    EXPECT_TRUE(p1.sawWardManaAnnotation && p2.sawWardManaAnnotation);
    EXPECT_TRUE(p1.sawWardManaCountered && p2.sawWardManaCountered);
    EXPECT_NE(p1.wardManaSpellOid, 0u);
    EXPECT_EQ(p1.wardManaSpellOid, p2.wardManaSpellOid);
    p1.wardManaFlowActive = false;
    p2.wardManaFlowActive = false;

    // Ward—Discard a card: only the payer receives aligned hand identities. Paying discards the
    // chosen physical Bear, then the preserved Unsummon resolves and returns Spectral Snatcher.
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    ASSERT_TRUE(devPut(p1.myId, "Spectral Snatcher", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(devPut(p2.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devPut(p2.myId, "Unsummon", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devBlueMana(p2.myId));
    p1.wardDiscardFlowActive = true;
    p2.wardDiscardFlowActive = true;
    ASSERT_TRUE(passPriority(p1));
    const auto *discardUnsummon = p2.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Unsummon"));
    ASSERT_NE(discardUnsummon, nullptr);
    const auto discardTargets = legalPermanentTargets(p2, *discardUnsummon);
    const auto snatcher = std::find_if(discardTargets.begin(), discardTargets.end(),
                                       [dirgurOid](quint32 objectId) { return objectId != dirgurOid; });
    ASSERT_NE(snatcher, discardTargets.end());
    const quint32 snatcherOid = *snatcher;
    p1.wardDiscardSourceOid = snatcherOid;
    p2.wardDiscardSourceOid = snatcherOid;
    ruled::v1::RuledCommand castDiscardWard;
    castDiscardWard.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    castDiscardWard.mutable_cast_spell()->mutable_source()->set_hand_index(discardUnsummon->hand_index());
    auto *discardTarget = castDiscardWard.mutable_cast_spell()->add_targets();
    discardTarget->set_object_id(snatcherOid);
    discardTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p2, castDiscardWard, QStringLiteral("cast Unsummon at Spectral Snatcher")));
    ASSERT_TRUE(passPriority(p2));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(p2.pendingChoice.has_value());
    EXPECT_EQ(p2.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_HAND_CARDS);
    EXPECT_TRUE(p2.sawWardDiscardPrivateCandidates);
    EXPECT_TRUE(p1.sawWardDiscardObserverRedaction);
    ASSERT_NE(p2.wardDiscardChosenOid, 0u);
    ruled::v1::RuledCommand payDiscardWard;
    payDiscardWard.mutable_submit_resolution_choice()->add_chosen_object_ids(p2.wardDiscardChosenOid);
    p2.pendingChoice.reset();
    ASSERT_TRUE(sendAndPump(p2, payDiscardWard, QStringLiteral("discard Grizzly Bears to pay Ward")));
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    EXPECT_TRUE(p1.sawWardDiscardAnnotation && p2.sawWardDiscardAnnotation);
    EXPECT_TRUE(p1.sawWardDiscardPhysicalHandToGrave && p2.sawWardDiscardPhysicalHandToGrave);
    EXPECT_EQ(p2.wardDiscardMovedServerCardId, p2.wardDiscardChosenServerCardId);
    EXPECT_TRUE(p1.sawWardDiscardSpellResolved && p2.sawWardDiscardSpellResolved);
    EXPECT_TRUE(p1.sawWardDiscardSourceToHand && p2.sawWardDiscardSourceToHand);
    EXPECT_EQ(p1.wardDiscardSpellOid, p2.wardDiscardSpellOid);
}

TEST_F(RuledE2ESmokeTest, KickedAangSearchSlotsArePrivateAndResolveForBothSeats)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("aangp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("aangp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Aang game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Aang game start (p2)"));
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
    auto pass = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("Aang resolution pass"));
    };

    ruled::v1::RuledCommand conjure;
    auto *dev = conjure.mutable_dev_command();
    dev->set_target_player_id(p1.myId);
    auto *put = dev->mutable_put_card_in_zone();
    put->set_card_name("Aang's Journey");
    put->set_zone(ruled::v1::DEV_ZONE_HAND);
    ASSERT_TRUE(send(p1, conjure, QStringLiteral("put Aang's Journey in hand")));

    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(4);
    ASSERT_TRUE(send(p1, mana, QStringLiteral("add mana for kicked Aang")));

    const auto *aang = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Aang's Journey"));
    ASSERT_NE(aang, nullptr);
    ASSERT_EQ(aang->cost_choices().cast_cost_groups_size(), 1);
    const auto &group = aang->cost_choices().cast_cost_groups(0);
    ASSERT_EQ(group.options_size(), 1);
    ruled::v1::RuledCommand cast;
    cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast.mutable_cast_spell()->mutable_source()->set_hand_index(aang->hand_index());
    auto *kicker = cast.mutable_cast_spell()->add_cast_cost_group_selections();
    kicker->set_group_index(group.group_index());
    kicker->set_option_index(group.options(0).option_index());
    ASSERT_TRUE(send(p1, cast, QStringLiteral("cast kicked Aang's Journey")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));

    ASSERT_TRUE(p1.pendingChoice.has_value());
    const auto &ownChoice = *p1.pendingChoice;
    EXPECT_EQ(ownChoice.choice_kind(), ruled::v1::CHOICE_KIND_LIBRARY_SEARCH);
    ASSERT_EQ(ownChoice.selection_slots_size(), 2);
    EXPECT_EQ(ownChoice.selection_slots(0).label(), "Search choice (slot_01)");
    ASSERT_TRUE(ownChoice.selection_slots(0).has_presentation());
    EXPECT_EQ(ownChoice.selection_slots(0).presentation().card_id(), "aangs_journey");
    ASSERT_GT(ownChoice.selection_slots(0).presentation().path_size(), 0);
    EXPECT_EQ(ownChoice.selection_slots(0)
                  .presentation()
                  .path(ownChoice.selection_slots(0).presentation().path_size() - 1)
                  .id(),
              "slot_01");
    EXPECT_GT(ownChoice.selection_slots(0).candidate_indices_size(), 0);
    EXPECT_EQ(ownChoice.selection_slots(1).label(), "Search choice (slot_02)");
    ASSERT_TRUE(ownChoice.selection_slots(1).has_presentation());
    EXPECT_EQ(ownChoice.selection_slots(1).presentation().card_id(), "aangs_journey");
    ASSERT_GT(ownChoice.selection_slots(1).presentation().path_size(), 0);
    EXPECT_EQ(ownChoice.selection_slots(1)
                  .presentation()
                  .path(ownChoice.selection_slots(1).presentation().path_size() - 1)
                  .id(),
              "slot_02");
    EXPECT_EQ(ownChoice.selection_slots(1).candidate_indices_size(), 0);
    ASSERT_GT(ownChoice.candidate_object_ids_size(), 0);
    EXPECT_EQ(ownChoice.candidate_object_ids_size(), ownChoice.candidate_server_card_ids_size());
    EXPECT_FALSE(p2.pendingChoice.has_value());
    ASSERT_TRUE(p2.lastResolutionChoice.has_value());
    EXPECT_EQ(p2.lastResolutionChoice->candidate_object_ids_size(), 0);
    EXPECT_EQ(p2.lastResolutionChoice->candidate_names_size(), 0);
    EXPECT_EQ(p2.lastResolutionChoice->selection_slots_size(), 0);

    const quint32 chosen = ownChoice.candidate_object_ids(0);
    const int handBefore = p1.handSizeByPlayer[p1.myId];
    ruled::v1::RuledCommand choose;
    choose.mutable_submit_resolution_choice()->add_chosen_object_ids(chosen);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, choose, QStringLiteral("choose Aang basic-land slot")));
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], handBefore + 1);
    EXPECT_EQ(p1.lifeByPlayer[p1.myId], 22);
    EXPECT_EQ(p2.lifeByPlayer[p1.myId], 22);
    EXPECT_EQ(p1.stackDepth, 0);
    EXPECT_EQ(p2.stackDepth, 0);
}

TEST_F(RuledE2ESmokeTest, FinalShowdownLinksEveryModeToItsCostAndPublishesItsPermanentChoice)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("showdownp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("showdownp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Plains")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Final Showdown game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Final Showdown game start (p2)"));
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
    auto conjure = [&](const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        dev->mutable_put_card_in_zone()->set_zone(zone);
        return send(p1, command, QStringLiteral("conjure %1").arg(QString::fromUtf8(name)));
    };
    auto pass = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("Final Showdown resolution pass"));
    };

    ASSERT_TRUE(conjure("Wind Drake", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(conjure("Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(conjure("Final Showdown", ruled::v1::DEV_ZONE_HAND));

    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_w(3);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(5);
    ASSERT_TRUE(send(p1, mana, QStringLiteral("add mana for all Final Showdown modes")));

    const auto *showdown = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Final Showdown"));
    ASSERT_NE(showdown, nullptr);
    ASSERT_EQ(showdown->modes_size(), 3);
    ASSERT_EQ(showdown->cost_choices().cast_cost_groups_size(), 1);
    EXPECT_EQ(showdown->cost_choices().cast_cost_groups(0).min(), 1u);
    EXPECT_EQ(showdown->cost_choices().cast_cost_groups(0).max(), 3u);

    ruled::v1::RuledCommand cast;
    auto *spell = cast.mutable_cast_spell();
    spell->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    spell->mutable_source()->set_hand_index(showdown->hand_index());
    std::set<std::pair<quint32, quint32>> linkedCosts;
    for (const auto &mode : showdown->modes()) {
        ASSERT_TRUE(mode.has_linked_cast_cost());
        spell->add_selected_modes()->set_mode_index(mode.mode_index());
        linkedCosts.insert({mode.linked_cast_cost().group_index(), mode.linked_cast_cost().option_index()});
    }
    ASSERT_EQ(linkedCosts.size(), 3);
    for (const auto &[groupIndex, optionIndex] : linkedCosts) {
        auto *selection = spell->add_cast_cost_group_selections();
        selection->set_group_index(groupIndex);
        selection->set_option_index(optionIndex);
    }
    ASSERT_TRUE(send(p1, cast, QStringLiteral("cast Final Showdown with all modes")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));

    ASSERT_TRUE(p1.pendingChoice.has_value());
    EXPECT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
    EXPECT_EQ(QString::fromStdString(p1.pendingChoice->prompt_text()),
              QStringLiteral("Choose a creature you control."));
    EXPECT_EQ(p1.pendingChoice->candidate_object_ids_size(), 2);
    ASSERT_TRUE(p2.lastResolutionChoice.has_value());
    EXPECT_EQ(p2.lastResolutionChoice->choice_kind(), ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
    EXPECT_EQ(p2.lastResolutionChoice->candidate_object_ids_size(), 2)
        << "the battlefield choice is public and must not be relay-redacted";

    const auto own = p1.battlefieldByPlayer.find(p1.myId);
    ASSERT_NE(own, p1.battlefieldByPlayer.end());
    const auto drake = std::find_if(own->second.cbegin(), own->second.cend(), [](const auto &permanent) {
        return permanent.cardId == QStringLiteral("wind_drake");
    });
    ASSERT_NE(drake, own->second.cend());
    ruled::v1::RuledCommand choose;
    choose.mutable_submit_resolution_choice()->add_chosen_object_ids(drake->oid);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, choose, QStringLiteral("choose Wind Drake for indestructible")));

    const auto finalOwn = p1.battlefieldByPlayer.find(p1.myId);
    ASSERT_NE(finalOwn, p1.battlefieldByPlayer.end());
    const auto survivingDrake =
        std::find_if(finalOwn->second.cbegin(), finalOwn->second.cend(),
                     [](const auto &permanent) { return permanent.cardId == QStringLiteral("wind_drake"); });
    ASSERT_NE(survivingDrake, finalOwn->second.cend());
    EXPECT_FALSE(survivingDrake->flying) << "the first mode removes Flying through end of turn";
    EXPECT_TRUE(survivingDrake->indestructible)
        << "the later mode grants Indestructible after the ability-removal layer was established";
    EXPECT_EQ(p1.countOwn(QStringLiteral("grizzly_bears"), false), 0)
        << "the unprotected creature should be destroyed by the final mode";
}

TEST_F(RuledE2ESmokeTest, WateryGraveWaitsInHandUntilItsLifePaymentCompletes)
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

    OpeningDriver p1(true, QStringLiteral("waterygravep1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("waterygravep2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Watery Grave")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Watery Grave game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Watery Grave game start (p2)"));
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

    const auto *land = p1.handAction(ruled::v1::HAND_ACTION_PLAY_LAND, QStringLiteral("Watery Grave"));
    ASSERT_NE(land, nullptr);
    ASSERT_TRUE(p1.handServerCardBySlot.count(static_cast<int>(land->hand_index())));
    const int physicalCardId = p1.handServerCardBySlot[static_cast<int>(land->hand_index())];
    ruled::v1::RuledCommand playLand;
    playLand.mutable_play_land()->mutable_source()->set_hand_index(land->hand_index());
    ASSERT_TRUE(sendAndPump(p1, playLand, QStringLiteral("play Watery Grave")));

    ASSERT_TRUE(p1.pendingChoice.has_value());
    EXPECT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    EXPECT_EQ(p1.pendingChoice->min(), 0u);
    EXPECT_EQ(p1.pendingChoice->max(), 1u);
    ASSERT_EQ(p1.pendingChoice->resolution_branches_size(), 1);
    EXPECT_TRUE(p1.pendingChoice->resolution_branches(0).selectable());
    EXPECT_FALSE(p2.pendingChoice.has_value());
    ASSERT_TRUE(p2.lastResolutionChoice.has_value());
    EXPECT_EQ(p2.lastResolutionChoice->resolution_branches_size(), 0);
    EXPECT_EQ(p2.lastResolutionChoice->prompt_text(), "Opponent is making a resolution choice.");
    EXPECT_TRUE(p1.physicalRowAndPt.count(std::make_pair(p1.myId, physicalCardId)) == 0)
        << "the physical land must remain in hand while its entry cost is pending";
    const auto beforeEntry = p1.battlefieldByPlayer.find(p1.myId);
    EXPECT_TRUE(beforeEntry == p1.battlefieldByPlayer.end() ||
                std::none_of(beforeEntry->second.cbegin(), beforeEntry->second.cend(),
                             [](const auto &permanent) { return permanent.cardId == "watery_grave"; }));

    ruled::v1::RuledCommand payLife;
    payLife.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_SELECT_BRANCH);
    payLife.mutable_submit_resolution_choice()->set_selected_branch_index(0);
    p1.pendingChoice.reset();
    ASSERT_TRUE(sendAndPump(p1, payLife, QStringLiteral("pay 2 life for Watery Grave")));

    ASSERT_EQ(p1.lifeByPlayer[p1.myId], 18);
    ASSERT_EQ(p2.lifeByPlayer[p1.myId], 18);
    const auto findWateryGrave = [&](const OpeningDriver &client) {
        const auto found = client.battlefieldByPlayer.find(p1.myId);
        if (found == client.battlefieldByPlayer.end()) {
            return quint32{0};
        }
        const auto permanent = std::find_if(found->second.cbegin(), found->second.cend(),
                                            [](const auto &entry) { return entry.cardId == "watery_grave"; });
        return permanent == found->second.cend() ? quint32{0} : permanent->oid;
    };
    const quint32 wateryGraveOid = findWateryGrave(p1);
    ASSERT_NE(wateryGraveOid, 0u);
    ASSERT_EQ(findWateryGrave(p2), wateryGraveOid);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(wateryGraveOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(wateryGraveOid));
    EXPECT_EQ(p1.serverCardByEngineOid[wateryGraveOid], physicalCardId);
    EXPECT_EQ(p2.serverCardByEngineOid[wateryGraveOid], physicalCardId);
    EXPECT_EQ(p1.physicalRowAndPt[std::make_pair(p1.myId, physicalCardId)].first, 2);
    EXPECT_EQ(p2.physicalRowAndPt[std::make_pair(p1.myId, physicalCardId)].first, 2);
    const auto &permanents = p1.battlefieldByPlayer[p1.myId];
    const auto entered = std::find_if(permanents.cbegin(), permanents.cend(),
                                      [&](const auto &permanent) { return permanent.oid == wateryGraveOid; });
    ASSERT_NE(entered, permanents.cend());
    EXPECT_FALSE(entered->tapped);
}

class BeholdDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    bool optionalCastCostFlowActive = false;
    bool sawPrivateBeholdCandidates = false;
    bool sawBeholdCandidateRedaction = false;
    bool sawBeholdStackReceipt = false;
    bool sawActiveBeholdReveal = false;
    bool sawActiveBeholdRevealClosed = false;
    bool activeBeholdReveal = false;
    void onRuledEvent(const ruled::v1::RuledEvent &ev) override
    {
        OpeningDriver::onRuledEvent(ev);
        if (ev.has_stack_pushed()) {
            const auto &sp = ev.stack_pushed();
            const QString cardId = QString::fromStdString(sp.card_id());
            if (cardId == QLatin1String("caustic_exhale")) {
                sawBeholdStackReceipt =
                    std::any_of(sp.chosen_cast_cost_presentations().begin(), sp.chosen_cast_cost_presentations().end(),
                                [](const auto &presentation) {
                                    return presentation.card_id() == "caustic_exhale" &&
                                           presentation.path_size() >= 3 &&
                                           presentation.path(presentation.path_size() - 1).kind() ==
                                               ruled::v1::PRESENTATION_PATH_KIND_CAST_COST_OPTION &&
                                           presentation.path(presentation.path_size() - 1).id() == "option_01";
                                });
            }
        }
        if (ev.has_active_public_reveal_snapshot()) {
            bool hasDragon = false;
            for (const auto &reveal : ev.active_public_reveal_snapshot().reveals()) {
                hasDragon =
                    hasDragon || (reveal.cards_size() == 1 && reveal.cards(0).card_id() == "adult_gold_dragon" &&
                                  reveal.cards(0).card_name() == "Adult Gold Dragon");
            }
            if (hasDragon) {
                sawActiveBeholdReveal = true;
                activeBeholdReveal = true;
            } else if (activeBeholdReveal) {
                activeBeholdReveal = false;
                sawActiveBeholdRevealClosed = true;
            }
        }
    }
    void onBatchEventsComplete(const ruled::v1::RuledEventBatch &batch) override
    {
        OpeningDriver::onBatchEventsComplete(batch);
        if (optionalCastCostFlowActive && !starts && oppId >= 0) {
            sawBeholdCandidateRedaction =
                sawBeholdCandidateRedaction || batch.legal_by_player().find(oppId) == batch.legal_by_player().end();
        }
    }
    void onLegalActions(const ruled::v1::RuledEventBatch &) override
    {
        if (optionalCastCostFlowActive && starts) {
            const auto caustic = std::find_if(
                latestLegal.hand_actions().begin(), latestLegal.hand_actions().end(), [](const auto &action) {
                    return action.kind() == ruled::v1::HAND_ACTION_CAST_SPELL && action.card_name() == "Caustic Exhale";
                });
            if (caustic != latestLegal.hand_actions().end() && caustic->cost_choices().cast_cost_groups_size() == 1) {
                const auto &group = caustic->cost_choices().cast_cost_groups(0);
                sawPrivateBeholdCandidates =
                    group.options_size() == 2 && group.options(0).kind() == ruled::v1::CAST_COST_OPTION_KIND_BEHOLD &&
                    group.options(0).selectable() && group.options(0).valid_hand_indices_size() == 1;
            }
        }
    }
};

TEST_F(RuledE2ESmokeTest, BeholdCastCostIsPrivateUntilItsPublicStackReveal)
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

    BeholdDriver p1(true, QStringLiteral("beholdp1"), &transcript);
    BeholdDriver p2(false, QStringLiteral("beholdp2"), &transcript);

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Swamp")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Behold cohort game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Behold cohort game start (p2)"));
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

    auto sendAndPump = [&](BeholdDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
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
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for Behold cohort").arg(cardName));
    };
    auto passPriority = [&](BeholdDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in Behold cohort"));
    };

    ASSERT_TRUE(devPut(p1.myId, "Caustic Exhale", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devPut(p1.myId, "Adult Gold Dragon", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devPut(p2.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    p1.optionalCastCostFlowActive = true;
    p2.optionalCastCostFlowActive = true;
    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_b(1);
    ASSERT_TRUE(sendAndPump(p1, addMana, QStringLiteral("dev: add black mana for Behold cohort")));
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    const auto *caustic = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Caustic Exhale"));
    ASSERT_NE(caustic, nullptr);
    ASSERT_EQ(caustic->cost_choices().cast_cost_groups_size(), 1);
    const auto &group = caustic->cost_choices().cast_cost_groups(0);
    ASSERT_GE(group.options_size(), 1);
    const auto &behold = group.options(0);
    ASSERT_EQ(behold.kind(), ruled::v1::CAST_COST_OPTION_KIND_BEHOLD);
    ASSERT_EQ(behold.valid_hand_indices_size(), 1);
    ASSERT_TRUE(p1.sawPrivateBeholdCandidates);
    ASSERT_TRUE(p2.sawBeholdCandidateRedaction);

    const quint32 targetKey = caustic->hand_index() << 8;
    const auto targetGroups = p1.latestLegal.valid_targets_by_hand_slot().find(targetKey);
    ASSERT_NE(targetGroups, p1.latestLegal.valid_targets_by_hand_slot().end());
    ASSERT_EQ(targetGroups->second.groups_size(), 1);
    ASSERT_EQ(targetGroups->second.groups(0).valid_permanent_ids_size(), 1);

    ruled::v1::RuledCommand cast;
    cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast.mutable_cast_spell()->mutable_source()->set_hand_index(caustic->hand_index());
    auto *selection = cast.mutable_cast_spell()->add_cast_cost_group_selections();
    selection->set_group_index(group.group_index());
    selection->set_option_index(behold.option_index());
    selection->set_hand_index(behold.valid_hand_indices(0));
    auto *target = cast.mutable_cast_spell()->add_targets();
    target->set_object_id(targetGroups->second.groups(0).valid_permanent_ids(0));
    target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p1, cast, QStringLiteral("cast Caustic Exhale by beholding a Dragon")));

    EXPECT_TRUE(p1.sawBeholdStackReceipt && p2.sawBeholdStackReceipt);
    EXPECT_TRUE(p1.sawActiveBeholdReveal && p2.sawActiveBeholdReveal);
    EXPECT_TRUE(p1.activeBeholdReveal && p2.activeBeholdReveal);

    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_TRUE(p1.sawActiveBeholdRevealClosed && p2.sawActiveBeholdRevealClosed);
    EXPECT_FALSE(p1.activeBeholdReveal || p2.activeBeholdReveal);
}

TEST_F(RuledE2ESmokeTest, BitterTriumphPaysEitherLifeOrAnExactPrivateHandCard)
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

    OpeningDriver p1(true, QStringLiteral("triumphp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("triumphp2"), &transcript);

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Swamp")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Bitter Triumph game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Bitter Triumph game start (p2)"));
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
    auto devPut = [&](int targetPlayer, const char *cardName, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(zone);
        put->set_ready(ready);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for Bitter Triumph").arg(cardName));
    };
    auto passPriority = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in Bitter Triumph flow"));
    };
    auto optionOfKind = [](const ruled::v1::LegalCastCostGroup &group, ruled::v1::CastCostOptionKind kind) {
        return std::find_if(group.options().begin(), group.options().end(),
                            [kind](const auto &option) { return option.kind() == kind; });
    };

    ASSERT_TRUE(devPut(p1.myId, "Bitter Triumph", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devPut(p1.myId, "Bitter Triumph", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devPut(p2.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(devPut(p2.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_b(2);
    addMana.mutable_dev_command()->mutable_add_mana()->set_c(2);
    ASSERT_TRUE(sendAndPump(p1, addMana, QStringLiteral("dev: add mana for both Bitter Triumph casts")));

    const auto *first = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Bitter Triumph"));
    ASSERT_NE(first, nullptr);
    ASSERT_EQ(first->cost_choices().cast_cost_groups_size(), 1);
    const auto &firstGroup = first->cost_choices().cast_cost_groups(0);
    ASSERT_EQ(firstGroup.min(), 1u);
    ASSERT_EQ(firstGroup.max(), 1u);
    const auto payLife = optionOfKind(firstGroup, ruled::v1::CAST_COST_OPTION_KIND_PAY_LIFE);
    const auto discard = optionOfKind(firstGroup, ruled::v1::CAST_COST_OPTION_KIND_DISCARD_CARD);
    ASSERT_NE(payLife, firstGroup.options().end());
    ASSERT_NE(discard, firstGroup.options().end());
    ASSERT_TRUE(payLife->selectable());
    ASSERT_TRUE(discard->selectable());
    ASSERT_GT(discard->valid_hand_indices_size(), 0);
    EXPECT_EQ(p2.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Bitter Triumph")), nullptr);

    const quint32 firstTargetKey = first->hand_index() << 8;
    const auto firstTargets = p1.latestLegal.valid_targets_by_hand_slot().find(firstTargetKey);
    ASSERT_NE(firstTargets, p1.latestLegal.valid_targets_by_hand_slot().end());
    ASSERT_GE(firstTargets->second.groups(0).valid_permanent_ids_size(), 2);
    const quint32 firstTarget = firstTargets->second.groups(0).valid_permanent_ids(0);
    ruled::v1::RuledCommand lifeCast;
    auto *lifeSpell = lifeCast.mutable_cast_spell();
    lifeSpell->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    lifeSpell->mutable_source()->set_hand_index(first->hand_index());
    auto *lifeSelection = lifeSpell->add_cast_cost_group_selections();
    lifeSelection->set_group_index(firstGroup.group_index());
    lifeSelection->set_option_index(payLife->option_index());
    auto *lifeTarget = lifeSpell->add_targets();
    lifeTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    lifeTarget->set_object_id(firstTarget);
    ASSERT_TRUE(sendAndPump(p1, lifeCast, QStringLiteral("cast Bitter Triumph by paying 3 life")));
    ASSERT_EQ(p1.lifeByPlayer[p1.myId], 17);
    ASSERT_EQ(p2.lifeByPlayer[p1.myId], 17);
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    ASSERT_EQ(p1.graveyardOwnerByEngineOid[firstTarget], p2.myId);
    ASSERT_EQ(p2.graveyardOwnerByEngineOid[firstTarget], p2.myId);

    const auto *second = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Bitter Triumph"));
    ASSERT_NE(second, nullptr);
    const auto &secondGroup = second->cost_choices().cast_cost_groups(0);
    const auto secondDiscard = optionOfKind(secondGroup, ruled::v1::CAST_COST_OPTION_KIND_DISCARD_CARD);
    ASSERT_NE(secondDiscard, secondGroup.options().end());
    ASSERT_GT(secondDiscard->valid_hand_indices_size(), 0);
    const int discardSlot = static_cast<int>(secondDiscard->valid_hand_indices(0));
    ASSERT_TRUE(p1.handServerCardBySlot.count(discardSlot));
    const int discardedServerCard = p1.handServerCardBySlot[discardSlot];
    const int handBeforeDiscardCast = p1.handSizeByPlayer[p1.myId];
    const quint32 secondTargetKey = second->hand_index() << 8;
    const auto secondTargets = p1.latestLegal.valid_targets_by_hand_slot().find(secondTargetKey);
    ASSERT_NE(secondTargets, p1.latestLegal.valid_targets_by_hand_slot().end());
    ASSERT_EQ(secondTargets->second.groups(0).valid_permanent_ids_size(), 1);
    const quint32 secondTarget = secondTargets->second.groups(0).valid_permanent_ids(0);

    ruled::v1::RuledCommand discardCast;
    auto *discardSpell = discardCast.mutable_cast_spell();
    discardSpell->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    discardSpell->mutable_source()->set_hand_index(second->hand_index());
    auto *discardSelection = discardSpell->add_cast_cost_group_selections();
    discardSelection->set_group_index(secondGroup.group_index());
    discardSelection->set_option_index(secondDiscard->option_index());
    discardSelection->set_hand_index(static_cast<quint32>(discardSlot));
    auto *discardTarget = discardSpell->add_targets();
    discardTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    discardTarget->set_object_id(secondTarget);
    ASSERT_TRUE(sendAndPump(p1, discardCast, QStringLiteral("cast Bitter Triumph by discarding a card")));
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], handBeforeDiscardCast - 2);
    const auto discarded =
        std::find_if(p1.graveyardOwnerByEngineOid.begin(), p1.graveyardOwnerByEngineOid.end(), [&](const auto &entry) {
            return entry.second == p1.myId && p1.serverCardByEngineOid[entry.first] == discardedServerCard;
        });
    ASSERT_NE(discarded, p1.graveyardOwnerByEngineOid.end());
    const quint32 discardedOid = discarded->first;
    ASSERT_EQ(p2.graveyardOwnerByEngineOid[discardedOid], p1.myId);
    ASSERT_EQ(p2.serverCardByEngineOid[discardedOid], discardedServerCard);
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_EQ(p1.graveyardOwnerByEngineOid[secondTarget], p2.myId);
    EXPECT_EQ(p2.graveyardOwnerByEngineOid[secondTarget], p2.myId);
}

TEST_F(RuledE2ESmokeTest, TappedTargetReductionIsPrivateAndAuthoritativeThroughBothClients)
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

    OpeningDriver p1(true, QStringLiteral("reductionp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("reductionp2"), &transcript);

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Plains")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Swamp")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "target reduction game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "target reduction game start (p2)"));
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
    auto devPut = [&](int targetPlayer, const char *cardName, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(zone);
        put->set_ready(ready);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for target reduction").arg(cardName));
    };
    auto passPriority = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in target reduction cohort"));
    };

    ASSERT_TRUE(devPut(p1.myId, "Luminous Rebuke", ruled::v1::DEV_ZONE_HAND, false));
    // Diregraf Ghoul's intrinsic replacement makes it a naturally tapped creature target.
    ASSERT_TRUE(devPut(p2.myId, "Diregraf Ghoul", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_w(1);
    addMana.mutable_dev_command()->mutable_add_mana()->set_c(1);
    ASSERT_TRUE(sendAndPump(p1, addMana, QStringLiteral("dev: add {1}{W} for target reduction")));

    const auto *rebuke = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Luminous Rebuke"));
    ASSERT_NE(rebuke, nullptr);
    EXPECT_EQ(rebuke->generic_cost_reduction(), 0u);
    EXPECT_EQ(p2.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Luminous Rebuke")), nullptr);
    const quint32 targetKey = rebuke->hand_index() << 8;
    const auto targets = p1.latestLegal.valid_targets_by_hand_slot().find(targetKey);
    ASSERT_NE(targets, p1.latestLegal.valid_targets_by_hand_slot().end());
    ASSERT_EQ(targets->second.groups_size(), 1);
    ASSERT_EQ(targets->second.groups(0).valid_permanent_ids_size(), 1);
    ASSERT_EQ(targets->second.targeted_cost_reduction_applications_size(), 1);
    const quint32 ghoulOid = targets->second.groups(0).valid_permanent_ids(0);
    const auto &reduction = targets->second.targeted_cost_reduction_applications(0);
    EXPECT_EQ(reduction.generic_mana(), 3u);
    ASSERT_EQ(reduction.qualifying_targets_size(), 1);
    EXPECT_EQ(reduction.qualifying_targets(0).kind(), ruled::v1::TARGET_REF_KIND_PERMANENT);
    EXPECT_EQ(reduction.qualifying_targets(0).object_id(), ghoulOid);

    ruled::v1::RuledCommand cast;
    cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast.mutable_cast_spell()->mutable_source()->set_hand_index(rebuke->hand_index());
    auto *target = cast.mutable_cast_spell()->add_targets();
    target->set_object_id(ghoulOid);
    target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p1, cast, QStringLiteral("cast Luminous Rebuke for {1}{W}")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
}

class HarmonizeDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    bool harmonizeFlowActive = false;
    bool normalWhisperFlowActive = false;
    bool sawHarmonizeGraveToStack = false;
    bool sawHarmonizeStackToExile = false;
    bool sawHarmonizeStackReceipt = false;
    bool sawNormalWhisperHandToStack = false;
    bool sawNormalWhisperStackToGrave = false;
    bool harmonizePhysicalIdentityContinuous = true;
    int harmonizePhysicalCardId = -1;
    quint32 harmonizeCreatureOid = 0;
    void onPhysicalEvent(const GameEvent &ev) override
    {
        if (ev.HasExtension(Event_MoveCard::ext)) {
            const auto &mc = ev.GetExtension(Event_MoveCard::ext);
            const QString from = QString::fromStdString(mc.start_zone());
            const QString to = QString::fromStdString(mc.target_zone());
            const QString name = QString::fromStdString(mc.card_name());
            // ZoneNames, not literals: Cockatrice's exile zone is spelled "rfg".
            const QLatin1String grave(ZoneNames::GRAVE);
            const QLatin1String stack(ZoneNames::STACK);
            const QLatin1String hand(ZoneNames::HAND);
            const QLatin1String exile(ZoneNames::EXILE);

            if (name == QLatin1String("Unending Whisper")) {
                if (harmonizeFlowActive && from == grave && to == stack) {
                    harmonizePhysicalIdentityContinuous = harmonizePhysicalCardId >= 0 &&
                                                          mc.card_id() == harmonizePhysicalCardId &&
                                                          mc.new_card_id() == harmonizePhysicalCardId;
                    harmonizePhysicalCardId = mc.new_card_id();
                    sawHarmonizeGraveToStack = true;
                } else if (harmonizeFlowActive && from == stack && to == exile) {
                    harmonizePhysicalIdentityContinuous =
                        harmonizePhysicalIdentityContinuous && harmonizePhysicalCardId >= 0 &&
                        mc.card_id() == harmonizePhysicalCardId && mc.new_card_id() == harmonizePhysicalCardId;
                    harmonizePhysicalCardId = mc.new_card_id();
                    sawHarmonizeStackToExile = true;
                } else if (normalWhisperFlowActive && from == hand && to == stack) {
                    sawNormalWhisperHandToStack = true;
                } else if (normalWhisperFlowActive && from == stack && to == grave) {
                    sawNormalWhisperStackToGrave = true;
                }
            }
        }
    }
    void onRuledEvent(const ruled::v1::RuledEvent &ev) override
    {
        OpeningDriver::onRuledEvent(ev);
        if (ev.has_stack_pushed()) {
            const auto &sp = ev.stack_pushed();
            const QString cardId = QString::fromStdString(sp.card_id());
            if (harmonizeFlowActive && cardId == QLatin1String("unending_whisper")) {
                sawHarmonizeStackReceipt =
                    std::any_of(sp.chosen_cast_cost_labels().begin(), sp.chosen_cast_cost_labels().end(),
                                [](const std::string &label) {
                                    const QString receipt = QString::fromStdString(label);
                                    return receipt.contains(QStringLiteral("Harmonize")) &&
                                           receipt.contains(QStringLiteral("reduce {2}"));
                                }) &&
                    QString::fromStdString(sp.ability_annotation()).contains(QStringLiteral("Harmonize"));
            }
        }
    }
};

TEST_F(RuledE2ESmokeTest, HarmonizeUsesOwnerOnlyReductionAndPreservesPhysicalIdentity)
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

    HarmonizeDriver p1(true, QStringLiteral("harmonizep1"), &transcript);
    HarmonizeDriver p2(false, QStringLiteral("harmonizep2"), &transcript);

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Harmonize cohort game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Harmonize cohort game start (p2)"));
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

    auto sendAndPump = [&](HarmonizeDriver &sender, const ruled::v1::RuledCommand &command,
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
    auto devPut = [&](const char *cardName, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(zone);
        put->set_ready(ready);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for Harmonize cohort").arg(cardName));
    };
    auto devMove = [&](const char *cardName, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *move = dev->mutable_move_card();
        move->set_card_name(cardName);
        move->set_zone(zone);
        return sendAndPump(p1, command, QStringLiteral("dev: move %1 for Harmonize cohort").arg(cardName));
    };
    auto passPriority = [&](HarmonizeDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in Harmonize cohort"));
    };
    auto permanentTapped = [](const HarmonizeDriver &client, int playerId, quint32 oid) {
        const auto battlefield = client.battlefieldByPlayer.find(playerId);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return false;
        }
        const auto permanent =
            std::find_if(battlefield->second.begin(), battlefield->second.end(),
                         [oid](const HarmonizeDriver::Permanent &candidate) { return candidate.oid == oid; });
        return permanent != battlefield->second.end() && permanent->tapped;
    };

    ASSERT_TRUE(devPut("Unending Whisper", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devMove("Unending Whisper", ruled::v1::DEV_ZONE_GRAVEYARD));
    ASSERT_TRUE(devPut("Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ruled::v1::RuledCommand addHarmonizeMana;
    addHarmonizeMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addHarmonizeMana.mutable_dev_command()->mutable_add_mana()->set_u(1);
    addHarmonizeMana.mutable_dev_command()->mutable_add_mana()->set_c(3);
    ASSERT_TRUE(sendAndPump(p1, addHarmonizeMana, QStringLiteral("dev: add {3}{U} for Harmonize")));

    const ruled::v1::LegalZoneCastAction *harmonize = nullptr;
    for (const auto &action : p1.latestLegal.zone_cast_actions()) {
        if (action.card_name() == "Unending Whisper" && action.cast_method() == ruled::v1::CAST_METHOD_HARMONIZE) {
            harmonize = &action;
            break;
        }
    }
    ASSERT_NE(harmonize, nullptr);
    EXPECT_TRUE(std::none_of(p2.latestLegal.zone_cast_actions().begin(), p2.latestLegal.zone_cast_actions().end(),
                             [](const ruled::v1::LegalZoneCastAction &action) {
                                 return action.card_name() == "Unending Whisper" &&
                                        action.cast_method() == ruled::v1::CAST_METHOD_HARMONIZE;
                             }));
    ASSERT_EQ(harmonize->cost_choices().cast_cost_groups_size(), 1);
    const auto &group = harmonize->cost_choices().cast_cost_groups(0);
    ASSERT_EQ(group.skip_label(), "Pay full Harmonize cost");
    ASSERT_EQ(group.options_size(), 1);
    const auto &tapOption = group.options(0);
    ASSERT_EQ(tapOption.kind(), ruled::v1::CAST_COST_OPTION_KIND_TAP_PERMANENT_FOR_GENERIC_REDUCTION);
    ASSERT_EQ(tapOption.valid_permanent_ids_size(), 1);
    ASSERT_EQ(tapOption.valid_permanent_generations_size(), 1);
    ASSERT_EQ(tapOption.valid_permanent_generic_reductions_size(), 1);
    EXPECT_EQ(tapOption.valid_permanent_generic_reductions(0), 2u);

    const quint32 whisperOid = harmonize->object_id();
    const quint32 creatureOid = tapOption.valid_permanent_ids(0);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(whisperOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(whisperOid));
    ASSERT_EQ(p1.serverCardByEngineOid[whisperOid], p2.serverCardByEngineOid[whisperOid]);
    p1.harmonizePhysicalCardId = p1.serverCardByEngineOid[whisperOid];
    p2.harmonizePhysicalCardId = p2.serverCardByEngineOid[whisperOid];
    p1.harmonizeCreatureOid = creatureOid;
    p2.harmonizeCreatureOid = creatureOid;
    p1.harmonizeFlowActive = true;
    p2.harmonizeFlowActive = true;

    ruled::v1::RuledCommand castHarmonize;
    auto *cast = castHarmonize.mutable_cast_spell();
    cast->set_cast_method(ruled::v1::CAST_METHOD_HARMONIZE);
    cast->mutable_source()->set_graveyard_object_id(whisperOid);
    cast->mutable_source()->set_expected_zone_change_generation(harmonize->zone_change_generation());
    cast->set_face_index(harmonize->face_index());
    auto *selection = cast->add_cast_cost_group_selections();
    selection->set_group_index(group.group_index());
    selection->set_option_index(tapOption.option_index());
    selection->set_permanent_id(creatureOid);
    selection->set_expected_zone_change_generation(tapOption.valid_permanent_generations(0));
    ASSERT_TRUE(sendAndPump(p1, castHarmonize, QStringLiteral("cast Unending Whisper with Harmonize")));
    EXPECT_TRUE(p1.sawHarmonizeStackReceipt && p2.sawHarmonizeStackReceipt);
    EXPECT_TRUE(p1.sawHarmonizeGraveToStack && p2.sawHarmonizeGraveToStack);
    EXPECT_TRUE(permanentTapped(p1, p1.myId, creatureOid));
    EXPECT_TRUE(permanentTapped(p2, p1.myId, creatureOid));

    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_TRUE(p1.sawHarmonizeStackToExile && p2.sawHarmonizeStackToExile);
    EXPECT_TRUE(p1.harmonizePhysicalIdentityContinuous && p2.harmonizePhysicalIdentityContinuous);
    EXPECT_EQ(p1.harmonizePhysicalCardId, p2.harmonizePhysicalCardId);

    p1.harmonizeFlowActive = false;
    p2.harmonizeFlowActive = false;
    p1.normalWhisperFlowActive = true;
    p2.normalWhisperFlowActive = true;
    ASSERT_TRUE(devPut("Unending Whisper", ruled::v1::DEV_ZONE_HAND, false));
    ruled::v1::RuledCommand addNormalMana;
    addNormalMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addNormalMana.mutable_dev_command()->mutable_add_mana()->set_u(1);
    ASSERT_TRUE(sendAndPump(p1, addNormalMana, QStringLiteral("dev: add {U} for normal cast")));
    const auto *normal = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Unending Whisper"));
    ASSERT_NE(normal, nullptr);
    ruled::v1::RuledCommand castNormal;
    castNormal.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    castNormal.mutable_cast_spell()->mutable_source()->set_hand_index(normal->hand_index());
    ASSERT_TRUE(sendAndPump(p1, castNormal, QStringLiteral("cast Unending Whisper normally")));
    EXPECT_TRUE(p1.sawNormalWhisperHandToStack && p2.sawNormalWhisperHandToStack);
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_TRUE(p1.sawNormalWhisperStackToGrave && p2.sawNormalWhisperStackToGrave);
}

TEST_F(RuledE2ESmokeTest, KaitoNinjutsuHandSourcePaymentPreviewIsPrivateAndValid)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("ninjutsup1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("ninjutsup2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "issue 203 Ninjutsu game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "issue 203 Ninjutsu game start (p2)"));
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
    for (OpeningDriver *client : {&p1, &p2}) {
        ruled::v1::RuledCommand stops;
        auto *policy = stops.mutable_set_auto_pass_policy();
        policy->add_stop_on_own_turn(ruled::v1::PHASE_ID_MAIN1);
        policy->add_stop_on_opponent_turn(ruled::v1::PHASE_ID_MAIN1);
        policy->add_stop_on_own_turn(ruled::v1::PHASE_ID_DECLARE_BLOCKERS);
        policy->add_stop_on_opponent_turn(ruled::v1::PHASE_ID_DECLARE_BLOCKERS);
        client->sendRuled(stops, QStringLiteral("issue 203 publish Ninjutsu combat stop"));
    }

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
    auto pass = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("issue 203 Ninjutsu pass"));
    };
    auto put = [&](const char *name, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *placement = dev->mutable_put_card_in_zone();
        placement->set_card_name(name);
        placement->set_zone(zone);
        placement->set_ready(ready);
        return send(p1, command, QStringLiteral("issue 203 put %1").arg(name));
    };

    ASSERT_TRUE(put("Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(put("Kaito, Bane of Nightmares", ruled::v1::DEV_ZONE_HAND, false));
    const auto &battlefield = p1.battlefieldByPlayer[p1.myId];
    const auto bearIt =
        std::find_if(battlefield.begin(), battlefield.end(), [](const OpeningDriver::Permanent &permanent) {
            return permanent.cardId == QStringLiteral("grizzly_bears");
        });
    ASSERT_NE(bearIt, battlefield.end());
    const OpeningDriver::Permanent bear = *bearIt;

    QElapsedTimer toAttack;
    toAttack.start();
    while (p1.phase != ruled::v1::PHASE_ID_DECLARE_ATTACKERS && toAttack.elapsed() < 20000) {
        ASSERT_TRUE(pass(p1.priorityPlayer == p1.myId ? p1 : p2));
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_DECLARE_ATTACKERS);
    const auto assignment =
        std::find_if(p1.latestLegal.legal_attack_assignments().begin(), p1.latestLegal.legal_attack_assignments().end(),
                     [&](const ruled::v1::AttackAssignment &candidate) {
                         return candidate.attacker_object_id() == bear.oid && candidate.has_defender() &&
                                candidate.defender().kind() == ruled::v1::TARGET_REF_KIND_PLAYER &&
                                candidate.defender().object_id() == static_cast<quint32>(p2.myId);
                     });
    ASSERT_NE(assignment, p1.latestLegal.legal_attack_assignments().end());
    ruled::v1::RuledCommand declare;
    *declare.mutable_declare_attackers()->add_assignments() = *assignment;
    ASSERT_TRUE(send(p1, declare, QStringLiteral("issue 203 declare unblocked attacker")));

    auto ninjutsuAvailable = [&] {
        const auto *action =
            p1.zoneAbilityAction(QStringLiteral("Kaito, Bane of Nightmares"), ruled::v1::ABILITY_SOURCE_ZONE_HAND);
        return p1.priorityPlayer == p1.myId && action && action->has_ability() && action->ability().activatable();
    };
    QElapsedTimer toNinjutsu;
    toNinjutsu.start();
    while (!ninjutsuAvailable() && toNinjutsu.elapsed() < 10000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_DECLARE_BLOCKERS) {
            ruled::v1::RuledCommand noBlocks;
            noBlocks.mutable_declare_blockers();
            ASSERT_TRUE(send(p2, noBlocks, QStringLiteral("issue 203 declare no blockers")));
        } else if (p1.priorityPlayer == p2.myId) {
            ASSERT_TRUE(pass(p2));
        }
    }
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    const auto *availableAction =
        p1.zoneAbilityAction(QStringLiteral("Kaito, Bane of Nightmares"), ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    ASSERT_NE(availableAction, nullptr);
    ASSERT_TRUE(availableAction->has_ability());
    ASSERT_TRUE(availableAction->ability().activatable());
    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_u(1);
    addMana.mutable_dev_command()->mutable_add_mana()->set_b(1);
    addMana.mutable_dev_command()->mutable_add_mana()->set_c(1);
    ASSERT_TRUE(send(p1, addMana, QStringLiteral("issue 203 add Ninjutsu mana")));
    const auto *published =
        p1.zoneAbilityAction(QStringLiteral("Kaito, Bane of Nightmares"), ruled::v1::ABILITY_SOURCE_ZONE_HAND);
    ASSERT_NE(published, nullptr);
    const auto action = *published;
    const quint64 abilityKey = (static_cast<quint64>(action.object_id()) << 32) | action.ability_index();
    const auto choices = p1.latestLegal.cost_choices_by_ability().find(abilityKey);
    ASSERT_NE(choices, p1.latestLegal.cost_choices_by_ability().end());
    const auto returnChoice =
        std::find_if(choices->second.choices().begin(), choices->second.choices().end(),
                     [](const ruled::v1::LegalCostChoice &choice) {
                         return choice.kind() == ruled::v1::COST_CHOICE_KIND_RETURN_UNBLOCKED_ATTACKER;
                     });
    ASSERT_NE(returnChoice, choices->second.choices().end());

    ruled::v1::RuledCommand query;
    auto *preview = query.mutable_preview_payment();
    preview->set_transaction_id(203);
    preview->set_revision(1);
    auto *activation = preview->mutable_activate_ability();
    activation->set_source_object_id(action.object_id());
    activation->set_source_zone(action.source_zone());
    activation->set_expected_zone_change_generation(action.zone_change_generation());
    activation->set_ability_index(action.ability_index());
    auto *cost = activation->add_cost_selections();
    cost->set_cost_index(returnChoice->cost_index());
    auto *returned = cost->mutable_battlefield_objects()->add_objects();
    returned->set_object_id(bear.oid);
    returned->set_zone_change_generation(bear.generation);
    const int previewCount = p1.paymentPreviewCount;
    const int observerPreviewCount = p2.paymentPreviewCount;
    const quint64 stateVersion = p1.stateVersion;
    p1.sendRuled(query, QStringLiteral("issue 203 private Ninjutsu payment preview"));
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.paymentPreviewCount == previewCount + 1; }, 10000,
                             "Ninjutsu hand-source preview"));
    p2.pump(100);
    ASSERT_TRUE(p1.paymentPreview.valid()) << p1.paymentPreview.error();
    EXPECT_FALSE(p1.paymentPreview.complete());
    EXPECT_EQ(p1.paymentPreview.selection().source().object_id(), action.object_id());
    EXPECT_EQ(p1.paymentPreview.selection().source().zone_change_generation(), action.zone_change_generation());
    EXPECT_EQ(p1.stateVersion, stateVersion) << "payment previews must remain read-only";
    EXPECT_EQ(p2.paymentPreviewCount, observerPreviewCount) << "payment previews are private";

    preview->set_revision(2);
    *activation->mutable_payment() = p1.paymentPreview.selection();
    activation->mutable_payment()->mutable_mana()->set_u(1);
    activation->mutable_payment()->mutable_mana()->set_b(1);
    activation->mutable_payment()->mutable_mana()->set_c(1);
    p1.sendRuled(query, QStringLiteral("issue 203 exact Ninjutsu mana preview"));
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.paymentPreviewCount == previewCount + 2; }, 10000, "Ninjutsu exact mana preview"));
    p2.pump(100);
    ASSERT_TRUE(p1.paymentPreview.valid()) << p1.paymentPreview.error();
    EXPECT_TRUE(p1.paymentPreview.complete());
    EXPECT_EQ(p1.stateVersion, stateVersion);
    EXPECT_EQ(p2.paymentPreviewCount, observerPreviewCount);

    ruled::v1::RuledCommand activate;
    *activate.mutable_activate_ability() = *activation;
    ASSERT_TRUE(send(p1, activate, QStringLiteral("issue 203 activate Ninjutsu with exact payment")));
    for (const OpeningDriver *client : {&p1, &p2}) {
        ASSERT_EQ(client->activePublicReveals.size(), 1u);
        const auto &reveal = client->activePublicReveals.front();
        ASSERT_EQ(reveal.cards_size(), 1);
        EXPECT_EQ(reveal.cards(0).card_name(), "Kaito, Bane of Nightmares");
        EXPECT_EQ(reveal.source_description(), "Kaito, Bane of Nightmares");
        EXPECT_EQ(reveal.zone_owner_player_id(), p1.myId);
    }
}

TEST_F(RuledE2ESmokeTest, ConvokeAndWaterbendPreviewsArePrivateReadOnlyAndCommitExactPhysicalTaps)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("convokep1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("convokep2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "issue 145 game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "issue 145 game start (p2)"));
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
    auto putPermanent = [&](const char *name, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(name);
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(ready);
        return send(p1, command, QStringLiteral("issue 145 put %1").arg(name));
    };
    auto findPermanent = [](const OpeningDriver &client, int controller,
                            const QString &cardId) -> std::optional<OpeningDriver::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto permanent =
            std::find_if(battlefield->second.begin(), battlefield->second.end(),
                         [&cardId](const OpeningDriver::Permanent &candidate) { return candidate.cardId == cardId; });
        return permanent == battlefield->second.end() ? std::nullopt : std::optional(*permanent);
    };

    ASSERT_TRUE(putPermanent("Grizzly Bears", false));
    const auto bear = findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"));
    ASSERT_TRUE(bear.has_value());
    ruled::v1::RuledCommand putSpell;
    auto *put = putSpell.mutable_dev_command();
    put->set_target_player_id(p1.myId);
    put->mutable_put_card_in_zone()->set_card_name("Unexpected Assistance");
    put->mutable_put_card_in_zone()->set_zone(ruled::v1::DEV_ZONE_HAND);
    ASSERT_TRUE(send(p1, putSpell, QStringLiteral("put Convoke spell")));
    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_u(2);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(2);
    ASSERT_TRUE(send(p1, mana, QStringLiteral("mixed Convoke mana")));
    const auto *hand = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Unexpected Assistance"));
    ASSERT_NE(hand, nullptr);
    EXPECT_TRUE(hand->has_convoke());
    ruled::v1::RuledCommand query;
    auto *preview = query.mutable_preview_payment();
    preview->set_transaction_id(145);
    preview->set_revision(1);
    auto *cast = preview->mutable_cast_spell();
    cast->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast->mutable_source()->set_hand_index(hand->hand_index());
    auto *selection = cast->mutable_payment();
    selection->mutable_mana()->set_u(2);
    selection->mutable_mana()->set_c(2);
    auto *creature = selection->add_convoke();
    creature->mutable_object()->set_object_id(bear->oid);
    creature->mutable_object()->set_zone_change_generation(bear->generation);
    creature->set_kind(ruled::v1::OBJECT_PAYMENT_KIND_GENERIC);
    const auto before1 = p1.stateVersion;
    const auto before2 = p2.stateVersion;
    const auto legal = p1.latestLegal.SerializeAsString();
    p1.sendRuled(query, QStringLiteral("private Convoke preview"));
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.paymentPreviewCount == 1; }, 10000, "Convoke preview"));
    p2.pump(200);
    ASSERT_TRUE(p1.paymentPreview.valid()) << p1.paymentPreview.error();
    ASSERT_TRUE(p1.paymentPreview.complete());
    EXPECT_EQ(p1.stateVersion, before1);
    EXPECT_EQ(p2.stateVersion, before2);
    EXPECT_EQ(p2.paymentPreviewCount, 0);
    EXPECT_EQ(p1.latestLegal.SerializeAsString(), legal);
    EXPECT_FALSE(findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"))->tapped);
    EXPECT_FALSE(findPermanent(p2, p1.myId, QStringLiteral("grizzly_bears"))->tapped);
    const auto authoritativeRevision = p1.paymentPreview.selection().expected_state_revision();
    preview->set_revision(2);
    p1.sendRuled(query, QStringLiteral("repeat read-only preview"));
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.paymentPreviewCount == 2; }, 10000, "repeat preview"));
    EXPECT_EQ(p1.paymentPreview.selection().expected_state_revision(), authoritativeRevision);
    ruled::v1::RuledCommand commit;
    *commit.mutable_cast_spell() = *cast;
    *commit.mutable_cast_spell()->mutable_payment() = p1.paymentPreview.selection();
    ASSERT_TRUE(send(p1, commit, QStringLiteral("commit mixed Convoke")));
    EXPECT_TRUE(findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"))->tapped);
    EXPECT_TRUE(findPermanent(p2, p1.myId, QStringLiteral("grizzly_bears"))->tapped);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(bear->oid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(bear->oid));
    EXPECT_EQ(p1.serverCardByEngineOid[bear->oid], p2.serverCardByEngineOid[bear->oid]);
    EXPECT_TRUE(p1.physicallyTappedCardIds.count(p1.serverCardByEngineOid[bear->oid]));
    EXPECT_TRUE(p2.physicallyTappedCardIds.count(p2.serverCardByEngineOid[bear->oid]));
    EXPECT_EQ(p1.myPool.total(), 0);

    // Waterbend uses the same private preview and physical-object transaction during activation.
    // The Convoke spell remains on the stack: Vinebender is allowed throughout its controller's turn.
    ASSERT_TRUE(putPermanent("Foggy Swamp Vinebender", false));
    ASSERT_TRUE(putPermanent("Goldvein Pick", false));
    const auto vine = findPermanent(p1, p1.myId, QStringLiteral("foggy_swamp_vinebender"));
    const auto pick = findPermanent(p1, p1.myId, QStringLiteral("goldvein_pick"));
    ASSERT_TRUE(vine && pick);
    ASSERT_TRUE(vine->sick);
    const auto eligibility = p1.latestLegal.mana_payment_by_ability().find(static_cast<quint64>(vine->oid) << 32);
    ASSERT_NE(eligibility, p1.latestLegal.mana_payment_by_ability().end());
    EXPECT_TRUE(eligibility->second.has_waterbend());
    mana.mutable_dev_command()->mutable_add_mana()->Clear();
    mana.mutable_dev_command()->mutable_add_mana()->set_c(3);
    ASSERT_TRUE(send(p1, mana, QStringLiteral("Waterbend activation mana")));
    ruled::v1::RuledCommand waterbendQuery;
    auto *waterbendPreview = waterbendQuery.mutable_preview_payment();
    waterbendPreview->set_transaction_id(146);
    waterbendPreview->set_revision(1);
    auto *activation = waterbendPreview->mutable_activate_ability();
    p1.setBattlefieldAbilitySource(activation, vine->oid);
    auto *payment = activation->mutable_payment();
    payment->mutable_mana()->set_c(3);
    for (const auto &object : {*vine, *pick}) {
        auto *ref = payment->add_waterbend();
        ref->set_object_id(object.oid);
        ref->set_zone_change_generation(object.generation);
    }
    const auto activationVersion = p1.stateVersion;
    p1.sendRuled(waterbendQuery, QStringLiteral("private Waterbend activation preview"));
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.paymentPreviewCount == 3; }, 10000, "Waterbend preview"));
    p2.pump(100);
    ASSERT_TRUE(p1.paymentPreview.valid()) << p1.paymentPreview.error();
    ASSERT_TRUE(p1.paymentPreview.complete());
    EXPECT_EQ(p1.stateVersion, activationVersion);
    EXPECT_EQ(p2.paymentPreviewCount, 0);
    EXPECT_FALSE(findPermanent(p2, p1.myId, QStringLiteral("goldvein_pick"))->tapped);
    ruled::v1::RuledCommand activate;
    *activate.mutable_activate_ability() = *activation;
    *activate.mutable_activate_ability()->mutable_payment() = p1.paymentPreview.selection();
    ASSERT_TRUE(send(p1, activate, QStringLiteral("commit mixed Waterbend activation")));
    for (const auto &object : {*vine, *pick}) {
        EXPECT_EQ(p1.serverCardByEngineOid[object.oid], p2.serverCardByEngineOid[object.oid]);
        EXPECT_TRUE(p1.physicallyTappedCardIds.count(p1.serverCardByEngineOid[object.oid]));
        EXPECT_TRUE(p2.physicallyTappedCardIds.count(p2.serverCardByEngineOid[object.oid]));
    }
    EXPECT_EQ(p1.myPool.total(), 0);
    auto pass = [&]() {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(p1.priorityPlayer == p1.myId ? p1 : p2, command, QStringLiteral("resolve payment scenario"));
    };
    ASSERT_TRUE(pass());
    ASSERT_TRUE(pass());
    EXPECT_EQ(findPermanent(p1, p1.myId, QStringLiteral("foggy_swamp_vinebender"))->power, 5);
    EXPECT_EQ(findPermanent(p2, p1.myId, QStringLiteral("foggy_swamp_vinebender"))->power, 5);
    ASSERT_TRUE(pass());
    ASSERT_TRUE(pass());
    ASSERT_TRUE(p1.pendingChoice);
    ruled::v1::RuledCommand discard;
    discard.mutable_submit_resolution_choice()->add_chosen_object_ids(p1.pendingChoice->candidate_object_ids(0));
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, discard, QStringLiteral("finish Unexpected Assistance")));

    ASSERT_TRUE(putPermanent("Ornithopter", false));
    ASSERT_TRUE(putPermanent("Island", true));
    const auto thopter = findPermanent(p1, p1.myId, QStringLiteral("ornithopter"));
    const auto island = findPermanent(p1, p1.myId, QStringLiteral("island"));
    ASSERT_TRUE(thopter && island);
    putSpell.mutable_dev_command()->mutable_put_card_in_zone()->set_card_name("Waterbending Lesson");
    ASSERT_TRUE(send(p1, putSpell, QStringLiteral("put Waterbending Lesson")));
    mana.mutable_dev_command()->mutable_add_mana()->Clear();
    mana.mutable_dev_command()->mutable_add_mana()->set_u(4);
    ASSERT_TRUE(send(p1, mana, QStringLiteral("Lesson casting mana")));
    const auto *lesson = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Waterbending Lesson"));
    ASSERT_NE(lesson, nullptr);
    ruled::v1::RuledCommand castLesson;
    castLesson.mutable_cast_spell()->mutable_source()->set_hand_index(lesson->hand_index());
    ASSERT_TRUE(send(p1, castLesson, QStringLiteral("cast Waterbending Lesson")));
    const int handBeforeDraw = p1.handSizeByPlayer[p1.myId];
    ASSERT_TRUE(pass());
    ASSERT_TRUE(pass());
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], handBeforeDraw + 3);
    ASSERT_TRUE(p1.pendingChoice);
    EXPECT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    ruled::v1::RuledCommand branch;
    branch.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_SELECT_BRANCH);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, branch, QStringLiteral("choose Lesson Waterbend")));
    ASSERT_TRUE(p1.pendingChoice);
    EXPECT_TRUE(p1.pendingChoice->waterbend());
    EXPECT_FALSE(p2.pendingChoice);
    ASSERT_TRUE(p2.lastResolutionChoice);
    EXPECT_FALSE(p2.lastResolutionChoice->waterbend());
    EXPECT_EQ(p2.lastResolutionChoice->generic_mana_cost(), 2u); // The printed cost is public; staging is private.
    ruled::v1::RuledCommand makeMana;
    p1.setBattlefieldAbilitySource(makeMana.mutable_activate_ability(), island->oid);
    ASSERT_TRUE(send(p1, makeMana, QStringLiteral("mana ability during Lesson payment")));
    ASSERT_TRUE(p1.pendingChoice);
    EXPECT_TRUE(p1.pendingChoice->waterbend());
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], handBeforeDraw + 3);
    waterbendQuery.Clear();
    waterbendPreview = waterbendQuery.mutable_preview_payment();
    waterbendPreview->set_transaction_id(147);
    waterbendPreview->set_revision(1);
    auto *resolution = waterbendPreview->mutable_resolution_choice();
    resolution->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
    resolution->mutable_payment()->mutable_mana()->set_u(1);
    auto *object = resolution->mutable_payment()->add_waterbend();
    object->set_object_id(thopter->oid);
    object->set_zone_change_generation(thopter->generation);
    const auto resolutionVersion = p1.stateVersion;
    p1.sendRuled(waterbendQuery, QStringLiteral("private Waterbend resolution preview"));
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.paymentPreviewCount == 4; }, 10000, "Lesson preview"));
    p2.pump(100);
    ASSERT_TRUE(p1.paymentPreview.valid()) << p1.paymentPreview.error();
    ASSERT_TRUE(p1.paymentPreview.complete());
    EXPECT_EQ(p1.stateVersion, resolutionVersion);
    EXPECT_EQ(p2.paymentPreviewCount, 0);
    ruled::v1::RuledCommand pay;
    *pay.mutable_submit_resolution_choice() = *resolution;
    *pay.mutable_submit_resolution_choice()->mutable_payment() = p1.paymentPreview.selection();
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, pay, QStringLiteral("commit Lesson mixed Waterbend")));
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], handBeforeDraw + 3);
    EXPECT_EQ(p1.stackDepth, 0);
    EXPECT_EQ(p1.myPool.total(), 0);
    EXPECT_TRUE(p1.physicallyTappedCardIds.count(p1.serverCardByEngineOid[thopter->oid]));
    EXPECT_TRUE(p2.physicallyTappedCardIds.count(p2.serverCardByEngineOid[thopter->oid]));
}

TEST_F(RuledE2ESmokeTest, SelectableTapCounterAndBlightPaymentsPreservePrivacyAndExactCardsForBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("tappaymentp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("tappaymentp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "issue 144 game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "issue 144 game start (p2)"));
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
    auto putPermanent = [&](const char *name, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(name);
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(ready);
        return send(p1, command, QStringLiteral("issue 144 put %1").arg(name));
    };
    auto findPermanent = [](const OpeningDriver &client, int controller,
                            const QString &cardId) -> std::optional<OpeningDriver::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto permanent =
            std::find_if(battlefield->second.begin(), battlefield->second.end(),
                         [&cardId](const OpeningDriver::Permanent &candidate) { return candidate.cardId == cardId; });
        return permanent == battlefield->second.end() ? std::nullopt : std::optional(*permanent);
    };

    ASSERT_TRUE(putPermanent("Gene Pollinator", true));
    ASSERT_TRUE(putPermanent("Grizzly Bears", false));
    const auto gene = findPermanent(p1, p1.myId, QStringLiteral("gene_pollinator"));
    const auto bear = findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"));
    ASSERT_TRUE(gene.has_value());
    ASSERT_TRUE(bear.has_value());
    ASSERT_FALSE(bear->tapped);
    ASSERT_TRUE(bear->sick) << "the separate tap payment must accept a newly controlled permanent";

    const quint64 abilityKey = static_cast<quint64>(gene->oid) << 32;
    const auto costs = p1.latestLegal.cost_choices_by_ability().find(abilityKey);
    ASSERT_NE(costs, p1.latestLegal.cost_choices_by_ability().end());
    const ruled::v1::LegalCostChoice *tapCost = nullptr;
    for (const auto &choice : costs->second.choices()) {
        if (choice.kind() == ruled::v1::COST_CHOICE_KIND_TAP && choice.min() == 1 && choice.max() == 1) {
            tapCost = &choice;
            break;
        }
    }
    ASSERT_NE(tapCost, nullptr);
    ASSERT_EQ(tapCost->candidate_objects_size(), 1);
    ASSERT_TRUE(tapCost->candidate_objects(0).has_object());
    EXPECT_EQ(tapCost->candidate_objects(0).object().object_id(), bear->oid);
    EXPECT_EQ(tapCost->candidate_objects(0).object().zone_change_generation(), bear->generation);

    ruled::v1::RuledCommand activate;
    auto *ability = activate.mutable_activate_ability();
    p1.setBattlefieldAbilitySource(ability, gene->oid);
    ability->set_ability_index(0);
    ability->set_mana_option_index(0);
    auto *selection = ability->add_cost_selections();
    selection->set_cost_index(tapCost->cost_index());
    auto *selected = selection->mutable_battlefield_objects()->add_objects();
    selected->set_object_id(bear->oid);
    selected->set_zone_change_generation(bear->generation);
    ASSERT_TRUE(send(p1, activate, QStringLiteral("issue 144 activate Gene Pollinator")));

    const auto p1Gene = findPermanent(p1, p1.myId, QStringLiteral("gene_pollinator"));
    const auto p1Bear = findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"));
    const auto p2Gene = findPermanent(p2, p1.myId, QStringLiteral("gene_pollinator"));
    const auto p2Bear = findPermanent(p2, p1.myId, QStringLiteral("grizzly_bears"));
    ASSERT_TRUE(p1Gene.has_value() && p1Bear.has_value() && p2Gene.has_value() && p2Bear.has_value());
    EXPECT_TRUE(p1Gene->tapped && p1Bear->tapped);
    EXPECT_TRUE(p2Gene->tapped && p2Bear->tapped);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(gene->oid) && p1.serverCardByEngineOid.count(bear->oid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(gene->oid) && p2.serverCardByEngineOid.count(bear->oid));
    EXPECT_EQ(p1.serverCardByEngineOid[gene->oid], p2.serverCardByEngineOid[gene->oid]);
    EXPECT_EQ(p1.serverCardByEngineOid[bear->oid], p2.serverCardByEngineOid[bear->oid]);
    EXPECT_TRUE(p1.physicallyTappedCardIds.count(p1.serverCardByEngineOid[gene->oid]));
    EXPECT_TRUE(p1.physicallyTappedCardIds.count(p1.serverCardByEngineOid[bear->oid]));
    EXPECT_TRUE(p2.physicallyTappedCardIds.count(p2.serverCardByEngineOid[gene->oid]));
    EXPECT_TRUE(p2.physicallyTappedCardIds.count(p2.serverCardByEngineOid[bear->oid]));
    EXPECT_EQ(p1.myPool.total(), 1);

    // Station reuses the same private, generation-bound tap picker, while its counters and
    // threshold-derived public characteristics must synchronize to both physical clients.
    ASSERT_TRUE(putPermanent("Wurmwall Sweeper", true));
    if (p1.pendingTriggerOrder) {
        ASSERT_GT(p1.pendingTriggerOrder->candidates_size(), 0);
        ruled::v1::RuledCommand order;
        order.mutable_submit_trigger_order()->set_trigger_object_id(
            p1.pendingTriggerOrder->candidates(0).trigger_object_id());
        p1.pendingTriggerOrder.reset();
        ASSERT_TRUE(send(p1, order, QStringLiteral("issue 147 order Wurmwall surveil trigger")));
    }
    ASSERT_EQ(p1.stackDepth, 1);
    ruled::v1::RuledCommand stationPass;
    stationPass.mutable_pass_priority();
    ASSERT_TRUE(send(p1, stationPass, QStringLiteral("issue 147 pass Wurmwall surveil")));
    ASSERT_TRUE(send(p2, stationPass, QStringLiteral("issue 147 resolve Wurmwall surveil")));
    ASSERT_TRUE(p1.pendingChoice.has_value());
    ASSERT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_LIBRARY_LOOK);
    ASSERT_GE(p1.pendingChoice->candidate_object_ids_size(), 1);
    ruled::v1::RuledCommand surveil;
    surveil.mutable_submit_resolution_choice()->add_chosen_object_ids(p1.pendingChoice->candidate_object_ids(0));
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, surveil, QStringLiteral("issue 147 finish Wurmwall surveil")));
    ASSERT_EQ(p1.stackDepth, 0);

    ASSERT_TRUE(putPermanent("Hill Giant", false));
    const auto station = findPermanent(p1, p1.myId, QStringLiteral("wurmwall_sweeper"));
    const auto crew = findPermanent(p1, p1.myId, QStringLiteral("hill_giant"));
    ASSERT_TRUE(station && crew);
    EXPECT_FALSE(station->creature);
    EXPECT_TRUE(crew->sick);
    const int stationPhysicalId = p1.serverCardByEngineOid.at(station->oid);
    EXPECT_EQ(p2.serverCardByEngineOid.at(station->oid), stationPhysicalId);

    const quint64 stationAbilityKey = static_cast<quint64>(station->oid) << 32;
    const auto stationCosts = p1.latestLegal.cost_choices_by_ability().find(stationAbilityKey);
    ASSERT_NE(stationCosts, p1.latestLegal.cost_choices_by_ability().end());
    ASSERT_EQ(stationCosts->second.choices_size(), 1);
    const auto &stationTap = stationCosts->second.choices(0);
    ASSERT_EQ(stationTap.kind(), ruled::v1::COST_CHOICE_KIND_TAP);
    const auto stationCrew =
        std::find_if(stationTap.candidate_objects().begin(), stationTap.candidate_objects().end(),
                     [&](const auto &candidate) { return candidate.object().object_id() == crew->oid; });
    ASSERT_NE(stationCrew, stationTap.candidate_objects().end());
    EXPECT_EQ(stationCrew->object().zone_change_generation(), crew->generation);
    EXPECT_EQ(p2.latestLegal.cost_choices_by_ability().count(stationAbilityKey), 0u);

    ruled::v1::RuledCommand stationActivation;
    auto *stationAbility = stationActivation.mutable_activate_ability();
    p1.setBattlefieldAbilitySource(stationAbility, station->oid);
    stationAbility->set_ability_index(0);
    auto *stationSelection = stationAbility->add_cost_selections();
    stationSelection->set_cost_index(stationTap.cost_index());
    auto *stationCrewRef = stationSelection->mutable_battlefield_objects()->add_objects();
    stationCrewRef->set_object_id(crew->oid);
    stationCrewRef->set_zone_change_generation(crew->generation);
    ASSERT_TRUE(send(p1, stationActivation, QStringLiteral("issue 147 activate Station")));
    EXPECT_TRUE(findPermanent(p1, p1.myId, QStringLiteral("hill_giant"))->tapped);
    EXPECT_TRUE(findPermanent(p2, p1.myId, QStringLiteral("hill_giant"))->tapped);
    EXPECT_FALSE(findPermanent(p1, p1.myId, QStringLiteral("wurmwall_sweeper"))->tapped);
    ASSERT_TRUE(send(p1, stationPass, QStringLiteral("issue 147 pass Station")));
    ASSERT_TRUE(send(p2, stationPass, QStringLiteral("issue 147 resolve Station")));
    const auto stationAfterCrew = findPermanent(p1, p1.myId, QStringLiteral("wurmwall_sweeper"));
    ASSERT_TRUE(stationAfterCrew);
    EXPECT_FALSE(stationAfterCrew->creature);
    ASSERT_TRUE(p1.annotationByServerCardId.count(stationPhysicalId));
    EXPECT_TRUE(p1.annotationByServerCardId.at(stationPhysicalId).contains(QStringLiteral("3 charge counter(s)")));

    ruled::v1::RuledCommand putDrill;
    putDrill.mutable_dev_command()->set_target_player_id(p1.myId);
    putDrill.mutable_dev_command()->mutable_put_card_in_zone()->set_card_name("Drill Too Deep");
    putDrill.mutable_dev_command()->mutable_put_card_in_zone()->set_zone(ruled::v1::DEV_ZONE_HAND);
    ASSERT_TRUE(send(p1, putDrill, QStringLiteral("issue 147 put Drill Too Deep")));
    ruled::v1::RuledCommand drillMana;
    drillMana.mutable_dev_command()->set_target_player_id(p1.myId);
    drillMana.mutable_dev_command()->mutable_add_mana()->set_r(1);
    drillMana.mutable_dev_command()->mutable_add_mana()->set_c(1);
    ASSERT_TRUE(send(p1, drillMana, QStringLiteral("issue 147 add Drill mana")));
    const auto *drill = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Drill Too Deep"));
    ASSERT_NE(drill, nullptr);
    ruled::v1::RuledCommand castDrill;
    auto *drillCast = castDrill.mutable_cast_spell();
    drillCast->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    drillCast->mutable_source()->set_hand_index(drill->hand_index());
    auto *drillMode = drillCast->add_selected_modes();
    drillMode->set_mode_index(0);
    auto *drillTarget = drillMode->add_targets();
    drillTarget->set_object_id(station->oid);
    drillTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(send(p1, castDrill, QStringLiteral("issue 147 cast Drill Too Deep")));
    ASSERT_TRUE(send(p1, stationPass, QStringLiteral("issue 147 pass Drill")));
    ASSERT_TRUE(send(p2, stationPass, QStringLiteral("issue 147 resolve Drill")));

    for (OpeningDriver *client : {&p1, &p2}) {
        const auto animated = findPermanent(*client, p1.myId, QStringLiteral("wurmwall_sweeper"));
        ASSERT_TRUE(animated);
        EXPECT_TRUE(animated->creature && animated->flying);
        EXPECT_EQ(animated->power, 2);
        EXPECT_EQ(animated->toughness, 2);
        EXPECT_EQ(client->serverCardByEngineOid.at(station->oid), stationPhysicalId);
        ASSERT_TRUE(client->annotationByServerCardId.count(stationPhysicalId));
        EXPECT_TRUE(
            client->annotationByServerCardId.at(stationPhysicalId).contains(QStringLiteral("8 charge counter(s)")));
    }

    // Sage of Fables proves that a source-absent counter choice crosses Rust, relay, and both
    // physical battlefield views without exposing the activating player's private legal cohort.
    ASSERT_TRUE(putPermanent("Sage of Fables", true));
    ASSERT_TRUE(putPermanent("Fugitive Wizard", false));
    const auto sage = findPermanent(p1, p1.myId, QStringLiteral("sage_of_fables"));
    const auto wizard = findPermanent(p1, p1.myId, QStringLiteral("fugitive_wizard"));
    ASSERT_TRUE(sage && wizard);
    EXPECT_EQ(wizard->power, 2);
    EXPECT_EQ(wizard->toughness, 2);
    const quint64 sageAbilityKey = static_cast<quint64>(sage->oid) << 32;
    const auto sageCosts = p1.latestLegal.cost_choices_by_ability().find(sageAbilityKey);
    ASSERT_NE(sageCosts, p1.latestLegal.cost_choices_by_ability().end());
    ASSERT_EQ(sageCosts->second.choices_size(), 1);
    const auto counterCost = sageCosts->second.choices(0);
    ASSERT_EQ(counterCost.kind(), ruled::v1::COST_CHOICE_KIND_REMOVE_COUNTERS);
    ASSERT_TRUE(counterCost.has_counter_removal());
    EXPECT_FALSE(counterCost.counter_removal().has_source());
    ASSERT_EQ(counterCost.counter_removal().options_size(), 1);
    EXPECT_EQ(counterCost.counter_removal().options(0).option_id(), 1u);
    ASSERT_EQ(counterCost.candidate_objects_size(), 1);
    EXPECT_EQ(counterCost.candidate_objects(0).object().object_id(), wizard->oid);
    EXPECT_EQ(counterCost.candidate_objects(0).object().zone_change_generation(), wizard->generation);
    EXPECT_EQ(counterCost.candidate_objects(0).contribution(), 1);
    EXPECT_EQ(p2.latestLegal.cost_choices_by_ability().count(sageAbilityKey), 0u);

    ruled::v1::RuledCommand sageActivation;
    auto *sageAbility = sageActivation.mutable_activate_ability();
    p1.setBattlefieldAbilitySource(sageAbility, sage->oid);
    sageAbility->set_ability_index(0);
    auto *counterSelection = sageAbility->add_cost_selections();
    counterSelection->set_cost_index(counterCost.cost_index());
    auto *counterRemoval = counterSelection->mutable_counter_removal();
    counterRemoval->set_option_id(counterCost.counter_removal().options(0).option_id());
    counterRemoval->mutable_source()->set_object_id(wizard->oid);
    counterRemoval->mutable_source()->set_zone_change_generation(wizard->generation);

    ruled::v1::RuledCommand sagePreviewQuery;
    auto *sagePreview = sagePreviewQuery.mutable_preview_payment();
    sagePreview->set_transaction_id(193);
    sagePreview->set_revision(1);
    *sagePreview->mutable_activate_ability() = *sageAbility;
    const int sagePreviewCount = p1.paymentPreviewCount;
    const auto sagePreviewVersion = p1.stateVersion;
    p1.sendRuled(sagePreviewQuery, QStringLiteral("issue 193 preview Sage activation payment"));
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.paymentPreviewCount == sagePreviewCount + 1; }, 10000, "Sage activation preview"));
    p2.pump(100);
    ASSERT_TRUE(p1.paymentPreview.valid()) << p1.paymentPreview.error();
    EXPECT_TRUE(p1.paymentPreview.candidates().empty());
    EXPECT_TRUE(p1.paymentPreview.selection().convoke().empty());
    EXPECT_EQ(p1.stateVersion, sagePreviewVersion);

    ruled::v1::RuledCommand addCounterAbilityMana;
    addCounterAbilityMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addCounterAbilityMana.mutable_dev_command()->mutable_add_mana()->set_c(1);
    ASSERT_TRUE(send(p1, addCounterAbilityMana, QStringLiteral("issue 193 add Sage activation mana")));
    const int sageHandBefore = p1.handSizeByPlayer[p1.myId];
    ASSERT_TRUE(send(p1, sageActivation, QStringLiteral("issue 193 activate Sage of Fables")));
    EXPECT_EQ(findPermanent(p1, p1.myId, QStringLiteral("fugitive_wizard"))->power, 1);
    EXPECT_EQ(findPermanent(p2, p1.myId, QStringLiteral("fugitive_wizard"))->power, 1);
    ruled::v1::RuledCommand passSage;
    passSage.mutable_pass_priority();
    ASSERT_TRUE(send(p1, passSage, QStringLiteral("issue 193 pass Sage ability")));
    ASSERT_TRUE(send(p2, passSage, QStringLiteral("issue 193 resolve Sage ability")));
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], sageHandBefore + 1);

    // Blight reuses the same physical picker, but accepts the already tapped, summoning-sick bear.
    ASSERT_TRUE(putPermanent("Gristle Glutton", true));
    ASSERT_TRUE(putPermanent("Tatterkite", false));
    const auto glutton = findPermanent(p1, p1.myId, QStringLiteral("gristle_glutton"));
    const auto kite = findPermanent(p1, p1.myId, QStringLiteral("tatterkite"));
    ASSERT_TRUE(glutton && kite);
    const auto blightCosts = p1.latestLegal.cost_choices_by_ability().find(static_cast<quint64>(glutton->oid) << 32);
    ASSERT_NE(blightCosts, p1.latestLegal.cost_choices_by_ability().end());
    ASSERT_EQ(blightCosts->second.choices_size(), 1);
    const auto &blight = blightCosts->second.choices(0);
    EXPECT_EQ(blight.kind(), ruled::v1::COST_CHOICE_KIND_BLIGHT);
    EXPECT_EQ(blight.blight_count(), 1u);
    EXPECT_EQ(blight.min(), 1u);
    EXPECT_EQ(blight.max(), 1u);
    EXPECT_TRUE(std::find(blight.candidate_ids().begin(), blight.candidate_ids().end(), bear->oid) !=
                blight.candidate_ids().end());
    EXPECT_TRUE(std::find(blight.candidate_ids().begin(), blight.candidate_ids().end(), kite->oid) ==
                blight.candidate_ids().end());
    ruled::v1::RuledCommand blightActivation;
    auto *blightAbility = blightActivation.mutable_activate_ability();
    p1.setBattlefieldAbilitySource(blightAbility, glutton->oid);
    blightAbility->set_ability_index(0);
    auto *blightSelection = blightAbility->add_cost_selections();
    blightSelection->set_cost_index(blight.cost_index());
    auto *blighted = blightSelection->mutable_battlefield_objects()->add_objects();
    blighted->set_object_id(bear->oid);
    blighted->set_zone_change_generation(bear->generation);
    ASSERT_TRUE(send(p1, blightActivation, QStringLiteral("Blight with tapped bear")));
    EXPECT_EQ(findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"))->toughness, 1);
    EXPECT_EQ(findPermanent(p2, p1.myId, QStringLiteral("grizzly_bears"))->toughness, 1);
    auto resolveToChoice = [&]() {
        for (int i = 0; i < 8 && !p1.pendingChoice && !p2.pendingChoice; ++i) {
            ruled::v1::RuledCommand pass;
            pass.mutable_pass_priority();
            if (!send(p1.priorityPlayer == p1.myId ? p1 : p2, pass, QStringLiteral("resolve Blight ability")))
                return false;
        }
        return p1.pendingChoice.has_value() || p2.pendingChoice.has_value();
    };
    ASSERT_TRUE(resolveToChoice());
    ASSERT_TRUE(p1.pendingChoice);
    EXPECT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_HAND_CARDS);
    EXPECT_FALSE(p2.pendingChoice);
    ASSERT_TRUE(p2.lastResolutionChoice);
    EXPECT_EQ(p2.lastResolutionChoice->candidate_object_ids_size(), 0);
    EXPECT_EQ(p2.lastResolutionChoice->candidate_names_size(), 0);
    const int handBefore = p1.handSizeByPlayer[p1.myId];
    ruled::v1::RuledCommand discard;
    discard.mutable_submit_resolution_choice()->add_chosen_object_ids(p1.pendingChoice->candidate_object_ids(0));
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, discard, QStringLiteral("Gristle discard then draw")));
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], handBefore);

    ASSERT_TRUE(putPermanent("Dream Seizer", false));
    ASSERT_TRUE(resolveToChoice());
    ASSERT_TRUE(p1.pendingChoice);
    EXPECT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    ruled::v1::RuledCommand branch;
    branch.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_SELECT_BRANCH);
    branch.mutable_submit_resolution_choice()->set_selected_branch_index(0);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, branch, QStringLiteral("Dream Seizer choose Blight")));
    ASSERT_TRUE(p1.pendingChoice);
    EXPECT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_COST_OBJECTS);
    EXPECT_EQ(p1.pendingChoice->candidate_server_card_ids_size(), p1.pendingChoice->candidate_object_ids_size());
    EXPECT_FALSE(p2.pendingChoice);
    ASSERT_TRUE(p2.lastResolutionChoice);
    EXPECT_EQ(p2.lastResolutionChoice->candidate_object_ids_size(), 0);
    EXPECT_EQ(p2.lastResolutionChoice->candidate_server_card_ids_size(), 0);
    const int physicalBear = p1.serverCardByEngineOid[bear->oid];
    ruled::v1::RuledCommand lethalBlight;
    lethalBlight.mutable_submit_resolution_choice()->add_chosen_object_ids(bear->oid);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, lethalBlight, QStringLiteral("Dream Seizer lethal Blight")));
    ASSERT_TRUE(p2.pendingChoice);
    EXPECT_EQ(p2.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_HAND_CARDS);
    EXPECT_FALSE(p1.pendingChoice);
    ASSERT_TRUE(findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears")));
    ASSERT_TRUE(findPermanent(p2, p1.myId, QStringLiteral("grizzly_bears")));
    EXPECT_EQ(p1.serverCardByEngineOid[bear->oid], physicalBear);
    EXPECT_EQ(p2.serverCardByEngineOid[bear->oid], physicalBear);
    ruled::v1::RuledCommand opponentDiscard;
    opponentDiscard.mutable_submit_resolution_choice()->add_chosen_object_ids(
        p2.pendingChoice->candidate_object_ids(0));
    p2.pendingChoice.reset();
    ASSERT_TRUE(send(p2, opponentDiscard, QStringLiteral("finish Dream Seizer discard")));
    EXPECT_FALSE(findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears")));
    EXPECT_FALSE(findPermanent(p2, p1.myId, QStringLiteral("grizzly_bears")));
    EXPECT_EQ(p1.graveyardOwnerByEngineOid[bear->oid], p1.myId);
    EXPECT_EQ(p2.graveyardOwnerByEngineOid[bear->oid], p1.myId);
}

TEST_F(RuledE2ESmokeTest, AggregatePowerAndManaValuePaymentsReachBothClientsWithExactPhysicalObjects)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("aggregatep1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("aggregatep2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "issue 178 game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "issue 178 game start (p2)"));
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
    auto putPermanent = [&](int playerId, const char *name) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(playerId);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(name);
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(true);
        return send(p1, command, QStringLiteral("issue 178 put %1").arg(name));
    };
    auto putInGraveyard = [&](const char *name) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(name);
        put->set_zone(ruled::v1::DEV_ZONE_HAND);
        if (!send(p1, command, QStringLiteral("issue 178 stage %1").arg(name))) {
            return false;
        }
        command.Clear();
        dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *move = dev->mutable_move_card();
        move->set_card_name(name);
        move->set_zone(ruled::v1::DEV_ZONE_GRAVEYARD);
        return send(p1, command, QStringLiteral("issue 178 graveyard %1").arg(name));
    };
    auto findPermanent = [](const OpeningDriver &client, int controller,
                            const QString &cardId) -> std::optional<OpeningDriver::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto permanent =
            std::find_if(battlefield->second.begin(), battlefield->second.end(),
                         [&cardId](const OpeningDriver::Permanent &candidate) { return candidate.cardId == cardId; });
        return permanent == battlefield->second.end() ? std::nullopt : std::optional(*permanent);
    };
    auto pass = [&]() {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(p1.priorityPlayer == p1.myId ? p1 : p2, command, QStringLiteral("resolve aggregate ability"));
    };

    ASSERT_TRUE(putPermanent(p1.myId, "Forensic Researcher"));
    ASSERT_TRUE(putPermanent(p2.myId, "Grizzly Bears"));
    ASSERT_TRUE(putInGraveyard("Lightning Bolt"));
    ASSERT_TRUE(putInGraveyard("Grizzly Bears"));
    const auto researcher = findPermanent(p1, p1.myId, QStringLiteral("forensic_researcher"));
    const auto opposingBear = findPermanent(p1, p2.myId, QStringLiteral("grizzly_bears"));
    ASSERT_TRUE(researcher && opposingBear);

    const quint64 researcherKey = (static_cast<quint64>(researcher->oid) << 32) | 1;
    const auto researcherCosts = p1.latestLegal.cost_choices_by_ability().find(researcherKey);
    ASSERT_NE(researcherCosts, p1.latestLegal.cost_choices_by_ability().end());
    const ruled::v1::LegalCostChoice *evidence = nullptr;
    for (const auto &choice : researcherCosts->second.choices()) {
        if (choice.zone() == ruled::v1::COST_CHOICE_ZONE_GRAVEYARD && choice.has_aggregate_minimum()) {
            evidence = &choice;
            break;
        }
    }
    ASSERT_NE(evidence, nullptr);
    EXPECT_EQ(evidence->aggregate_minimum().minimum(), 3u);
    EXPECT_EQ(evidence->aggregate_minimum().contribution_kind(), ruled::v1::OBJECT_CONTRIBUTION_KIND_MANA_VALUE);
    const ruled::v1::CostObjectCandidate *mvOne = nullptr;
    const ruled::v1::CostObjectCandidate *mvTwo = nullptr;
    for (const auto &candidate : evidence->candidate_objects()) {
        if (candidate.contribution() == 1)
            mvOne = &candidate;
        if (candidate.contribution() == 2)
            mvTwo = &candidate;
    }
    ASSERT_NE(mvOne, nullptr);
    ASSERT_NE(mvTwo, nullptr);
    ASSERT_TRUE(mvOne->has_object() && mvTwo->has_object());
    const quint32 boltOid = mvOne->object().object_id();
    const quint32 graveBearOid = mvTwo->object().object_id();
    ASSERT_EQ(p1.graveyardOwnerByEngineOid[boltOid], p1.myId);
    ASSERT_EQ(p2.graveyardOwnerByEngineOid[boltOid], p1.myId);
    ASSERT_EQ(p1.serverCardByEngineOid[boltOid], p2.serverCardByEngineOid[boltOid]);
    ASSERT_EQ(p1.serverCardByEngineOid[graveBearOid], p2.serverCardByEngineOid[graveBearOid]);

    ruled::v1::RuledCommand collectEvidence;
    auto *research = collectEvidence.mutable_activate_ability();
    p1.setBattlefieldAbilitySource(research, researcher->oid);
    research->set_ability_index(1);
    research->add_targets()->set_object_id(opposingBear->oid);
    auto *evidenceSelection = research->add_cost_selections();
    evidenceSelection->set_cost_index(evidence->cost_index());
    *evidenceSelection->mutable_graveyard_objects()->add_objects() = mvOne->object();
    *evidenceSelection->mutable_graveyard_objects()->add_objects() = mvTwo->object();
    ASSERT_TRUE(send(p1, collectEvidence, QStringLiteral("Forensic Researcher collect evidence 3")));
    EXPECT_TRUE(findPermanent(p1, p1.myId, QStringLiteral("forensic_researcher"))->tapped);
    EXPECT_TRUE(findPermanent(p2, p1.myId, QStringLiteral("forensic_researcher"))->tapped);
    EXPECT_EQ(p1.serverCardByEngineOid[boltOid], p2.serverCardByEngineOid[boltOid]);
    EXPECT_EQ(p1.serverCardByEngineOid[graveBearOid], p2.serverCardByEngineOid[graveBearOid]);
    ASSERT_TRUE(pass());
    ASSERT_TRUE(pass());
    EXPECT_TRUE(findPermanent(p1, p2.myId, QStringLiteral("grizzly_bears"))->tapped);
    EXPECT_TRUE(findPermanent(p2, p2.myId, QStringLiteral("grizzly_bears"))->tapped);

    ASSERT_TRUE(putPermanent(p1.myId, "Mossbridge Troll"));
    ASSERT_TRUE(putPermanent(p1.myId, "Colossal Dreadmaw"));
    ASSERT_TRUE(putPermanent(p1.myId, "Serra Angel"));
    const auto troll = findPermanent(p1, p1.myId, QStringLiteral("mossbridge_troll"));
    ASSERT_TRUE(troll);
    const auto trollCosts = p1.latestLegal.cost_choices_by_ability().find(static_cast<quint64>(troll->oid) << 32);
    ASSERT_NE(trollCosts, p1.latestLegal.cost_choices_by_ability().end());
    ASSERT_EQ(trollCosts->second.choices_size(), 1);
    const auto &powerCost = trollCosts->second.choices(0);
    ASSERT_TRUE(powerCost.has_aggregate_minimum());
    EXPECT_EQ(powerCost.aggregate_minimum().minimum(), 10u);
    EXPECT_EQ(powerCost.aggregate_minimum().contribution_kind(), ruled::v1::OBJECT_CONTRIBUTION_KIND_CURRENT_POWER);
    std::vector<ruled::v1::CostObjectRef> powerObjects;
    int totalPower = 0;
    for (const auto &candidate : powerCost.candidate_objects()) {
        EXPECT_NE(candidate.object().object_id(), troll->oid);
        if (candidate.contribution() == 6 || candidate.contribution() == 4) {
            powerObjects.push_back(candidate.object());
            totalPower += static_cast<int>(candidate.contribution());
        }
    }
    ASSERT_EQ(totalPower, 10);
    ASSERT_EQ(powerObjects.size(), 2u);

    ruled::v1::RuledCommand pumpTroll;
    auto *pump = pumpTroll.mutable_activate_ability();
    p1.setBattlefieldAbilitySource(pump, troll->oid);
    auto *powerSelection = pump->add_cost_selections();
    powerSelection->set_cost_index(powerCost.cost_index());
    for (const auto &object : powerObjects) {
        *powerSelection->mutable_battlefield_objects()->add_objects() = object;
    }
    ASSERT_TRUE(send(p1, pumpTroll, QStringLiteral("Mossbridge Troll tap power ten")));
    for (const auto &object : powerObjects) {
        ASSERT_TRUE(p1.serverCardByEngineOid.count(object.object_id()));
        ASSERT_TRUE(p2.serverCardByEngineOid.count(object.object_id()));
        EXPECT_EQ(p1.serverCardByEngineOid[object.object_id()], p2.serverCardByEngineOid[object.object_id()]);
        EXPECT_TRUE(p1.physicallyTappedCardIds.count(p1.serverCardByEngineOid[object.object_id()]));
        EXPECT_TRUE(p2.physicallyTappedCardIds.count(p2.serverCardByEngineOid[object.object_id()]));
    }
    EXPECT_FALSE(findPermanent(p1, p1.myId, QStringLiteral("mossbridge_troll"))->tapped);
    ASSERT_TRUE(pass());
    ASSERT_TRUE(pass());
    EXPECT_EQ(findPermanent(p1, p1.myId, QStringLiteral("mossbridge_troll"))->power, 25);
    EXPECT_EQ(findPermanent(p2, p1.myId, QStringLiteral("mossbridge_troll"))->power, 25);
}

TEST_F(RuledE2ESmokeTest, DiscardReplacementPrivacyAndMadnessPaymentReachBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("discardp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("discardp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "issue 197 game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "issue 197 game start (p2)"));
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
    auto put = [&](int player, const char *name, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(player);
        auto *placement = dev->mutable_put_card_in_zone();
        placement->set_card_name(name);
        placement->set_zone(zone);
        placement->set_ready(ready);
        return send(p1, command, QStringLiteral("issue 197 put %1").arg(name));
    };
    auto pass = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("issue 197 pass"));
    };

    ASSERT_TRUE(put(p2.myId, "Library of Leng", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(put(p2.myId, "Mountain", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    auto choose = [&](std::initializer_list<quint32> ids) {
        ruled::v1::RuledCommand command;
        for (auto id : ids)
            command.mutable_submit_resolution_choice()->add_chosen_object_ids(id);
        p2.pendingChoice.reset();
        return send(p2, command, QStringLiteral("issue 197 discard choice"));
    };
    for (const int destination : {1, 2}) {
        ASSERT_TRUE(put(p1.myId, "Mind Rot", ruled::v1::DEV_ZONE_HAND, false));
        ASSERT_TRUE(put(p2.myId, "Fiery Temper", ruled::v1::DEV_ZONE_HAND, false));
        ruled::v1::RuledCommand mana;
        mana.mutable_dev_command()->set_target_player_id(p1.myId);
        mana.mutable_dev_command()->mutable_add_mana()->set_b(3);
        ASSERT_TRUE(send(p1, mana, QStringLiteral("issue 197 discard spell mana")));
        const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Mind Rot"));
        ASSERT_NE(action, nullptr);
        ruled::v1::RuledCommand spell;
        spell.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        spell.mutable_cast_spell()->mutable_source()->set_hand_index(action->hand_index());
        auto *target = spell.mutable_cast_spell()->add_targets();
        target->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
        target->set_object_id(p2.myId);
        ASSERT_TRUE(send(p1, spell, QStringLiteral("issue 197 Mind Rot")));
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        ASSERT_TRUE(p2.pendingChoice.has_value());
        quint32 fiery = 0, island = 0;
        int physicalId = -1;
        for (int index = 0; index < p2.pendingChoice->candidate_names_size(); ++index) {
            const auto &name = p2.pendingChoice->candidate_names(index);
            if (name == "Fiery Temper") {
                fiery = p2.pendingChoice->candidate_object_ids(index);
                physicalId = p2.pendingChoice->candidate_server_card_ids(index);
            } else if (name == "Island")
                island = p2.pendingChoice->candidate_object_ids(index);
        }
        ASSERT_NE(fiery, 0u);
        ASSERT_NE(island, 0u);
        ASSERT_GE(physicalId, 0);
        ASSERT_TRUE(choose({fiery, island}));
        ASSERT_TRUE(p2.pendingChoice.has_value());
        EXPECT_EQ(p2.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_PRIVATE_REPLACEMENT);
        ASSERT_TRUE(p1.lastResolutionChoice.has_value());
        EXPECT_EQ(p1.lastResolutionChoice->candidate_names_size(), 0);
        EXPECT_EQ(p1.lastResolutionChoice->candidate_object_ids_size(), 0);
        ASSERT_TRUE(choose({static_cast<quint32>(destination)}));
        ASSERT_TRUE(choose({0}));
        if (destination == 1) {
            EXPECT_EQ(p1.stackDepth, 0);
            EXPECT_TRUE(
                std::any_of(p2.physicalMoveEvents.begin(), p2.physicalMoveEvents.end(), [physicalId](const auto &move) {
                    return move.card_id() == physicalId && move.start_zone() == ZoneNames::HAND &&
                           move.target_zone() == ZoneNames::DECK;
                }));
            continue;
        }
        ASSERT_TRUE(p1.serverCardByEngineOid.count(fiery));
        ASSERT_TRUE(p2.serverCardByEngineOid.count(fiery));
        EXPECT_EQ(p2.serverCardByEngineOid[fiery], physicalId);
        EXPECT_EQ(p1.serverCardByEngineOid[fiery], physicalId);
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        ASSERT_TRUE(p2.pendingChoice.has_value());
        EXPECT_EQ(p2.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_SPECIAL_CAST);
        ASSERT_EQ(p2.latestLegal.zone_cast_actions_size(), 1);
        EXPECT_EQ(p1.latestLegal.zone_cast_actions_size(), 0);
        const auto offer = p2.latestLegal.zone_cast_actions(0);
        EXPECT_EQ(offer.cast_method(), ruled::v1::CAST_METHOD_MADNESS);
        EXPECT_TRUE(p2.latestLegal.exile_play_permission_groups().empty());
        const auto &permanents = p2.battlefieldByPlayer[p2.myId];
        const auto mountain = std::find_if(permanents.begin(), permanents.end(),
                                           [](const auto &card) { return card.cardId == "mountain"; });
        ASSERT_NE(mountain, permanents.end());
        ruled::v1::RuledCommand accepted;
        auto *choice = accepted.mutable_submit_resolution_choice();
        choice->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_CAST_SPELL);
        auto *cast = choice->mutable_cast_spell();
        cast->set_cast_method(offer.cast_method());
        cast->set_face_index(offer.face_index());
        cast->set_casting_permission_id(offer.casting_permission_id());
        cast->mutable_source()->set_exile_object_id(fiery);
        cast->mutable_source()->set_expected_zone_change_generation(offer.zone_change_generation());
        target = cast->add_targets();
        target->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
        target->set_object_id(p1.myId);
        ruled::v1::RuledCommand query;
        auto *preview = query.mutable_preview_payment();
        preview->set_transaction_id(197);
        preview->set_revision(1);
        *preview->mutable_cast_spell() = *cast;
        preview->mutable_cast_spell()->mutable_payment();
        int previewCount = p2.paymentPreviewCount;
        p2.sendRuled(query, QStringLiteral("issue 197 preview madness before mana"));
        ASSERT_TRUE(
            p2.pumpUntil([&] { return p2.paymentPreviewCount == previewCount + 1; }, 10000, "madness unpaid preview"));
        ASSERT_TRUE(p2.paymentPreview.valid()) << p2.paymentPreview.error();
        EXPECT_FALSE(p2.paymentPreview.complete());
        ruled::v1::RuledCommand activate;
        p2.setBattlefieldAbilitySource(activate.mutable_activate_ability(), mountain->oid);
        ASSERT_TRUE(send(p2, activate, QStringLiteral("issue 197 mana during madness offer")));
        ASSERT_TRUE(p2.pendingChoice.has_value());
        preview->set_revision(2);
        preview->mutable_cast_spell()->mutable_payment()->mutable_mana()->set_r(1);
        previewCount = p2.paymentPreviewCount;
        p2.sendRuled(query, QStringLiteral("issue 197 preview selected red mana"));
        ASSERT_TRUE(
            p2.pumpUntil([&] { return p2.paymentPreviewCount == previewCount + 1; }, 10000, "madness paid preview"));
        ASSERT_TRUE(p2.paymentPreview.valid()) << p2.paymentPreview.error();
        ASSERT_TRUE(p2.paymentPreview.complete());
        EXPECT_EQ(p1.paymentPreviewCount, 0);
        *cast->mutable_payment() = p2.paymentPreview.selection();
        p2.pendingChoice.reset();
        ASSERT_TRUE(send(p2, accepted, QStringLiteral("issue 197 cast Fiery Temper")));
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        EXPECT_EQ(p1.lifeByPlayer[p1.myId], 17);
        EXPECT_EQ(p2.lifeByPlayer[p1.myId], 17);
        for (OpeningDriver *client : {&p1, &p2}) {
            const auto toStack = std::find_if(
                client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(), [physicalId](const auto &move) {
                    return move.card_id() == physicalId && move.start_zone() == ZoneNames::EXILE &&
                           move.target_zone() == ZoneNames::STACK;
                });
            ASSERT_NE(toStack, client->physicalMoveEvents.end());
            const int stackId = toStack->new_card_id();
            const auto toGrave = std::find_if(
                client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(), [stackId](const auto &move) {
                    return move.card_id() == stackId && move.start_zone() == ZoneNames::STACK &&
                           move.target_zone() == ZoneNames::GRAVE;
                });
            ASSERT_NE(toGrave, client->physicalMoveEvents.end());
            // Crossing the shared stack between seats reissues the server's seat-local id.
            // Follow the explicit old/new id chain for the same physical card.
            EXPECT_EQ(client->serverCardByEngineOid[fiery], toGrave->new_card_id());
            EXPECT_EQ(toGrave->target_player_id(), p2.myId);
        }
    }
}

} // namespace
} // namespace ruled_e2e
