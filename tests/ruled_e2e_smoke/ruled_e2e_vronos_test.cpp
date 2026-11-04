#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"

namespace ruled_e2e
{
namespace
{
class VronosDriver final : public OpeningDriver
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

TEST_F(RuledE2ESmokeTest, ActualVronosPhasesReturnsAndAnimatesAcrossTwoRecipients)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0)
        GTEST_SKIP() << std::string(started.message()).substr(5);
    VronosDriver p1(true, QStringLiteral("vronosp1"), &transcript);
    VronosDriver p2(false, QStringLiteral("vronosp2"), &transcript);
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
    ASSERT_TRUE(wait([&] { return p1.gameStarted && p2.gameStarted; }, "Vronos startup"));
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
    const auto send = [&](VronosDriver &sender, const ruled::v1::RuledCommand &command, const char *description) {
        const auto before1 = p1.stateVersion;
        const auto before2 = p2.stateVersion;
        sender.sendRuled(command, QString::fromUtf8(description));
        return wait([&] { return p1.stateVersion > before1 && p2.stateVersion > before2; }, description);
    };
    const auto pass = [&](VronosDriver &sender) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(sender, command, "Vronos priority pass");
    };
    const auto advance = [&](const std::function<bool()> &done) {
        QElapsedTimer timer;
        timer.start();
        while (!done() && timer.elapsed() < 30000) {
            p1.pump(20);
            p2.pump(20);
            if (done())
                break;
            VronosDriver &active = p1.activePlayer == actor ? p1 : p2;
            const auto discards = active.handActions(ruled::v1::HAND_ACTION_CLEANUP_DISCARD);
            if (discards.size() > 7) {
                ruled::v1::RuledCommand command;
                for (int index = 0; index < discards.size() - 7; ++index)
                    command.mutable_discard_to_hand_size()->add_hand_card_indices(discards.at(index)->hand_index());
                if (!send(active, command, "Vronos cleanup discard"))
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
        return send(p1, command, "Vronos prepared card");
    };
    const auto find = [&](int player, const QString &card) -> quint32 {
        for (const auto &object : p1.battlefieldByPlayer[player])
            if (object.cardId == card)
                return object.oid;
        return 0;
    };
    // Prepared real-sidecar transport proof; the strict normal server pilot proves legal reachability.
    ASSERT_TRUE(put(actor, "Jace Beleren", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put(actor, "Ornithopter", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put(opponent, "Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_TRUE(put(actor, "Vronos, Masked Inquisitor", ruled::v1::DEV_ZONE_HAND));
    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(actor);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(3);
    mana.mutable_dev_command()->mutable_add_mana()->set_u(2);
    ASSERT_TRUE(send(p1, mana, "Vronos prepared cast mana"));
    const auto *offer = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Vronos, Masked Inquisitor"));
    ASSERT_NE(offer, nullptr);
    ruled::v1::RuledCommand cast;
    cast.mutable_cast_spell()->mutable_source()->set_hand_index(offer->hand_index());
    cast.mutable_cast_spell()->set_cast_method(offer->cast_method());
    ASSERT_TRUE(send(p1, cast, "Vronos actual cast"));
    ASSERT_TRUE(wait([&] { return !p1.latestLegal.has_pending_spell_cast() && p1.stackDepth > 0; },
                     "Vronos actual cast committed"));
    ASSERT_TRUE(settle());
    const quint32 source = find(actor, QStringLiteral("vronos,_masked_inquisitor"));
    const quint32 jace = find(actor, QStringLiteral("jace_beleren"));
    const quint32 artifact = find(actor, QStringLiteral("ornithopter"));
    const quint32 bear = find(opponent, QStringLiteral("grizzly_bears"));
    ASSERT_NE(source, 0u);
    ASSERT_NE(jace, 0u);
    ASSERT_NE(artifact, 0u);
    ASSERT_NE(bear, 0u);
    ASSERT_EQ(p1.publicObjects.at(source).loyalty(), 5u);
    const auto jaceGeneration = p1.publicObjects.at(jace).zone_change_generation();
    const int jacePhysical = p1.serverCardByEngineOid.at(jace);
    const int artifactPhysical = p1.serverCardByEngineOid.at(artifact);
    const auto artifactGeneration = p1.publicObjects.at(artifact).zone_change_generation();
    const auto activate = [&](quint32 index, std::optional<quint32> object) -> ::testing::AssertionResult {
        const auto &abilities = p1.publicObjects.at(source).activated_abilities();
        const auto ability = std::find_if(abilities.begin(), abilities.end(), [index](const auto &candidate) {
            return candidate.ability_index() == index && candidate.activatable();
        });
        if (ability == abilities.end())
            return ::testing::AssertionFailure() << "Vronos ability was not offered";
        ruled::v1::RuledCommand command;
        p1.setBattlefieldAbilitySource(command.mutable_activate_ability(), source);
        command.mutable_activate_ability()->set_ability_index(ability->ability_index());
        if (object) {
            auto *target = command.mutable_activate_ability()->add_targets();
            target->set_object_id(*object);
            target->set_group_index(0);
            target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
            target->set_expected_zone_change_generation(p1.publicObjects.at(*object).zone_change_generation());
        }
        if (!send(p1, command, "Vronos actual loyalty ability") || !settle())
            return ::testing::AssertionFailure() << "Vronos ability did not settle";
        return ::testing::AssertionSuccess();
    };
    ASSERT_TRUE(activate(0, jace));
    EXPECT_FALSE(p1.publicObjects.at(jace).phased_out());
    ASSERT_TRUE(main(opponent));
    for (const auto *recipient : {&p1, &p2}) {
        EXPECT_TRUE(recipient->publicObjects.at(jace).phased_out());
        EXPECT_EQ(recipient->publicObjects.at(jace).zone_change_generation(), jaceGeneration);
        EXPECT_EQ(recipient->serverCardByEngineOid.at(jace), jacePhysical);
    }
    ASSERT_TRUE(main(actor));
    for (const auto *recipient : {&p1, &p2}) {
        EXPECT_FALSE(recipient->publicObjects.at(jace).phased_out());
        EXPECT_EQ(recipient->publicObjects.at(jace).zone_change_generation(), jaceGeneration);
        EXPECT_EQ(recipient->serverCardByEngineOid.at(jace), jacePhysical);
    }
    const quint64 abilityKey = (static_cast<quint64>(source) << 32) | 1u;
    const auto targets = p1.latestLegal.valid_targets_by_ability().find(abilityKey);
    ASSERT_NE(targets, p1.latestLegal.valid_targets_by_ability().end());
    ASSERT_EQ(targets->second.groups_size(), 1);
    const auto &group = targets->second.groups(0);
    EXPECT_EQ(group.min(), 0u);
    EXPECT_EQ(group.max(), 1u);
    ASSERT_EQ(group.valid_permanent_ids_size(), 1);
    EXPECT_EQ(group.valid_permanent_ids(0), bear);
    EXPECT_EQ(p2.latestLegal.valid_targets_by_ability().count(abilityKey), 0u);
    const int bearPhysical = p2.serverCardByEngineOid.at(bear);
    const auto beforeMoves = p2.physicalMoveEvents.size();
    const auto beforeHand = p2.handServerCardBySlot.size();
    ASSERT_TRUE(activate(1, bear));
    for (const auto *recipient : {&p1, &p2})
        EXPECT_EQ(recipient->publicObjects.count(bear), 0u);
    // Hand-name lists in the shared observer are full-snapshot-only. Assert the actual
    // incremental owner move and fresh private hand-slot mapping for this return instead.
    const auto returned =
        std::find_if(p2.physicalMoveEvents.begin() + beforeMoves, p2.physicalMoveEvents.end(), [&](const auto &move) {
            return move.start_player_id() == opponent && move.card_id() == bearPhysical &&
                   move.start_zone() == ZoneNames::TABLE && move.target_zone() == ZoneNames::HAND;
        });
    ASSERT_NE(returned, p2.physicalMoveEvents.end());
    EXPECT_EQ(returned->target_player_id(), opponent);
    EXPECT_EQ(returned->card_name(), "Grizzly Bears");
    EXPECT_EQ(p2.handServerCardBySlot.size(), beforeHand + 1);
    EXPECT_TRUE(std::any_of(p2.handServerCardBySlot.begin(), p2.handServerCardBySlot.end(),
                            [&](const auto &entry) { return entry.second == returned->new_card_id(); }));
    EXPECT_TRUE(p1.handSlotMapsRemainPrivate);
    EXPECT_TRUE(p2.handSlotMapsRemainPrivate);
    EXPECT_EQ(p1.publicObjects.at(source).loyalty(), 4u);
    for (int index = 0; index < 3; ++index) {
        ASSERT_TRUE(main(opponent));
        ASSERT_TRUE(main(actor));
        ASSERT_TRUE(activate(0, std::nullopt));
    }
    ASSERT_EQ(p1.publicObjects.at(source).loyalty(), 7u);
    ASSERT_TRUE(main(opponent));
    ASSERT_TRUE(main(actor));
    ASSERT_TRUE(activate(2, artifact));
    for (const auto *recipient : {&p1, &p2}) {
        EXPECT_EQ(recipient->publicObjects.count(source), 0u);
        EXPECT_EQ(recipient->graveyardOwnerByEngineOid.at(source), actor);
        const auto &animated = recipient->publicObjects.at(artifact);
        EXPECT_TRUE(animated.is_creature());
        EXPECT_EQ(animated.power(), 9u);
        EXPECT_EQ(animated.toughness(), 9u);
        EXPECT_EQ(animated.zone_change_generation(), artifactGeneration);
        EXPECT_EQ(recipient->serverCardByEngineOid.at(artifact), artifactPhysical);
        for (const char *keyword : {"Flying", "Vigilance", "Indestructible"})
            EXPECT_NE(std::find(animated.keywords().begin(), animated.keywords().end(), keyword),
                      animated.keywords().end());
        EXPECT_EQ(std::find(animated.keywords().begin(), animated.keywords().end(), "Haste"),
                  animated.keywords().end());
        EXPECT_NE(std::find(animated.rules_annotation_labels().begin(), animated.rules_annotation_labels().end(),
                            "Can't be blocked"),
                  animated.rules_annotation_labels().end());
        EXPECT_TRUE(
            recipient->annotationByServerCardId.at(artifactPhysical).contains(QStringLiteral("Can't be blocked")));
    }
}
} // namespace ruled_e2e
