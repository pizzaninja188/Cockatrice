#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
class PreparationDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    std::map<quint32, ruled::v1::PrepareSpellCopyView> copies;
    std::map<quint32, quint32> prepared;
    std::map<int, std::map<int, QString>> physicalExile;
    std::map<quint32, ruled::v1::StackPushed> pushed;

    void onRuledEvent(const ruled::v1::RuledEvent &event) override
    {
        OpeningDriver::onRuledEvent(event);
        if (event.has_zone_view()) {
            copies.clear();
            if (!event.zone_view().battlefields_unchanged()) {
                prepared.clear();
            }
            for (const auto &player : event.zone_view().per_player()) {
                for (const auto &copy : player.prepare_spell_copies()) {
                    copies[copy.object_id()] = copy;
                }
                for (const auto &object : player.battlefield_objects()) {
                    if (object.has_preparation()) {
                        prepared[object.object_id()] = object.preparation().copy_object_id();
                    }
                }
            }
        }
        if (event.has_stack_pushed()) {
            pushed[event.stack_pushed().object_id()] = event.stack_pushed();
        }
    }
    void onPhysicalEvent(const GameEvent &event) override
    {
        if (!event.HasExtension(Event_GameStateChanged::ext)) {
            return;
        }
        for (const auto &player : event.GetExtension(Event_GameStateChanged::ext).player_list()) {
            for (const auto &zone : player.zone_list()) {
                if (zone.name() != ZoneNames::EXILE) {
                    continue;
                }
                auto &cards = physicalExile[player.properties().player_id()];
                cards.clear();
                for (const auto &card : zone.card_list()) {
                    cards[card.id()] = QString::fromStdString(card.name());
                }
            }
        }
    }
};
}

TEST_F(RuledE2ESmokeTest, PreparationCopiesReachBothRecipientsAndNeverMoveTheSource)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    PreparationDriver p1(true, QStringLiteral("preparep1"), &transcript);
    PreparationDriver p2(false, QStringLiteral("preparep2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "preparation start p1"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "preparation start p2"));
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
    auto send = [&](PreparationDriver &sender, const ruled::v1::RuledCommand &command) {
        const auto before1 = p1.stateVersion;
        const auto before2 = p2.stateVersion;
        sender.sendRuled(command, QStringLiteral("preparation scenario"));
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto pass = [&](PreparationDriver &sender) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(sender, command);
    };
    auto put = [&](PreparationDriver &sender, const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(sender.myId);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        dev->mutable_put_card_in_zone()->set_zone(zone);
        dev->mutable_put_card_in_zone()->set_ready(true);
        return send(sender, command);
    };
    for (const char *name : {"Infirmary Healer // Stream of Life", "Elite Interceptor // Rejoinder",
                            "Quill-Blade Laureate // Twofold Intent", "Pigment Wrangler // Striking Palette"}) {
        ASSERT_TRUE(put(p1, name, ruled::v1::DEV_ZONE_BATTLEFIELD));
    }
    ASSERT_EQ(p1.copies.size(), 4u);
    ASSERT_EQ(p2.copies.size(), 4u);
    ASSERT_EQ(p1.physicalExile[p1.myId].size(), 4u);
    EXPECT_EQ(p1.physicalExile[p1.myId], p2.physicalExile[p1.myId]);
    EXPECT_TRUE(p2.latestLegal.zone_cast_actions().empty());
    quint32 source = 0;
    for (const auto &[oid, copy] : p1.copies) {
        ASSERT_NE(p2.copies.find(oid), p2.copies.end());
        EXPECT_EQ(copy.SerializeAsString(), p2.copies.at(oid).SerializeAsString());
        const int physicalCopy = p1.serverCardByEngineOid.at(oid);
        EXPECT_EQ(p1.physicalExile[p1.myId].at(physicalCopy), QString::fromStdString(copy.display_name()));
        EXPECT_NE(physicalCopy, p1.serverCardByEngineOid.at(copy.source().object_id()));
        if (copy.display_name() == "Stream of Life") {
            source = copy.source().object_id();
        }
    }
    ASSERT_NE(source, 0u);
    const int sourcePhysicalId = p1.serverCardByEngineOid.at(source);
    const quint32 copy = p1.prepared.at(source);
    ruled::v1::LegalZoneCastAction offer;
    for (const auto &action : p1.latestLegal.zone_cast_actions()) {
        if (action.object_id() == copy) {
            offer = action;
        }
    }
    ASSERT_EQ(offer.preparation_source().object_id(), source);
    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_g(4);
    ASSERT_TRUE(send(p1, mana));
    ruled::v1::RuledCommand cast;
    auto *spell = cast.mutable_cast_spell();
    spell->mutable_source()->set_exile_object_id(copy);
    spell->mutable_source()->set_expected_zone_change_generation(offer.zone_change_generation());
    spell->set_casting_permission_id(offer.casting_permission_id());
    spell->set_face_index(offer.face_index());
    spell->set_cast_method(offer.cast_method());
    spell->set_x_value(3);
    spell->add_targets()->set_object_id(p2.myId);
    ASSERT_TRUE(send(p1, cast));
    EXPECT_EQ(p1.prepared.count(source), 0u);
    EXPECT_EQ(p2.prepared.count(source), 0u);
    EXPECT_EQ(p1.copies.count(copy), 0u);
    EXPECT_EQ(p1.physicalExile[p1.myId].size(), 3u);
    EXPECT_EQ(p1.physicalExile[p1.myId], p2.physicalExile[p1.myId]);
    ASSERT_NE(p1.pushed.find(copy), p1.pushed.end());
    EXPECT_TRUE(p1.pushed.at(copy).is_copy());
    EXPECT_TRUE(p1.pushed.at(copy).is_prepare_spell());
    EXPECT_EQ(p1.pushed.at(copy).description(), "Stream of Life");
    EXPECT_EQ(p1.pushed.at(copy).SerializeAsString(), p2.pushed.at(copy).SerializeAsString());
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    EXPECT_EQ(p1.lifeByPlayer[p2.myId], 23);
    EXPECT_EQ(p2.lifeByPlayer[p2.myId], 23);
    EXPECT_EQ(p1.serverCardByEngineOid.at(source), sourcePhysicalId);
    EXPECT_EQ(p2.serverCardByEngineOid.at(source), sourcePhysicalId);
    EXPECT_TRUE(p1.physicalRowAndPt.count({p1.myId, sourcePhysicalId}));
    for (const auto &move : p1.physicalMoveEvents) {
        EXPECT_FALSE(move.start_zone() == ZoneNames::TABLE && move.card_id() == sourcePhysicalId);
        EXPECT_FALSE(move.start_zone() == ZoneNames::EXILE && move.target_zone() == ZoneNames::STACK);
    }
}
} // namespace ruled_e2e
