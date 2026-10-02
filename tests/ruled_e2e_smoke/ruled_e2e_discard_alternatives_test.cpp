#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
TEST_F(RuledE2ESmokeTest, AlternativeDiscardChoicesRemainPrivateAndBindPhysicalCards)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    OpeningDriver p1(true, QStringLiteral("discardp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("discardp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "discard start p1"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "discard start p2"));
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
        sender.sendRuled(command, QStringLiteral("alternative discard"));
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto put = [&](const char *name) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        dev->mutable_put_card_in_zone()->set_zone(ruled::v1::DEV_ZONE_HAND);
        return send(p1, command);
    };
    for (bool harmonize : {false, true}) {
        ASSERT_TRUE(put("Grizzly Bears"));
        if (!harmonize) {
            ASSERT_TRUE(put("Winternight Stories"));
        }
        ruled::v1::RuledCommand mana;
        mana.mutable_dev_command()->set_target_player_id(p1.myId);
        mana.mutable_dev_command()->mutable_add_mana()->set_u(harmonize ? 5 : 3);
        ASSERT_TRUE(send(p1, mana));
        ruled::v1::RuledCommand cast;
        auto *spell = cast.mutable_cast_spell();
        quint32 spellOid = 0;
        if (harmonize) {
            for (const auto &action : p1.latestLegal.zone_cast_actions()) {
                if (action.card_name() == "Winternight Stories" &&
                    action.cast_method() == ruled::v1::CAST_METHOD_HARMONIZE) {
                    spellOid = action.object_id();
                    spell->mutable_source()->set_graveyard_object_id(spellOid);
                    spell->mutable_source()->set_expected_zone_change_generation(action.zone_change_generation());
                    spell->set_cast_method(ruled::v1::CAST_METHOD_HARMONIZE);
                }
            }
            ASSERT_NE(spellOid, 0u);
        } else {
            const auto *action =
                p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Winternight Stories"));
            ASSERT_NE(action, nullptr);
            spell->mutable_source()->set_hand_index(action->hand_index());
            spell->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        }
        const int handBefore = p1.handSizeByPlayer[p1.myId];
        ASSERT_TRUE(send(p1, cast));
        ruled::v1::RuledCommand pass;
        pass.mutable_pass_priority();
        ASSERT_TRUE(send(p1, pass));
        ASSERT_TRUE(send(p2, pass));
        ASSERT_TRUE(p1.pendingChoice.has_value());
        const auto choice = *p1.pendingChoice;
        EXPECT_EQ(choice.choice_kind(), ruled::v1::CHOICE_KIND_HAND_CARDS);
        ASSERT_EQ(choice.selection_alternatives_size(), 2);
        EXPECT_EQ(p1.handSizeByPlayer[p1.myId], handBefore + (harmonize ? 3 : 2));
        ASSERT_TRUE(p2.lastResolutionChoice.has_value());
        EXPECT_EQ(p2.lastResolutionChoice->candidate_object_ids_size(), 0);
        EXPECT_EQ(p2.lastResolutionChoice->candidate_names_size(), 0);
        EXPECT_EQ(p2.lastResolutionChoice->candidate_server_card_ids_size(), 0);
        EXPECT_EQ(p2.lastResolutionChoice->selection_alternatives_size(), 0);
        EXPECT_FALSE(p2.pendingChoice.has_value());
        std::vector<std::pair<quint32, int>> selected;
        for (int i = 0; i < choice.candidate_names_size(); ++i) {
            if ((!harmonize && choice.candidate_names(i) == "Grizzly Bears") ||
                (harmonize && choice.candidate_names(i) == "Island" && selected.size() < 2)) {
                selected.emplace_back(choice.candidate_object_ids(i), choice.candidate_server_card_ids(i));
            }
        }
        ASSERT_EQ(selected.size(), harmonize ? 2u : 1u);
        ruled::v1::RuledCommand answer;
        for (const auto &[oid, scid] : selected) {
            EXPECT_GE(scid, 0);
            answer.mutable_submit_resolution_choice()->add_chosen_object_ids(oid);
        }
        ASSERT_TRUE(send(p1, answer));
        EXPECT_EQ(p1.stackDepth, 0);
        EXPECT_EQ(p2.stackDepth, 0);
        for (const auto &[oid, scid] : selected) {
            ASSERT_TRUE(p1.graveyardOwnerByEngineOid.count(oid));
            ASSERT_TRUE(p2.graveyardOwnerByEngineOid.count(oid));
            EXPECT_EQ(p1.serverCardByEngineOid.at(oid), scid);
            EXPECT_EQ(p2.serverCardByEngineOid.at(oid), scid);
        }
        if (harmonize) {
            EXPECT_TRUE(p1.serverCardByEngineOid.count(spellOid));
            EXPECT_TRUE(p2.serverCardByEngineOid.count(spellOid));
            EXPECT_FALSE(std::any_of(p1.latestLegal.zone_cast_actions().begin(),
                                     p1.latestLegal.zone_cast_actions().end(),
                                     [&](const auto &action) { return action.object_id() == spellOid; }));
            for (const auto *recipient : {&p1, &p2}) {
                EXPECT_TRUE(std::any_of(
                    recipient->physicalMoveEvents.begin(), recipient->physicalMoveEvents.end(), [&](const auto &move) {
                        return move.target_zone() == ZoneNames::EXILE && move.card_name() == "Winternight Stories" &&
                               move.new_card_id() == recipient->serverCardByEngineOid.at(spellOid);
                    }));
            }
        }
    }
}
TEST_F(RuledE2ESmokeTest, FranticSearchPrivateDiscardThenPublicOpposingLandCohort)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << std::string(started.message()).substr(5);
    OpeningDriver p1(true, QStringLiteral("franticp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("franticp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Frantic start p1"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Frantic start p2"));
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
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    auto send = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command) {
        const auto before1 = p1.stateVersion;
        const auto before2 = p2.stateVersion;
        sender.sendRuled(command, QStringLiteral("Frantic Search"));
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto put = [&](int player, const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(player);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        dev->mutable_put_card_in_zone()->set_zone(zone);
        return send(p1, command);
    };
    ASSERT_TRUE(put(p1.myId, "Izzet Guildgate", ruled::v1::DEV_ZONE_BATTLEFIELD));
    for (int index = 0; index < 4; ++index)
        ASSERT_TRUE(put(p2.myId, "Izzet Guildgate", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put(p1.myId, "Sol Ring", ruled::v1::DEV_ZONE_BATTLEFIELD));
    std::vector<quint32> allLands;
    std::vector<quint32> opposingLands;
    for (int player : {p1.myId, p2.myId}) {
        for (const auto &permanent : p1.battlefieldByPlayer[player]) {
            if (permanent.cardId != "izzet_guildgate")
                continue;
            ASSERT_TRUE(permanent.tapped);
            allLands.push_back(permanent.oid);
            if (player == p2.myId)
                opposingLands.push_back(permanent.oid);
        }
    }
    ASSERT_EQ(allLands.size(), 5u);
    ASSERT_EQ(opposingLands.size(), 4u);
    std::map<quint32, int> bindings;
    for (quint32 oid : allLands) {
        ASSERT_TRUE(p1.serverCardByEngineOid.count(oid));
        bindings[oid] = p1.serverCardByEngineOid.at(oid);
        EXPECT_EQ(p2.serverCardByEngineOid.at(oid), bindings[oid]);
    }
    for (bool chooseThree : {false, true}) {
        ASSERT_TRUE(put(p1.myId, "Frantic Search", ruled::v1::DEV_ZONE_HAND));
        ruled::v1::RuledCommand mana;
        mana.mutable_dev_command()->set_target_player_id(p1.myId);
        mana.mutable_dev_command()->mutable_add_mana()->set_u(1);
        mana.mutable_dev_command()->mutable_add_mana()->set_c(2);
        ASSERT_TRUE(send(p1, mana));
        const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Frantic Search"));
        ASSERT_NE(action, nullptr);
        const int handBefore = p1.handSizeByPlayer[p1.myId];
        ruled::v1::RuledCommand cast;
        auto *spell = cast.mutable_cast_spell();
        spell->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        spell->mutable_source()->set_hand_index(action->hand_index());
        spell->mutable_payment()->mutable_mana()->set_u(1);
        spell->mutable_payment()->mutable_mana()->set_c(2);
        ASSERT_TRUE(send(p1, cast));
        QElapsedTimer paid;
        paid.start();
        while ((p1.stackDepth != 1 || p2.stackDepth != 1 || p1.myPool.total() != 0) && paid.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        ASSERT_EQ(p1.stackDepth, 1);
        ASSERT_EQ(p2.stackDepth, 1);
        ASSERT_EQ(p1.myPool.total(), 0);
        ruled::v1::RuledCommand pass;
        pass.mutable_pass_priority();
        ASSERT_TRUE(send(p1, pass));
        ASSERT_TRUE(send(p2, pass));
        ASSERT_TRUE(p1.pendingChoice.has_value());
        const auto discard = *p1.pendingChoice;
        ASSERT_EQ(discard.choice_kind(), ruled::v1::CHOICE_KIND_HAND_CARDS);
        EXPECT_EQ(discard.deciding_player_id(), p1.myId);
        EXPECT_EQ(discard.min(), 2u);
        EXPECT_EQ(discard.max(), 2u);
        EXPECT_EQ(p1.handSizeByPlayer[p1.myId], handBefore + 1);
        ASSERT_GE(discard.candidate_object_ids_size(), 2);
        ASSERT_EQ(discard.candidate_object_ids_size(), discard.candidate_names_size());
        ASSERT_EQ(discard.candidate_object_ids_size(), discard.candidate_server_card_ids_size());
        ASSERT_TRUE(p2.lastResolutionChoice.has_value());
        EXPECT_EQ(p2.lastResolutionChoice->candidate_object_ids_size(), 0);
        EXPECT_EQ(p2.lastResolutionChoice->candidate_names_size(), 0);
        EXPECT_EQ(p2.lastResolutionChoice->candidate_server_card_ids_size(), 0);
        EXPECT_FALSE(p2.pendingChoice.has_value());
        std::vector<std::pair<quint32, int>> discarded;
        ruled::v1::RuledCommand discardAnswer;
        for (int index = 0; index < 2; ++index) {
            const int scid = discard.candidate_server_card_ids(index);
            ASSERT_GE(scid, 0);
            EXPECT_TRUE(std::any_of(p1.handServerCardBySlot.begin(), p1.handServerCardBySlot.end(),
                                   [&](const auto &slot) { return slot.second == scid; }));
            discarded.emplace_back(discard.candidate_object_ids(index), scid);
            discardAnswer.mutable_submit_resolution_choice()->add_chosen_object_ids(discard.candidate_object_ids(index));
        }
        EXPECT_NE(discarded[0].second, discarded[1].second);
        p1.pendingChoice.reset();
        ASSERT_TRUE(send(p1, discardAnswer));
        for (const auto &[oid, scid] : discarded) {
            EXPECT_EQ(p1.serverCardByEngineOid.at(oid), scid);
            EXPECT_EQ(p2.serverCardByEngineOid.at(oid), scid);
            EXPECT_EQ(p1.graveyardOwnerByEngineOid.at(oid), p1.myId);
            EXPECT_EQ(p2.graveyardOwnerByEngineOid.at(oid), p1.myId);
        }
        ASSERT_TRUE(p1.pendingChoice.has_value());
        const auto landChoice = *p1.pendingChoice;
        ASSERT_EQ(landChoice.choice_kind(), ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
        EXPECT_EQ(landChoice.min(), 0u);
        EXPECT_EQ(landChoice.max(), 3u);
        EXPECT_EQ(landChoice.candidate_object_ids_size(), 5);
        ASSERT_TRUE(p2.lastResolutionChoice.has_value());
        EXPECT_EQ(p2.lastResolutionChoice->choice_kind(), ruled::v1::CHOICE_KIND_PERMANENT_OBJECTS);
        std::set<quint32> candidates(landChoice.candidate_object_ids().begin(), landChoice.candidate_object_ids().end());
        const auto &publicIds = p2.lastResolutionChoice->candidate_object_ids();
        EXPECT_EQ(candidates, std::set<quint32>(publicIds.begin(), publicIds.end()));
        EXPECT_EQ(candidates, std::set<quint32>(allLands.begin(), allLands.end()));
        EXPECT_FALSE(p2.pendingChoice.has_value());
        ruled::v1::RuledCommand landAnswer;
        auto *selection = landAnswer.mutable_submit_resolution_choice();
        std::set<quint32> selected;
        if (chooseThree) {
            for (int index = 0; index < 3; ++index) {
                selection->add_chosen_object_ids(opposingLands[index]);
                selected.insert(opposingLands[index]);
            }
        }
        p1.pendingChoice.reset();
        ASSERT_TRUE(send(p1, landAnswer));
        EXPECT_EQ(p1.stackDepth, 0);
        EXPECT_EQ(p2.stackDepth, 0);
        for (const auto *recipient : {&p1, &p2}) {
            for (int player : {p1.myId, p2.myId}) {
                for (const auto &permanent : recipient->battlefieldByPlayer.at(player)) {
                    if (candidates.count(permanent.oid)) {
                        EXPECT_EQ(permanent.tapped, selected.count(permanent.oid) == 0);
                        EXPECT_EQ(recipient->physicallyTappedCardIds.count({player, bindings.at(permanent.oid)}) != 0,
                                  selected.count(permanent.oid) == 0);
                        EXPECT_EQ(recipient->serverCardByEngineOid.at(permanent.oid), bindings.at(permanent.oid));
                    }
                }
            }
        }
    }
}
TEST_F(RuledE2ESmokeTest, DrawReplacementPartialHandsRemainPrivateAndBrainstormBindingsStayStable)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << std::string(started.message()).substr(5);
    OpeningDriver p1(true, QStringLiteral("drawreplacep1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("drawreplacep2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{60, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{60, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "draw replacement start"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "draw replacement start"));
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
    auto send = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command) {
        const auto before1 = p1.stateVersion;
        const auto before2 = p2.stateVersion;
        sender.sendRuled(command, QStringLiteral("draw replacement physical privacy"));
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    ruled::v1::RuledCommand pass;
    pass.mutable_pass_priority();
    for (const char *name : {"Thought Reflection", "Teferi's Ageless Insight", "Brainstorm"}) {
        ruled::v1::RuledCommand put;
        auto *dev = put.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        dev->mutable_put_card_in_zone()->set_zone(ruled::v1::DEV_ZONE_HAND);
        ASSERT_TRUE(send(p1, put));
        const int blue = std::string(name) == "Thought Reflection" ? 3 : std::string(name) == "Brainstorm" ? 1 : 2;
        const int colorless = std::string(name) == "Thought Reflection" ? 4 : std::string(name) == "Brainstorm" ? 0 : 2;
        ruled::v1::RuledCommand mana;
        auto *add = mana.mutable_dev_command();
        add->set_target_player_id(p1.myId);
        add->mutable_add_mana()->set_u(blue);
        add->mutable_add_mana()->set_c(colorless);
        ASSERT_TRUE(send(p1, mana));
        const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QString::fromUtf8(name));
        ASSERT_NE(action, nullptr);
        ruled::v1::RuledCommand cast;
        auto *spell = cast.mutable_cast_spell();
        spell->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        spell->mutable_source()->set_hand_index(action->hand_index());
        spell->mutable_payment()->mutable_mana()->set_u(blue);
        spell->mutable_payment()->mutable_mana()->set_c(colorless);
        ASSERT_TRUE(send(p1, cast));
        ASSERT_EQ(p1.myPool.total(), 0);
        ASSERT_EQ(p1.stackDepth, 1);
        const int handBefore = p1.handSizeByPlayer[p1.myId];
        const int opponentHand = p2.handSizeByPlayer[p2.myId];
        const auto opponentSlots = p2.handServerCardBySlot;
        ASSERT_TRUE(send(p1, pass));
        ASSERT_TRUE(send(p2, pass));
        if (std::string(name) != "Brainstorm") {
            EXPECT_EQ(p1.stackDepth, 0);
            EXPECT_EQ(p2.stackDepth, 0);
            continue;
        }
        std::set<quint32> oldHandles;
        std::set<int> priorHandBindings;
        for (const auto &[slot, scid] : p1.handServerCardBySlot)
            priorHandBindings.insert(scid);
        for (int original = 0; original < 3; ++original) {
            ASSERT_TRUE(p1.pendingChoice.has_value());
            const auto choice = *p1.pendingChoice;
            ASSERT_EQ(choice.choice_kind(), ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT);
            ASSERT_EQ(choice.replacement_options_size(), 2);
            EXPECT_EQ(choice.deciding_player_id(), p1.myId);
            EXPECT_EQ(choice.min(), 1u);
            EXPECT_EQ(choice.max(), 1u);
            EXPECT_EQ(p1.handSizeByPlayer[p1.myId], handBefore + original * 4);
            EXPECT_EQ(p2.physicalHandCountByPlayer[p1.myId], handBefore + original * 4);
            for (const auto &name : p2.physicalHandNamesByPlayer[p1.myId])
                EXPECT_TRUE(name.empty());
            EXPECT_EQ(p2.handServerCardBySlot, opponentSlots);
            EXPECT_EQ(p2.handSizeByPlayer[p2.myId], opponentHand);
            ASSERT_TRUE(p2.lastResolutionChoice.has_value());
            EXPECT_FALSE(p2.pendingChoice.has_value());
            EXPECT_EQ(p2.lastResolutionChoice->candidate_names_size(), 0);
            EXPECT_EQ(p2.lastResolutionChoice->candidate_card_ids_size(), 0);
            EXPECT_EQ(p2.lastResolutionChoice->candidate_server_card_ids_size(), 0);
            quint32 selected = 0;
            for (const auto &option : choice.replacement_options()) {
                EXPECT_FALSE(oldHandles.count(option.application_id()));
                oldHandles.insert(option.application_id());
                EXPECT_TRUE(p1.serverCardByEngineOid.count(option.source_object_id()));
                EXPECT_EQ(p1.serverCardByEngineOid.at(option.source_object_id()),
                          p2.serverCardByEngineOid.at(option.source_object_id()));
                if (option.source_card_name() == "Teferi's Ageless Insight")
                    selected = option.application_id();
            }
            ASSERT_NE(selected, 0u);
            ruled::v1::RuledCommand answer;
            answer.mutable_submit_resolution_choice()->add_chosen_object_ids(selected);
            p1.pendingChoice.reset();
            ASSERT_TRUE(send(p1, answer));
            std::set<int> current;
            for (const auto &[slot, scid] : p1.handServerCardBySlot)
                current.insert(scid);
            EXPECT_EQ(current.size(), static_cast<size_t>(handBefore + (original + 1) * 4));
            for (int scid : priorHandBindings)
                EXPECT_TRUE(current.count(scid));
            priorHandBindings = current;
        }
        ASSERT_TRUE(p1.pendingChoice.has_value());
        const auto hand = *p1.pendingChoice;
        ASSERT_EQ(hand.choice_kind(), ruled::v1::CHOICE_KIND_HAND_CARDS);
        EXPECT_TRUE(hand.ordered());
        ASSERT_EQ(hand.candidate_object_ids_size(), handBefore + 12);
        ASSERT_EQ(hand.candidate_server_card_ids_size(), hand.candidate_object_ids_size());
        ASSERT_TRUE(p2.lastResolutionChoice.has_value());
        EXPECT_EQ(p2.lastResolutionChoice->candidate_object_ids_size(), 0);
        EXPECT_EQ(p2.lastResolutionChoice->candidate_names_size(), 0);
        EXPECT_EQ(p2.lastResolutionChoice->candidate_server_card_ids_size(), 0);
        std::set<int> returned;
        ruled::v1::RuledCommand putBack;
        for (int index = 0; index < 2; ++index) {
            const int scid = hand.candidate_server_card_ids(index);
            EXPECT_TRUE(priorHandBindings.count(scid));
            returned.insert(scid);
            putBack.mutable_submit_resolution_choice()->add_chosen_object_ids(hand.candidate_object_ids(index));
        }
        p1.pendingChoice.reset();
        ASSERT_TRUE(send(p1, putBack));
        EXPECT_EQ(p1.stackDepth, 0);
        EXPECT_EQ(p2.stackDepth, 0);
        EXPECT_EQ(p1.handSizeByPlayer[p1.myId], handBefore + 10);
        EXPECT_EQ(p2.physicalHandCountByPlayer[p1.myId], handBefore + 10);
        for (const auto &name : p2.physicalHandNamesByPlayer[p1.myId])
            EXPECT_TRUE(name.empty());
        EXPECT_EQ(p2.handServerCardBySlot, opponentSlots);
        for (const auto &[slot, scid] : p1.handServerCardBySlot) {
            EXPECT_TRUE(priorHandBindings.count(scid));
            EXPECT_FALSE(returned.count(scid));
        }
    }
}
} // namespace ruled_e2e
