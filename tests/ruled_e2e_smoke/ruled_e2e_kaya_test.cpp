#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
// Prepared actual-card fixture through a real relay/sidecar and two independent clients.
// Strict dev-disabled legal-session reachability is covered by the server capture pilots.
TEST_F(RuledE2ESmokeTest, KayaEmblemMixedZonesKeepPrivateHandAndExactDuplicatePhysicalCards)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << std::string(started.message()).substr(5);
    OpeningDriver p1(true, QStringLiteral("kayap1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("kayap2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "Kaya start P1"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "Kaya start P2"));
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
    auto send = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const auto before1 = p1.stateVersion;
        const auto before2 = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto put = [&](const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        const bool relocate = zone == ruled::v1::DEV_ZONE_GRAVEYARD || zone == ruled::v1::DEV_ZONE_EXILE;
        dev->mutable_put_card_in_zone()->set_zone(relocate ? ruled::v1::DEV_ZONE_HAND : zone);
        dev->mutable_put_card_in_zone()->set_ready(true);
        if (!send(p1, command, QStringLiteral("Kaya prepared put %1").arg(name)))
            return false;
        if (!relocate)
            return true;
        ruled::v1::RuledCommand move;
        move.mutable_dev_command()->set_target_player_id(p1.myId);
        move.mutable_dev_command()->mutable_move_card()->set_card_name(name);
        move.mutable_dev_command()->mutable_move_card()->set_zone(zone);
        return send(p1, move, QStringLiteral("Kaya prepared relocate %1").arg(name));
    };
    auto pass = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("Kaya pass"));
    };
    ASSERT_TRUE(put("Doubling Season", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put("Kaya the Inexorable", ruled::v1::DEV_ZONE_BATTLEFIELD));
    const auto &board = p1.battlefieldByPlayer[p1.myId];
    const auto source = std::find_if(board.begin(), board.end(),
                                     [](const auto &p) { return p.cardId == QStringLiteral("kaya_the_inexorable"); });
    ASSERT_NE(source, board.end());
    ASSERT_EQ(source->loyalty, 10);
    ruled::v1::RuledCommand ultimate;
    p1.setBattlefieldAbilitySource(ultimate.mutable_activate_ability(), source->oid);
    ultimate.mutable_activate_ability()->set_ability_index(2);
    ASSERT_TRUE(send(p1, ultimate, QStringLiteral("Kaya prepared -7")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    for (const auto *client : {&p1, &p2}) {
        EXPECT_TRUE(std::any_of(client->physicalCreateTokenEvents.begin(), client->physicalCreateTokenEvents.end(),
                                [](const Event_CreateToken &token) {
                                    return token.card_name() == "Kaya the Inexorable Emblem" &&
                                           token.annotation() == "Emblem";
                                }));
    }
    auto advanceToOffer = [&] {
        for (int step = 0; step < 160; ++step) {
            p1.pump(10);
            p2.pump(10);
            if (p1.resolutionPromptActive && p1.pendingChoice &&
                p1.pendingChoice->choice_kind() == ruled::v1::CHOICE_KIND_SPECIAL_CAST &&
                p1.pendingChoice->resolution_prompt_id() == p1.currentResolutionPromptId)
                return true;
            bool discarded = false;
            for (auto *client : {&p1, &p2}) {
                std::vector<quint32> indices;
                for (const auto &a : client->latestLegal.hand_actions())
                    if (a.kind() == ruled::v1::HAND_ACTION_CLEANUP_DISCARD &&
                        (a.card_name() == "Island" || a.card_name() == "Forest"))
                        indices.push_back(a.hand_index());
                if (!indices.empty()) {
                    const int excess = client->handSizeByPlayer[client->myId] - 7;
                    if (excess <= 0 || indices.size() < static_cast<size_t>(excess))
                        return false;
                    ruled::v1::RuledCommand command;
                    for (int i = 0; i < excess; ++i)
                        command.mutable_discard_to_hand_size()->add_hand_card_indices(indices[i]);
                    if (!send(*client, command, QStringLiteral("Kaya prepared cleanup")))
                        return false;
                    discarded = true;
                    break;
                }
            }
            if (discarded)
                continue;
            if (p1.priorityPlayer == p1.myId) {
                if (!pass(p1))
                    return false;
            } else if (p1.priorityPlayer == p2.myId) {
                if (!pass(p2))
                    return false;
            } else
                return false;
        }
        return false;
    };
    // Empty eligibility follows the same private waiting path, with an explicit decline.
    ASSERT_TRUE(advanceToOffer());
    ASSERT_TRUE(p1.pendingChoice);
    EXPECT_EQ(p1.pendingChoice->candidate_object_ids_size(), 0);
    EXPECT_EQ(p1.pendingChoice->min(), 0);
    EXPECT_EQ(p1.pendingChoice->max(), 1);
    ASSERT_TRUE(p2.lastResolutionChoice);
    const auto emptyObserverPrompt = *p2.lastResolutionChoice;
    EXPECT_EQ(emptyObserverPrompt.candidate_object_ids_size(), 0);
    ruled::v1::RuledCommand decline;
    decline.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_DECLINE);
    decline.mutable_submit_resolution_choice()->set_resolution_prompt_id(p1.currentResolutionPromptId);
    ASSERT_TRUE(send(p1, decline, QStringLiteral("Kaya explicitly declines empty offer")));
    for (int i = 0; i < 2; ++i) {
        ASSERT_TRUE(put("The Chain Veil", ruled::v1::DEV_ZONE_HAND));
        ASSERT_TRUE(put("Teferi, Temporal Archmage", ruled::v1::DEV_ZONE_GRAVEYARD));
        ASSERT_TRUE(put("Tamiyo, Field Researcher", ruled::v1::DEV_ZONE_EXILE));
    }
    for (const auto zone :
         {ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND, ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_GRAVEYARD,
          ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_EXILE}) {
        ASSERT_TRUE(advanceToOffer());
        ASSERT_TRUE(p1.pendingChoice);
        const auto offer = *p1.pendingChoice;
        ASSERT_EQ(offer.candidate_source_zones_size(), offer.candidate_object_ids_size());
        ASSERT_EQ(offer.candidate_server_card_ids_size(), offer.candidate_object_ids_size());
        ASSERT_TRUE(p2.lastResolutionChoice);
        EXPECT_EQ(p2.lastResolutionChoice->candidate_object_ids_size(), 0);
        EXPECT_EQ(p2.lastResolutionChoice->candidate_names_size(), 0);
        EXPECT_EQ(p2.lastResolutionChoice->candidate_server_card_ids_size(), 0);
        EXPECT_EQ(p2.lastResolutionChoice->candidate_source_zones_size(), 0);
        EXPECT_EQ(p2.lastResolutionChoice->choice_kind(), emptyObserverPrompt.choice_kind());
        EXPECT_EQ(p2.lastResolutionChoice->min(), emptyObserverPrompt.min());
        EXPECT_EQ(p2.lastResolutionChoice->max(), emptyObserverPrompt.max());
        EXPECT_EQ(p2.lastResolutionChoice->prompt_text(), emptyObserverPrompt.prompt_text());
        std::vector<int> candidates;
        for (int i = 0; i < offer.candidate_object_ids_size(); ++i)
            if (offer.candidate_source_zones(i) == zone)
                candidates.push_back(i);
        ASSERT_EQ(candidates.size(), 2u);
        const int selected = candidates.back();
        const auto oid = offer.candidate_object_ids(selected);
        const int exactPhysical = offer.candidate_server_card_ids(selected);
        const int otherPhysical = offer.candidate_server_card_ids(candidates.front());
        ASSERT_GE(exactPhysical, 0);
        EXPECT_NE(exactPhysical, otherPhysical);
        const auto physicalStart = p1.physicalMoveEvents.size();
        auto begin = [&] {
            ruled::v1::RuledCommand command;
            auto *choice = command.mutable_submit_resolution_choice();
            choice->set_resolution_prompt_id(p1.currentResolutionPromptId);
            choice->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_CAST_SPELL);
            auto *announcement = choice->mutable_spell_cast_announcement();
            announcement->set_cast_method(ruled::v1::CAST_METHOD_WITHOUT_MANA_COST);
            if (zone == ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND) {
                const auto slot = std::find_if(p1.handServerCardBySlot.begin(), p1.handServerCardBySlot.end(),
                                               [&](const auto &entry) { return entry.second == exactPhysical; });
                if (slot == p1.handServerCardBySlot.end())
                    return false;
                announcement->mutable_source()->set_hand_index(slot->first);
            } else {
                const auto action = std::find_if(p1.latestLegal.zone_cast_actions().begin(),
                                                 p1.latestLegal.zone_cast_actions().end(), [&](const auto &a) {
                                                     return a.object_id() == oid &&
                                                            a.cast_method() == ruled::v1::CAST_METHOD_WITHOUT_MANA_COST;
                                                 });
                if (action == p1.latestLegal.zone_cast_actions().end())
                    return false;
                announcement->mutable_source()->set_expected_zone_change_generation(action->zone_change_generation());
                if (zone == ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_GRAVEYARD)
                    announcement->mutable_source()->set_graveyard_object_id(oid);
                else
                    announcement->mutable_source()->set_exile_object_id(oid);
            }
            p1.sendRuled(command, QStringLiteral("Kaya begin mixed-zone cast"));
            return static_cast<bool>(p1.pumpUntil(
                [&] {
                    p2.pump(25);
                    return p1.latestLegal.has_pending_spell_cast();
                },
                10000, "Kaya pending cast"));
        };
        const auto firstPrompt = p1.currentResolutionPromptId;
        ASSERT_TRUE(begin());
        EXPECT_FALSE(p2.latestLegal.has_pending_spell_cast());
        ASSERT_EQ(p1.latestLegal.pending_spell_cast().reserved_object_id(), oid);
        ruled::v1::RuledCommand cancel;
        cancel.mutable_cancel_spell_cast()->set_transaction_id(p1.latestLegal.pending_spell_cast().transaction_id());
        p1.sendRuled(cancel, QStringLiteral("Kaya cancel exact mixed-zone source"));
        ASSERT_TRUE(p1.pumpUntil(
            [&] {
                p2.pump(25);
                return !p1.latestLegal.has_pending_spell_cast() && p1.resolutionPromptActive;
            },
            10000, "Kaya cancellation"));
        EXPECT_NE(p1.currentResolutionPromptId, firstPrompt);
        ASSERT_TRUE(begin());
        const auto pending = p1.latestLegal.pending_spell_cast();
        EXPECT_FALSE(p2.latestLegal.has_pending_spell_cast());
        const int beforePreview = p1.paymentPreviewCount;
        ruled::v1::RuledCommand preview;
        auto *query = preview.mutable_preview_payment();
        query->set_transaction_id(pending.transaction_id());
        query->set_revision(p1.stateVersion);
        query->mutable_commit_spell_cast()->set_transaction_id(pending.transaction_id());
        p1.sendRuled(preview, QStringLiteral("Kaya zero-base payment preview"));
        ASSERT_TRUE(p1.pumpUntil(
            [&] {
                p2.pump(25);
                return p1.paymentPreviewCount > beforePreview;
            },
            10000, "Kaya payment preview"));
        ASSERT_TRUE(p1.paymentPreview.valid()) << p1.paymentPreview.error();
        ASSERT_TRUE(p1.paymentPreview.complete());
        ruled::v1::RuledCommand commit;
        commit.mutable_commit_spell_cast()->set_transaction_id(pending.transaction_id());
        *commit.mutable_commit_spell_cast()->mutable_payment() = p1.paymentPreview.selection();
        ASSERT_TRUE(send(p1, commit, QStringLiteral("Kaya commit exact mixed-zone cast")));
        EXPECT_EQ(p1.stackDepth, 1);
        EXPECT_EQ(p2.stackDepth, 1);
        EXPECT_EQ(p1.myPool.total(), 0);
        for (auto *client : {&p1, &p2}) {
            ASSERT_NE(client->serverCardByEngineOid.find(oid), client->serverCardByEngineOid.end());
            EXPECT_EQ(client->serverCardByEngineOid.at(oid), exactPhysical);
        }
        const auto sourceZone = zone == ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_HAND        ? ZoneNames::HAND
                                : zone == ruled::v1::CHOICE_CANDIDATE_SOURCE_ZONE_GRAVEYARD ? ZoneNames::GRAVE
                                                                                            : ZoneNames::EXILE;
        EXPECT_TRUE(std::none_of(p1.physicalMoveEvents.begin() + physicalStart, p1.physicalMoveEvents.end(),
                                 [&](const auto &move) {
                                     return move.card_id() == otherPhysical && move.start_zone() == sourceZone &&
                                            move.target_zone() == ZoneNames::STACK;
                                 }));
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        EXPECT_EQ(p1.stackDepth, 0);
        EXPECT_EQ(p2.stackDepth, 0);
    }
}
} // namespace
} // namespace ruled_e2e
