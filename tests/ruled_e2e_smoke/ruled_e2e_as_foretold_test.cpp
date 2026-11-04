#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
class AsForetoldDriver final : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    std::map<quint32, ruled::v1::BattlefieldObject> publicObjects;
    bool handSlotMapsRemainPrivate = true;

    void onRuledEvent(const ruled::v1::RuledEvent &event) override
    {
        OpeningDriver::onRuledEvent(event);
        if (event.has_hand_slot_map())
            for (const auto &entry : event.hand_slot_map().entries())
                handSlotMapsRemainPrivate &= entry.player_id() == myId;
        if (event.has_zone_view() && !event.zone_view().battlefields_unchanged()) {
            publicObjects.clear();
            for (const auto &player : event.zone_view().per_player())
                for (const auto &object : player.battlefield_objects())
                    publicObjects[object.object_id()] = object;
        }
    }
};
} // namespace

TEST_F(RuledE2ESmokeTest, ActualAsForetoldKeepsPrivateSourceAndExactPhysicalCast)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << std::string(started.message()).substr(5);
    AsForetoldDriver p1(true, QStringLiteral("asforetoldp1"), &transcript);
    AsForetoldDriver p2(false, QStringLiteral("asforetoldp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    const auto wait = [&](const std::function<bool()> &done, const char *description) -> ::testing::AssertionResult {
        QElapsedTimer timer;
        timer.start();
        while (!done() && timer.elapsed() < 20000) {
            p1.pump(20);
            p2.pump(20);
        }
        if (!done())
            return ::testing::AssertionFailure() << description;
        return ::testing::AssertionSuccess();
    };
    ASSERT_TRUE(wait([&] { return p1.gameStarted && p2.gameStarted; }, "As Foretold startup"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());
    QElapsedTimer opening;
    opening.start();
    while ((p1.phase != ruled::v1::PHASE_ID_MAIN1 || p2.phase != ruled::v1::PHASE_ID_MAIN1 ||
            p1.priorityPlayer != p1.myId || p2.priorityPlayer != p1.myId) &&
           opening.elapsed() < 30000) {
        p1.pump(20);
        p2.pump(20);
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    const int actor = p1.myId;
    const int opponent = p2.myId;
    const auto send = [&](AsForetoldDriver &sender, const ruled::v1::RuledCommand &command, const char *description) {
        const auto before1 = p1.stateVersion;
        const auto before2 = p2.stateVersion;
        sender.sendRuled(command, QString::fromUtf8(description));
        return wait([&] { return p1.stateVersion > before1 && p2.stateVersion > before2; }, description);
    };
    const auto pass = [&](AsForetoldDriver &sender) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(sender, command, "As Foretold priority pass");
    };
    const auto advance = [&](const std::function<bool()> &done) {
        QElapsedTimer timer;
        timer.start();
        while (!done() && timer.elapsed() < 30000) {
            p1.pump(20);
            p2.pump(20);
            if (done())
                break;
            AsForetoldDriver &active = p1.activePlayer == actor ? p1 : p2;
            const auto discards = active.handActions(ruled::v1::HAND_ACTION_CLEANUP_DISCARD);
            if (discards.size() > 7) {
                ruled::v1::RuledCommand command;
                for (int index = 0; index < discards.size() - 7; ++index)
                    command.mutable_discard_to_hand_size()->add_hand_card_indices(discards.at(index)->hand_index());
                if (!send(active, command, "As Foretold cleanup discard"))
                    return false;
            } else if (p1.pendingTriggerOrder || p2.pendingTriggerOrder) {
                auto &chooser = p1.pendingTriggerOrder ? p1 : p2;
                const auto order = *chooser.pendingTriggerOrder;
                chooser.pendingTriggerOrder.reset();
                ruled::v1::RuledCommand command;
                command.mutable_submit_trigger_order()->set_trigger_object_id(order.candidates(0).trigger_object_id());
                if (!send(chooser, command, "As Foretold upkeep trigger order"))
                    return false;
            } else if (p1.phase == ruled::v1::PHASE_ID_CLEANUP && p1.stackDepth == 0) {
                p1.pump(25);
                p2.pump(25);
            } else if (!pass(p1.priorityPlayer == actor ? p1 : p2)) {
                return false;
            }
        }
        return done();
    };
    const auto main = [&](int player) {
        return advance([&] {
            return p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
                   p1.activePlayer == player && p2.activePlayer == player && p1.priorityPlayer == player &&
                   p1.stackDepth == 0 && p2.stackDepth == 0;
        });
    };
    const auto settle = [&] { return advance([&] { return p1.stackDepth == 0 && p2.stackDepth == 0; }); };
    const auto put = [&](int player, const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        command.mutable_dev_command()->set_target_player_id(player);
        auto *placement = command.mutable_dev_command()->mutable_put_card_in_zone();
        placement->set_card_name(name);
        placement->set_zone(zone);
        placement->set_ready(true);
        return send(p1, command, "As Foretold prepared card");
    };

    // Prepared real relay/sidecar proof. The server pilot separately establishes dev-disabled play.
    ASSERT_TRUE(put(actor, "As Foretold", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put(actor, "As Foretold", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(main(opponent));
    ASSERT_TRUE(main(actor)); // Actual own upkeep gives both real enchantments one Time counter.
    ASSERT_TRUE(put(actor, "Lightning Bolt", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(put(actor, "Lightning Bolt", ruled::v1::DEV_ZONE_HAND));
    std::vector<int> boltSlots;
    for (const auto &action : p1.latestLegal.hand_actions())
        if (action.card_name() == "Lightning Bolt" && action.cast_method() == ruled::v1::CAST_METHOD_NORMAL)
            boltSlots.push_back(action.hand_index());
    ASSERT_EQ(boltSlots.size(), 2u);
    const int slot = boltSlots.back();
    const int exactPhysical = p1.handServerCardBySlot.at(slot);
    const int otherPhysical = p1.handServerCardBySlot.at(boltSlots.front());
    const auto *offer = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Lightning Bolt"));
    ASSERT_NE(offer, nullptr);
    ASSERT_EQ(offer->as_foretold_costs_size(), 2);
    const auto selected = offer->as_foretold_costs(1).source();
    const auto remaining = offer->as_foretold_costs(0).source();
    ASSERT_NE(selected.object_id(), remaining.object_id());
    const int sourcePhysical = p1.serverCardByEngineOid.at(selected.object_id());
    for (const auto *client : {&p1, &p2}) {
        // Definition IDs are server-only; both seats receive the same physical source binding.
        EXPECT_TRUE(client->publicObjects.at(selected.object_id()).card_id().empty());
        EXPECT_EQ(client->publicObjects.at(selected.object_id()).zone_change_generation(), selected.zone_change_generation());
    }
    ASSERT_EQ(p1.myPool.total(), 0);
    const auto begin = [&] {
        ruled::v1::RuledCommand command;
        auto *announcement = command.mutable_begin_spell_cast()->mutable_announcement();
        announcement->mutable_source()->set_hand_index(slot);
        announcement->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        *announcement->mutable_as_foretold_source() = selected;
        auto *target = announcement->add_targets();
        target->set_object_id(opponent);
        target->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
        p1.sendRuled(command, QStringLiteral("As Foretold begin exact selected source"));
        return wait([&] { return p1.latestLegal.has_pending_spell_cast(); }, "As Foretold private pending");
    };
    const auto handBefore = p1.handServerCardBySlot;
    ASSERT_TRUE(begin());
    ASSERT_TRUE(p1.latestLegal.pending_spell_cast().announcement().has_as_foretold_source());
    EXPECT_EQ(p1.latestLegal.pending_spell_cast().announcement().as_foretold_source().SerializeAsString(),
              selected.SerializeAsString());
    EXPECT_FALSE(p2.latestLegal.has_pending_spell_cast());
    for (const auto *client : {&p1, &p2})
        EXPECT_FALSE(std::any_of(client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(), [&](const auto &move) {
            return move.card_id() == exactPhysical && move.start_zone() == ZoneNames::HAND;
        }));
    ruled::v1::RuledCommand cancel;
    cancel.mutable_cancel_spell_cast()->set_transaction_id(p1.latestLegal.pending_spell_cast().transaction_id());
    ASSERT_TRUE(send(p1, cancel, "As Foretold cancel"));
    ASSERT_FALSE(p1.latestLegal.has_pending_spell_cast());
    EXPECT_EQ(p1.handServerCardBySlot, handBefore);
    offer = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Lightning Bolt"));
    ASSERT_NE(offer, nullptr);
    ASSERT_EQ(offer->as_foretold_costs_size(), 2);
    ASSERT_TRUE(begin());
    const auto pending = p1.latestLegal.pending_spell_cast();
    const int beforePreview = p1.paymentPreviewCount;
    ruled::v1::RuledCommand preview;
    preview.mutable_preview_payment()->set_transaction_id(pending.transaction_id());
    preview.mutable_preview_payment()->set_revision(p1.stateVersion);
    preview.mutable_preview_payment()->mutable_commit_spell_cast()->set_transaction_id(pending.transaction_id());
    p1.sendRuled(preview, QStringLiteral("As Foretold zero-cost payment preview"));
    ASSERT_TRUE(wait([&] { return p1.paymentPreviewCount > beforePreview; }, "As Foretold payment preview"));
    ASSERT_TRUE(p1.paymentPreview.valid()) << p1.paymentPreview.error();
    ASSERT_TRUE(p1.paymentPreview.complete());
    ruled::v1::RuledCommand commit;
    commit.mutable_commit_spell_cast()->set_transaction_id(pending.transaction_id());
    *commit.mutable_commit_spell_cast()->mutable_payment() = p1.paymentPreview.selection();
    ASSERT_TRUE(send(p1, commit, "As Foretold commit exact physical spell"));
    EXPECT_EQ(p1.myPool.total(), 0);
    EXPECT_EQ(p1.stackDepth, 1);
    EXPECT_EQ(p2.stackDepth, 1);
    for (const auto *client : {&p1, &p2}) {
        EXPECT_EQ(client->serverCardByEngineOid.at(pending.reserved_object_id()), exactPhysical);
        EXPECT_EQ(client->serverCardByEngineOid.at(selected.object_id()), sourcePhysical);
        EXPECT_EQ(std::count_if(client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(), [&](const auto &move) {
            return move.card_id() == exactPhysical && move.start_zone() == ZoneNames::HAND && move.target_zone() == ZoneNames::STACK;
        }), 1);
        EXPECT_FALSE(std::any_of(client->physicalMoveEvents.begin(), client->physicalMoveEvents.end(), [&](const auto &move) {
            return move.card_id() == otherPhysical && move.start_zone() == ZoneNames::HAND;
        }));
        EXPECT_TRUE(client->handSlotMapsRemainPrivate);
    }
    ASSERT_TRUE(settle());
    offer = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Lightning Bolt"));
    ASSERT_NE(offer, nullptr);
    ASSERT_EQ(offer->as_foretold_costs_size(), 1);
    EXPECT_EQ(offer->as_foretold_costs(0).source().SerializeAsString(), remaining.SerializeAsString());
    EXPECT_FALSE(p2.latestLegal.has_pending_spell_cast());
    for (const auto &action : p2.latestLegal.hand_actions())
        EXPECT_EQ(action.as_foretold_costs_size(), 0);
}
} // namespace ruled_e2e
