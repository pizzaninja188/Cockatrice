#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
class CopyDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    std::map<quint32, ruled::v1::BattlefieldObject> objects;
    std::map<int, ServerInfo_Card> physicalTokens;
    void onRuledEvent(const ruled::v1::RuledEvent &event) override
    {
        OpeningDriver::onRuledEvent(event);
        if (event.has_zone_view() && !event.zone_view().battlefields_unchanged()) {
            objects.clear();
            for (const auto &player : event.zone_view().per_player()) {
                for (const auto &object : player.battlefield_objects()) {
                    objects[object.object_id()] = object;
                }
            }
        }
    }
    void onPhysicalEvent(const GameEvent &event) override
    {
        if (event.HasExtension(Event_GameStateChanged::ext)) {
            physicalTokens.clear();
            for (const auto &player : event.GetExtension(Event_GameStateChanged::ext).player_list()) {
                for (const auto &zone : player.zone_list()) {
                    if (zone.name() == ZoneNames::TABLE) {
                        for (const auto &card : zone.card_list()) {
                            if (card.destroy_on_zone_change()) {
                                physicalTokens[card.id()] = card;
                            }
                        }
                    }
                }
            }
        }
    }
};
} // namespace

TEST_F(RuledE2ESmokeTest, SourceCopiesAndDoubleFacedTokensReachBothRecipients)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    CopyDriver p1(true, QStringLiteral("copyp1"), &transcript);
    CopyDriver p2(false, QStringLiteral("copyp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "token copy start p1"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "token copy start p2"));
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
    auto send = [&](CopyDriver &sender, const ruled::v1::RuledCommand &command) {
        const auto before1 = p1.stateVersion;
        const auto before2 = p2.stateVersion;
        sender.sendRuled(command, QStringLiteral("token copy scenario"));
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto pass = [&](CopyDriver &sender) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(sender, command);
    };
    auto put = [&](CopyDriver &sender, const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(sender.myId);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        dev->mutable_put_card_in_zone()->set_zone(zone);
        dev->mutable_put_card_in_zone()->set_ready(true);
        return send(sender, command);
    };

    auto mana = [&](int amount) {
        ruled::v1::RuledCommand command;
        command.mutable_dev_command()->set_target_player_id(p1.myId);
        command.mutable_dev_command()->mutable_add_mana()->set_u(amount);
        return send(p1, command);
    };
    auto cast = [&](const QString &name, quint32 target = 0) {
        const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, name);
        if (!action) {
            return false;
        }
        ruled::v1::RuledCommand command;
        command.mutable_cast_spell()->mutable_source()->set_hand_index(action->hand_index());
        command.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        if (target) {
            const auto found = p1.latestLegal.valid_targets_by_hand_slot().find(action->hand_index() << 8);
            bool legal = false;
            if (found != p1.latestLegal.valid_targets_by_hand_slot().end()) {
                for (const auto &group : found->second.groups()) {
                    for (auto oid : group.valid_permanent_ids()) {
                        legal |= oid == target;
                    }
                }
            }
            if (!legal) {
                return false;
            }
            auto *chosen = command.mutable_cast_spell()->add_targets();
            chosen->set_object_id(target);
            chosen->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
        }
        return send(p1, command);
    };
    ASSERT_TRUE(put(p1, "Colorstorm Stallion", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put(p1, "Tidings", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(mana(5));
    const auto beforeStallion = p1.physicalCreateTokenEvents.size();
    ASSERT_TRUE(cast(QStringLiteral("Tidings")));
    ASSERT_EQ(p1.stackDepth, 2);
    EXPECT_FALSE(p1.pendingTriggerTarget.has_value());
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    ASSERT_EQ(p1.stackDepth, 1);
    quint32 stallionToken = 0;
    for (const auto &[oid, object] : p1.objects) {
        if (object.has_token_identity()) {
            stallionToken = oid;
            EXPECT_EQ(object.token_identity().name(), "Colorstorm Stallion");
        }
    }
    ASSERT_NE(stallionToken, 0u);
    EXPECT_EQ(p1.objects.at(stallionToken).SerializeAsString(), p2.objects.at(stallionToken).SerializeAsString());
    EXPECT_EQ(p1.serverCardByEngineOid.at(stallionToken), p2.serverCardByEngineOid.at(stallionToken));
    ASSERT_EQ(p1.physicalCreateTokenEvents.size(), beforeStallion + 1);
    ASSERT_EQ(p2.physicalCreateTokenEvents.size(), beforeStallion + 1);
    for (const auto &permanent : p1.battlefieldByPlayer[p1.myId]) {
        EXPECT_EQ(permanent.power, permanent.oid == stallionToken ? 3 : 4);
        EXPECT_TRUE(permanent.haste);
    }
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    auto remove = [&](const char *name) {
        ruled::v1::RuledCommand command;
        command.mutable_dev_command()->set_target_player_id(p1.myId);
        auto *move = command.mutable_dev_command()->mutable_move_card();
        move->set_card_name(name);
        move->set_zone(ruled::v1::DEV_ZONE_GRAVEYARD);
        return send(p1, command);
    };
    ASSERT_TRUE(remove("Colorstorm Stallion"));
    ASSERT_TRUE(remove("Colorstorm Stallion"));
    ASSERT_TRUE(put(p1, "Reckless Waif // Merciless Predator", ruled::v1::DEV_ZONE_BATTLEFIELD));
    quint32 waif = p1.objects.begin()->first;
    ASSERT_TRUE(put(p1, "Cackling Counterpart", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(mana(3));
    const auto beforeWaif = p1.physicalCreateTokenEvents.size();
    ASSERT_TRUE(cast(QStringLiteral("Cackling Counterpart"), waif));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    quint32 copy = 0;
    for (const auto &[oid, object] : p1.objects) {
        if (object.has_token_identity()) {
            copy = oid;
        }
    }
    ASSERT_NE(copy, 0u);
    const int physical = p1.serverCardByEngineOid.at(copy);
    EXPECT_EQ(p2.serverCardByEngineOid.at(copy), physical);
    EXPECT_EQ(p1.objects.at(copy).face_up_index(), 0u);
    ASSERT_TRUE(remove("Reckless Waif // Merciless Predator"));
    ASSERT_TRUE(p1.objects.count(copy));
    // Advance actual turns: after a spell-free turn, the copied upkeep ability transforms it.
    for (int step = 0; step < 100 && p1.objects.at(copy).face_up_index() == 0; ++step) {
        CopyDriver *sender = p1.priorityPlayer == p1.myId ? &p1 : &p2;
        ruled::v1::RuledCommand command;
        if (p1.pendingTriggerOrder || p2.pendingTriggerOrder) {
            sender = p1.pendingTriggerOrder ? &p1 : &p2;
            command.mutable_submit_trigger_order()->set_trigger_object_id(
                sender->pendingTriggerOrder->candidates(0).trigger_object_id());
            sender->pendingTriggerOrder.reset();
        } else {
            const auto discard1 = p1.handActions(ruled::v1::HAND_ACTION_CLEANUP_DISCARD);
            const auto discard2 = p2.handActions(ruled::v1::HAND_ACTION_CLEANUP_DISCARD);
            if (!discard1.empty() || !discard2.empty()) {
                sender = !discard1.empty() ? &p1 : &p2;
                const auto &actions = !discard1.empty() ? discard1 : discard2;
                for (int i = 0; i < actions.size() - 7; ++i) {
                    command.mutable_discard_to_hand_size()->add_hand_card_indices(actions[i]->hand_index());
                }
            } else {
                command.mutable_pass_priority();
            }
        }
        ASSERT_TRUE(send(*sender, command)) << "turn advance " << step;
    }
    ASSERT_EQ(p1.objects.at(copy).face_up_index(), 1u);
    EXPECT_EQ(p1.objects.at(copy).SerializeAsString(), p2.objects.at(copy).SerializeAsString());
    EXPECT_EQ(p1.serverCardByEngineOid.at(copy), physical);
    EXPECT_EQ(p2.serverCardByEngineOid.at(copy), physical);
    ASSERT_TRUE(p1.physicalTokens.count(physical));
    ASSERT_TRUE(p2.physicalTokens.count(physical));
    EXPECT_EQ(p1.physicalTokens.at(physical).name(), "Merciless Predator");
    EXPECT_EQ(p1.physicalTokens.at(physical).pt(), "3/2");
    EXPECT_EQ(p1.physicalTokens.at(physical).SerializeAsString(), p2.physicalTokens.at(physical).SerializeAsString());
    EXPECT_EQ(p1.physicalCreateTokenEvents.size(), beforeWaif + 1);
    ASSERT_TRUE(remove("Reckless Waif // Merciless Predator"));
    EXPECT_FALSE(p1.objects.count(copy));
    EXPECT_FALSE(p2.objects.count(copy));
}
} // namespace ruled_e2e
