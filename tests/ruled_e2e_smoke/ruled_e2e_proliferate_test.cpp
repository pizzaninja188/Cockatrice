#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
std::optional<ObservedState::Permanent> permanent(const OpeningDriver &client, int controller,
                                                 const QString &cardId)
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

TEST_F(RuledE2ESmokeTest, ProliferateMixedRecipientsKeepsIdentityDomainsAndPublishesCountersToBothClients)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("prolifp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("prolifp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "Proliferate E2E start p1"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "Proliferate E2E start p2"));
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

    const auto send = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command,
                          const QString &description) {
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
    const auto pass = [&](OpeningDriver &sender) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(sender, command, QStringLiteral("Proliferate E2E pass"));
    };
    const auto put = [&](int player, const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(player);
        auto *placement = dev->mutable_put_card_in_zone();
        placement->set_card_name(name);
        placement->set_zone(zone);
        placement->set_ready(true);
        return send(p1, command, QStringLiteral("Proliferate E2E put %1").arg(name));
    };

    ASSERT_TRUE(put(p1.myId, "Jace Beleren", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put(p1.myId, "Tezzeret's Gambit", ruled::v1::DEV_ZONE_HAND));
    ruled::v1::RuledCommand poison;
    poison.mutable_dev_command()->set_target_player_id(p2.myId);
    poison.mutable_dev_command()->mutable_add_poison_counters()->set_count(1);
    ASSERT_TRUE(send(p1, poison, QStringLiteral("Proliferate E2E prepare opponent poison")));
    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_u(1);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(3);
    ASSERT_TRUE(send(p1, mana, QStringLiteral("Proliferate E2E mana")));

    const auto jace = permanent(p1, p1.myId, QStringLiteral("jace_beleren"));
    ASSERT_TRUE(jace.has_value());
    ASSERT_EQ(jace->loyalty, 3);
    const auto *gambit = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Tezzeret's Gambit"));
    ASSERT_NE(gambit, nullptr);
    ruled::v1::RuledCommand cast;
    cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast.mutable_cast_spell()->mutable_source()->set_hand_index(gambit->hand_index());
    ASSERT_TRUE(send(p1, cast, QStringLiteral("Proliferate E2E cast Tezzeret's Gambit")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));

    ASSERT_TRUE(p1.pendingChoice.has_value());
    ASSERT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_PROLIFERATE);
    ASSERT_TRUE(std::find(p1.pendingChoice->candidate_object_ids().begin(),
                          p1.pendingChoice->candidate_object_ids().end(), jace->oid) !=
                p1.pendingChoice->candidate_object_ids().end());
    ASSERT_TRUE(std::find(p1.pendingChoice->candidate_player_ids().begin(),
                          p1.pendingChoice->candidate_player_ids().end(), p2.myId) !=
                p1.pendingChoice->candidate_player_ids().end());
    ASSERT_TRUE(p2.lastResolutionChoice.has_value());
    EXPECT_TRUE(std::find(p2.lastResolutionChoice->candidate_object_ids().begin(),
                          p2.lastResolutionChoice->candidate_object_ids().end(), jace->oid) !=
                p2.lastResolutionChoice->candidate_object_ids().end());
    EXPECT_TRUE(p2.lastResolutionChoice->candidate_player_ids().empty());
    EXPECT_EQ(p2.lastResolutionChoice->prompt_text(),
              "Choose any number of permanents and/or players with counters.");

    ruled::v1::RuledCommand answer;
    answer.mutable_submit_resolution_choice()->add_chosen_object_ids(jace->oid);
    answer.mutable_submit_resolution_choice()->add_chosen_player_ids(p2.myId);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, answer, QStringLiteral("Proliferate E2E choose mixed recipients")));

    const auto updatedJace1 = permanent(p1, p1.myId, QStringLiteral("jace_beleren"));
    const auto updatedJace2 = permanent(p2, p1.myId, QStringLiteral("jace_beleren"));
    ASSERT_TRUE(updatedJace1.has_value() && updatedJace2.has_value());
    EXPECT_EQ(updatedJace1->loyalty, 4);
    EXPECT_EQ(updatedJace2->loyalty, 4);
    ASSERT_TRUE(p1.playerCountersByPlayer.count(p2.myId));
    ASSERT_TRUE(p2.playerCountersByPlayer.count(p2.myId));
    EXPECT_EQ(p1.playerCountersByPlayer.at(p2.myId).at("poison"), 2);
    EXPECT_EQ(p2.playerCountersByPlayer.at(p2.myId).at("poison"), 2);
}
} // namespace ruled_e2e
