#include "ruled_e2e_opening_driver.h"
#include "ruled_e2e_session.h"
namespace ruled_e2e
{
namespace
{
TEST_F(RuledE2ESmokeTest, EquipmentAttachmentAndMerchantGraveyardReturnReachBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("attachmentp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("attachmentp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "attachment game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "attachment game start (p2)"));
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
    auto devPut = [&](const char *cardName, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(zone);
        put->set_ready(ready);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for attachment flow").arg(cardName));
    };
    auto devMove = [&](const char *cardName, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *move = dev->mutable_move_card();
        move->set_card_name(cardName);
        move->set_zone(zone);
        return sendAndPump(p1, command, QStringLiteral("dev: move %1 for attachment flow").arg(cardName));
    };
    auto passPriority = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in attachment flow"));
    };

    ASSERT_TRUE(devPut("Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    const auto bear = std::find_if(
        p1.battlefieldByPlayer[p1.myId].begin(), p1.battlefieldByPlayer[p1.myId].end(),
        [](const OpeningDriver::Permanent &permanent) { return permanent.cardId == QStringLiteral("grizzly_bears"); });
    ASSERT_NE(bear, p1.battlefieldByPlayer[p1.myId].end());
    const quint32 bearOid = bear->oid;

    ASSERT_TRUE(devPut("Illvoi Light Jammer", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(p1.pendingTriggerTarget.has_value());
    ASSERT_EQ(p1.pendingTriggerTarget->targets().groups_size(), 1);
    const auto &attachGroup = p1.pendingTriggerTarget->targets().groups(0);
    ASSERT_TRUE(std::find(attachGroup.valid_permanent_ids().begin(), attachGroup.valid_permanent_ids().end(),
                          bearOid) != attachGroup.valid_permanent_ids().end());
    ruled::v1::RuledCommand chooseAttachment;
    auto *attachmentTarget = chooseAttachment.mutable_choose_trigger_target()->add_targets();
    attachmentTarget->set_object_id(bearOid);
    attachmentTarget->set_group_index(attachGroup.group_index());
    attachmentTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p1, chooseAttachment, QStringLiteral("attach Illvoi Light Jammer")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    for (OpeningDriver *client : {&p1, &p2}) {
        const auto &battlefield = client->battlefieldByPlayer[p1.myId];
        const auto attachedEquipment =
            std::find_if(battlefield.begin(), battlefield.end(), [](const OpeningDriver::Permanent &permanent) {
                return permanent.cardId == QStringLiteral("illvoi_light_jammer");
            });
        ASSERT_NE(attachedEquipment, battlefield.end());
        EXPECT_EQ(attachedEquipment->attachmentObjectId, bearOid);
        const auto enhancedBear = std::find_if(battlefield.begin(), battlefield.end(),
                                               [bearOid](const auto &permanent) { return permanent.oid == bearOid; });
        ASSERT_NE(enhancedBear, battlefield.end());
        EXPECT_EQ(enhancedBear->power, 3);
        EXPECT_EQ(enhancedBear->toughness, 4);
        ASSERT_TRUE(client->serverCardByEngineOid.count(attachedEquipment->oid));
    }
    const auto p1Equipment =
        std::find_if(p1.battlefieldByPlayer[p1.myId].begin(), p1.battlefieldByPlayer[p1.myId].end(),
                     [](const auto &permanent) { return permanent.cardId == QStringLiteral("illvoi_light_jammer"); });
    ASSERT_NE(p1Equipment, p1.battlefieldByPlayer[p1.myId].end());
    EXPECT_EQ(p1.serverCardByEngineOid[p1Equipment->oid], p2.serverCardByEngineOid[p1Equipment->oid]);

    ASSERT_TRUE(devPut("Merchant of Many Hats", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devMove("Merchant of Many Hats", ruled::v1::DEV_ZONE_GRAVEYARD));
    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_b(1);
    addMana.mutable_dev_command()->mutable_add_mana()->set_c(2);
    ASSERT_TRUE(sendAndPump(p1, addMana, QStringLiteral("dev: add {2}{B} for Merchant")));

    const ruled::v1::LegalZoneAbilityAction *merchantAction = nullptr;
    for (const auto &action : p1.latestLegal.zone_ability_actions()) {
        if (action.card_name() == "Merchant of Many Hats" &&
            action.source_zone() == ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD) {
            merchantAction = &action;
            break;
        }
    }
    ASSERT_NE(merchantAction, nullptr);
    EXPECT_TRUE(std::none_of(p2.latestLegal.zone_ability_actions().begin(), p2.latestLegal.zone_ability_actions().end(),
                             [](const auto &action) { return action.card_name() == "Merchant of Many Hats"; }));
    const quint32 merchantOid = merchantAction->object_id();
    ASSERT_TRUE(p1.serverCardByEngineOid.count(merchantOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(merchantOid));
    EXPECT_EQ(p1.serverCardByEngineOid[merchantOid], p2.serverCardByEngineOid[merchantOid]);
    const int handBefore = p1.handSizeByPlayer[p1.myId];
    const auto opponentPrivateHandBefore = p2.handServerCardBySlot;

    ruled::v1::RuledCommand activateMerchant;
    auto *ability = activateMerchant.mutable_activate_ability();
    ability->set_source_object_id(merchantOid);
    ability->set_source_zone(merchantAction->source_zone());
    ability->set_expected_zone_change_generation(merchantAction->zone_change_generation());
    ability->set_ability_index(merchantAction->ability_index());
    ASSERT_TRUE(sendAndPump(p1, activateMerchant, QStringLiteral("activate Merchant from graveyard")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], handBefore + 1);
    EXPECT_EQ(p2.handServerCardBySlot, opponentPrivateHandBefore);
    EXPECT_TRUE(std::none_of(p1.latestLegal.zone_ability_actions().begin(), p1.latestLegal.zone_ability_actions().end(),
                             [](const auto &action) { return action.card_name() == "Merchant of Many Hats"; }));
}

TEST_F(RuledE2ESmokeTest, PlaneswalkerBattleTargetsSplitCombatAndSpecialCastReachBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("battlep1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("battlep2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "issue 72 game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "issue 72 game start (p2)"));
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
        return send(p1, command, QStringLiteral("issue 72 put %1").arg(name));
    };
    auto mana = [&](int blue, int black, int red) {
        ruled::v1::RuledCommand command;
        command.mutable_dev_command()->set_target_player_id(p1.myId);
        auto *gift = command.mutable_dev_command()->mutable_add_mana();
        gift->set_u(blue);
        gift->set_b(black);
        gift->set_r(red);
        return send(p1, command, QStringLiteral("issue 72 add mana"));
    };
    auto pass = [&](OpeningDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("issue 72 pass"));
    };
    auto find = [](const OpeningDriver &client, int controller,
                   const QString &id) -> std::optional<OpeningDriver::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto card =
            std::find_if(battlefield->second.begin(), battlefield->second.end(),
                         [&](const OpeningDriver::Permanent &permanent) { return permanent.cardId == id; });
        return card == battlefield->second.end() ? std::nullopt : std::optional(*card);
    };
    auto findMatching = [](const OpeningDriver &client, int controller,
                           const auto &predicate) -> std::optional<OpeningDriver::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto card = std::find_if(battlefield->second.begin(), battlefield->second.end(), predicate);
        return card == battlefield->second.end() ? std::nullopt : std::optional(*card);
    };
    auto castAt = [&](const QString &name, quint32 oid) {
        const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, name);
        if (!action) {
            return false;
        }
        const auto published = p1.latestLegal.valid_targets_by_hand_slot().find(action->hand_index() << 8);
        if (published == p1.latestLegal.valid_targets_by_hand_slot().end() || published->second.groups_size() != 1) {
            return false;
        }
        const auto &group = published->second.groups(0);
        if (std::find(group.valid_permanent_ids().begin(), group.valid_permanent_ids().end(), oid) ==
            group.valid_permanent_ids().end()) {
            return false;
        }
        ruled::v1::RuledCommand command;
        auto *cast = command.mutable_cast_spell();
        cast->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        cast->mutable_source()->set_hand_index(action->hand_index());
        auto *target = cast->add_targets();
        target->set_object_id(oid);
        target->set_group_index(group.group_index());
        target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
        return send(p1, command, QStringLiteral("issue 72 cast %1").arg(name));
    };

    ASSERT_TRUE(put(p1.myId, "Jace Beleren", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    auto ownJace = find(p1, p1.myId, QStringLiteral("jace_beleren"));
    ASSERT_TRUE(ownJace.has_value());
    ASSERT_TRUE(put(p1.myId, "Shock", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(mana(0, 0, 1));
    ASSERT_TRUE(castAt(QStringLiteral("Shock"), ownJace->oid));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    ownJace = find(p1, p1.myId, QStringLiteral("jace_beleren"));
    ASSERT_TRUE(ownJace.has_value());
    EXPECT_TRUE(ownJace->planeswalker);
    EXPECT_EQ(ownJace->loyalty, 1);
    ASSERT_TRUE(find(p2, p1.myId, QStringLiteral("jace_beleren")).has_value());
    EXPECT_EQ(find(p2, p1.myId, QStringLiteral("jace_beleren"))->loyalty, 1);

    ruled::v1::RuledCommand activate;
    p1.setBattlefieldAbilitySource(activate.mutable_activate_ability(), ownJace->oid);
    activate.mutable_activate_ability()->set_ability_index(0);
    ASSERT_TRUE(send(p1, activate, QStringLiteral("issue 72 activate Jace +2")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    EXPECT_EQ(find(p1, p1.myId, QStringLiteral("jace_beleren"))->loyalty, 3);
    EXPECT_FALSE(find(p1, p1.myId, QStringLiteral("jace_beleren"))->firstAbilityActivatable);

    ASSERT_TRUE(put(p2.myId, "Jace Beleren", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    auto opposingJace = find(p1, p2.myId, QStringLiteral("jace_beleren"));
    ASSERT_TRUE(opposingJace.has_value());
    ASSERT_TRUE(put(p1.myId, "Finishing Blow", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(mana(0, 5, 0));
    ASSERT_TRUE(castAt(QStringLiteral("Finishing Blow"), opposingJace->oid));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    EXPECT_FALSE(find(p1, p2.myId, QStringLiteral("jace_beleren")).has_value());

    ASSERT_TRUE(put(p2.myId, "Jace Beleren", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(put(p1.myId, "Invasion of Ulgrotha // Grandmother Ravi Sengir", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(p1.pendingChoice.has_value());
    ASSERT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_BATTLE_PROTECTOR);
    EXPECT_FALSE(p2.pendingChoice.has_value());
    EXPECT_FALSE(findMatching(p1, p1.myId, [](const OpeningDriver::Permanent &permanent) {
                     return permanent.battle;
                 }).has_value());
    ruled::v1::RuledCommand chooseProtector;
    chooseProtector.mutable_submit_resolution_choice()->add_chosen_object_ids(p2.myId);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, chooseProtector, QStringLiteral("issue 72 choose Battle protector")));
    ASSERT_TRUE(p1.pendingTriggerTarget.has_value());
    ruled::v1::RuledCommand targetEtb;
    auto *playerTarget = targetEtb.mutable_choose_trigger_target()->add_targets();
    playerTarget->set_object_id(p2.myId);
    playerTarget->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
    p1.pendingTriggerTarget.reset();
    ASSERT_TRUE(send(p1, targetEtb, QStringLiteral("issue 72 choose Battle ETB target")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));

    auto battle = findMatching(p1, p1.myId, [](const OpeningDriver::Permanent &permanent) { return permanent.battle; });
    opposingJace = find(p1, p2.myId, QStringLiteral("jace_beleren"));
    ASSERT_TRUE(battle.has_value() && opposingJace.has_value());
    EXPECT_TRUE(battle->battle);
    EXPECT_EQ(battle->defense, 5);
    EXPECT_EQ(battle->battleProtector, p2.myId);
    ASSERT_TRUE(findMatching(p2, p1.myId, [](const OpeningDriver::Permanent &permanent) {
                    return permanent.battle;
                }).has_value());
    EXPECT_EQ(
        findMatching(p2, p1.myId, [](const OpeningDriver::Permanent &permanent) { return permanent.battle; })->defense,
        5);
    const quint32 battleOid = battle->oid;
    ASSERT_TRUE(p1.serverCardByEngineOid.count(battleOid));
    const int physicalId = p1.serverCardByEngineOid[battleOid];
    EXPECT_EQ(p2.serverCardByEngineOid[battleOid], physicalId);

    ASSERT_TRUE(put(p1.myId, "Hill Giant", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(put(p1.myId, "Serra Angel", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(put(p1.myId, "Craw Wurm", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    QElapsedTimer toAttack;
    toAttack.start();
    while (p1.phase != ruled::v1::PHASE_ID_DECLARE_ATTACKERS && toAttack.elapsed() < 20000) {
        ASSERT_TRUE(pass(p1.priorityPlayer == p1.myId ? p1 : p2));
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_DECLARE_ATTACKERS);
    const auto hill = find(p1, p1.myId, QStringLiteral("hill_giant"));
    const auto angel = find(p1, p1.myId, QStringLiteral("serra_angel"));
    const auto wurm = find(p1, p1.myId, QStringLiteral("craw_wurm"));
    ASSERT_TRUE(hill.has_value() && angel.has_value() && wurm.has_value());
    std::map<quint32, ruled::v1::AttackAssignment> selected;
    for (const auto &assignment : p1.latestLegal.legal_attack_assignments()) {
        const auto &defender = assignment.defender();
        if ((assignment.attacker_object_id() == hill->oid && defender.kind() == ruled::v1::TARGET_REF_KIND_PLAYER &&
             defender.object_id() == static_cast<quint32>(p2.myId)) ||
            (assignment.attacker_object_id() == angel->oid && defender.object_id() == opposingJace->oid) ||
            (assignment.attacker_object_id() == wurm->oid && defender.object_id() == battleOid)) {
            selected[assignment.attacker_object_id()] = assignment;
        }
    }
    ASSERT_EQ(selected.size(), 3u);
    ruled::v1::RuledCommand preview;
    ruled::v1::RuledCommand declare;
    for (const auto &[_, assignment] : selected) {
        *preview.mutable_preview_declare_attackers()->add_assignments() = assignment;
        *declare.mutable_declare_attackers()->add_assignments() = assignment;
    }
    ASSERT_TRUE(send(p1, preview, QStringLiteral("issue 72 preview split attackers")));
    EXPECT_EQ(p1.latestAttackPreviewAssignments.size(), 3u);
    EXPECT_EQ(p2.latestAttackPreviewAssignments.size(), 3u);
    ASSERT_TRUE(send(p1, declare, QStringLiteral("issue 72 declare split attackers")));
    EXPECT_EQ(p1.latestDeclaredAttackAssignments.size(), 3u);
    EXPECT_EQ(p2.latestDeclaredAttackAssignments.size(), 3u);

    // With no eligible blockers, the authoritative engine auto-commits an empty declaration and
    // advances directly into combat damage.
    QElapsedTimer siege;
    siege.start();
    while ((!p1.pendingChoice.has_value() || p1.pendingChoice->choice_kind() != ruled::v1::CHOICE_KIND_SPECIAL_CAST) &&
           siege.elapsed() < 30000) {
        if (p1.priorityPlayer == p1.myId) {
            ASSERT_TRUE(pass(p1));
        } else if (p1.priorityPlayer == p2.myId) {
            ASSERT_TRUE(pass(p2));
        } else {
            p1.pump(25);
            p2.pump(25);
        }
    }
    ASSERT_TRUE(p1.pendingChoice.has_value());
    ASSERT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_SPECIAL_CAST);
    EXPECT_FALSE(p2.pendingChoice.has_value());
    ASSERT_TRUE(p1.serverCardByEngineOid.count(battleOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(battleOid));
    const int exiledPhysicalId = p1.serverCardByEngineOid[battleOid];
    EXPECT_EQ(p2.serverCardByEngineOid[battleOid], exiledPhysicalId);

    ruled::v1::RuledCommand castBack;
    auto *submission = castBack.mutable_submit_resolution_choice();
    submission->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_CAST_SPELL);
    auto *cast = submission->mutable_cast_spell();
    const auto offer =
        std::find_if(p1.latestLegal.zone_cast_actions().begin(), p1.latestLegal.zone_cast_actions().end(),
                     [battleOid](const auto &action) { return action.object_id() == battleOid; });
    ASSERT_NE(offer, p1.latestLegal.zone_cast_actions().end());
    cast->set_casting_permission_id(offer->casting_permission_id());
    cast->mutable_source()->set_expected_zone_change_generation(offer->zone_change_generation());
    cast->set_cast_method(ruled::v1::CAST_METHOD_SIEGE_DEFEAT);
    cast->set_face_index(1);
    cast->mutable_source()->set_exile_object_id(battleOid);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, castBack, QStringLiteral("issue 72 cast Siege transformed")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    battle = findMatching(
        p1, p1.myId, [battleOid](const OpeningDriver::Permanent &permanent) { return permanent.oid == battleOid; });
    const auto observer = findMatching(
        p2, p1.myId, [battleOid](const OpeningDriver::Permanent &permanent) { return permanent.oid == battleOid; });
    ASSERT_TRUE(battle.has_value() && observer.has_value());
    EXPECT_EQ(battle->oid, battleOid);
    EXPECT_EQ(observer->oid, battleOid);
    EXPECT_EQ(battle->faceIndex, 1);
    EXPECT_EQ(observer->faceIndex, 1);
    EXPECT_TRUE(battle->creature && observer->creature);
    EXPECT_EQ(battle->defense, -1);
    EXPECT_EQ(observer->defense, -1);
    EXPECT_EQ(p1.serverCardByEngineOid[battleOid], exiledPhysicalId);
    EXPECT_EQ(p2.serverCardByEngineOid[battleOid], exiledPhysicalId);
}

TEST_F(RuledE2ESmokeTest, KaitoStaticEmblemCreatesPresentationTokenForBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("kaitop1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("kaitop2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "issue 203 game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "issue 203 game start (p2)"));
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
        return send(client, command, QStringLiteral("issue 203 pass"));
    };

    ruled::v1::RuledCommand putKaito;
    auto *dev = putKaito.mutable_dev_command();
    dev->set_target_player_id(p1.myId);
    auto *placement = dev->mutable_put_card_in_zone();
    placement->set_card_name("Kaito, Bane of Nightmares");
    placement->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
    placement->set_ready(true);
    ASSERT_TRUE(send(p1, putKaito, QStringLiteral("issue 203 put Kaito")));

    const auto ownBattlefield = p1.battlefieldByPlayer.find(p1.myId);
    ASSERT_NE(ownBattlefield, p1.battlefieldByPlayer.end());
    ASSERT_EQ(ownBattlefield->second.size(), 1u);
    const auto &kaito = ownBattlefield->second.front();
    EXPECT_EQ(kaito.abilityIndices, std::vector<quint32>({1, 2, 3}));
    ASSERT_EQ(p2.battlefieldByPlayer[p1.myId].size(), 1u);
    EXPECT_EQ(p2.battlefieldByPlayer[p1.myId].front().abilityIndices, std::vector<quint32>({1, 2, 3}));
    const std::size_t physicalCardsBefore = p1.physicalRowAndPt.size();

    ruled::v1::RuledCommand activate;
    p1.setBattlefieldAbilitySource(activate.mutable_activate_ability(), kaito.oid);
    activate.mutable_activate_ability()->set_ability_index(1);
    ASSERT_TRUE(send(p1, activate, QStringLiteral("issue 203 activate Kaito +1")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));

    const QString emblemName = QStringLiteral("Kaito, Bane of Nightmares Emblem");
    auto hasEmblemCreate = [&](const OpeningDriver &client) {
        return std::any_of(client.physicalCreateTokenEvents.begin(), client.physicalCreateTokenEvents.end(),
                           [&](const Event_CreateToken &event) {
                               return event.zone_name() == ZoneNames::TABLE &&
                                      QString::fromStdString(event.card_name()) == emblemName &&
                                      event.annotation() == "Emblem";
                           });
    };
    EXPECT_TRUE(hasEmblemCreate(p1));
    EXPECT_TRUE(hasEmblemCreate(p2));
    EXPECT_EQ(p1.physicalRowAndPt.size(), physicalCardsBefore + 1);
    EXPECT_EQ(p2.physicalRowAndPt.size(), physicalCardsBefore + 1);
}

class MobilizeDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    bool sawMobilizeDefenderChoice = false;
    bool sawMobilizeObserverWait = false;
    bool sawMobilizeTokenCreated = false;
    bool sawMobilizeTokenSacrificed = false;
    quint32 mobilizeTokenOid = 0;
    void onRuledEvent(const ruled::v1::RuledEvent &ev) override
    {
        OpeningDriver::onRuledEvent(ev);
        if (ev.has_resolution_choice_required()) {
            const auto &rcr = ev.resolution_choice_required();
            if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_ATTACKING_TOKEN_DEFENDER) {
                if (rcr.deciding_player_id() == myId) {
                    sawMobilizeDefenderChoice =
                        rcr.min() == 1 && rcr.max() == 1 && rcr.combat_defender_options_size() >= 2;
                } else {
                    sawMobilizeObserverWait = rcr.combat_defender_options_size() == 0;
                }
            }
        }
        if (ev.has_token_created()) {
            const auto &token = ev.token_created();
            if (token.card_id() == "warrior_r_1_1") {
                sawMobilizeTokenCreated =
                    token.enters_tapped() && token.identity().name() == "Warrior" && token.identity().pt() == "1/1";
                mobilizeTokenOid = token.object_id();
            }
        }
        if (ev.has_permanent_moved()) {
            const auto &moved = ev.permanent_moved();
            if (mobilizeTokenOid != 0 && moved.object_id() == mobilizeTokenOid &&
                moved.destination() == ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD) {
                sawMobilizeTokenSacrificed = true;
            }
        }
    }
};

TEST_F(RuledE2ESmokeTest, MobilizeDefenderChoiceAndTokenLifecycleReachBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    MobilizeDriver p1(true, QStringLiteral("mobilizep1"), &transcript);
    MobilizeDriver p2(false, QStringLiteral("mobilizep2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "issue 106 game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "issue 106 game start (p2)"));
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

    auto send = [&](MobilizeDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
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
    auto put = [&](int player, const char *name) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(player);
        auto *placement = dev->mutable_put_card_in_zone();
        placement->set_card_name(name);
        placement->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        placement->set_ready(true);
        return send(p1, command, QStringLiteral("issue 106 put %1").arg(name));
    };
    auto pass = [&](MobilizeDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("issue 106 pass"));
    };
    auto find = [](const MobilizeDriver &client, int controller,
                   const QString &id) -> std::optional<MobilizeDriver::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto card =
            std::find_if(battlefield->second.begin(), battlefield->second.end(),
                         [&](const MobilizeDriver::Permanent &permanent) { return permanent.cardId == id; });
        return card == battlefield->second.end() ? std::nullopt : std::optional(*card);
    };

    ASSERT_TRUE(put(p2.myId, "Jace Beleren"));
    ASSERT_TRUE(put(p1.myId, "Dragonback Lancer"));
    const auto lancer = find(p1, p1.myId, QStringLiteral("dragonback_lancer"));
    const auto jace = find(p1, p2.myId, QStringLiteral("jace_beleren"));
    ASSERT_TRUE(lancer.has_value() && jace.has_value());

    QElapsedTimer toAttack;
    toAttack.start();
    while (p1.phase != ruled::v1::PHASE_ID_DECLARE_ATTACKERS && toAttack.elapsed() < 20000) {
        ASSERT_TRUE(pass(p1.priorityPlayer == p1.myId ? p1 : p2));
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_DECLARE_ATTACKERS);

    const auto assignment =
        std::find_if(p1.latestLegal.legal_attack_assignments().begin(), p1.latestLegal.legal_attack_assignments().end(),
                     [&](const ruled::v1::AttackAssignment &candidate) {
                         return candidate.attacker_object_id() == lancer->oid && candidate.has_defender() &&
                                candidate.defender().kind() == ruled::v1::TARGET_REF_KIND_PLAYER &&
                                candidate.defender().object_id() == static_cast<quint32>(p2.myId);
                     });
    ASSERT_NE(assignment, p1.latestLegal.legal_attack_assignments().end());
    ruled::v1::RuledCommand declare;
    *declare.mutable_declare_attackers()->add_assignments() = *assignment;
    ASSERT_TRUE(send(p1, declare, QStringLiteral("issue 106 declare Dragonback Lancer")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));

    ASSERT_TRUE(p1.pendingChoice.has_value());
    ASSERT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_ATTACKING_TOKEN_DEFENDER);
    EXPECT_TRUE(p1.sawMobilizeDefenderChoice);
    EXPECT_TRUE(p2.sawMobilizeObserverWait);
    EXPECT_FALSE(p2.pendingChoice.has_value());
    const auto defender = std::find_if(p1.pendingChoice->combat_defender_options().begin(),
                                       p1.pendingChoice->combat_defender_options().end(),
                                       [&](const ruled::v1::CombatDefenderOption &option) {
                                           return option.has_defender() &&
                                                  option.defender().kind() == ruled::v1::TARGET_REF_KIND_PERMANENT &&
                                                  option.defender().object_id() == jace->oid &&
                                                  option.defender_zone_change_generation() == jace->generation;
                                       });
    ASSERT_NE(defender, p1.pendingChoice->combat_defender_options().end());

    ruled::v1::RuledCommand chooseDefender;
    *chooseDefender.mutable_submit_resolution_choice()->mutable_chosen_combat_defender() = *defender;
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, chooseDefender, QStringLiteral("issue 106 choose Jace defender")));

    ASSERT_TRUE(p1.sawMobilizeTokenCreated && p2.sawMobilizeTokenCreated);
    ASSERT_NE(p1.mobilizeTokenOid, 0u);
    ASSERT_EQ(p1.mobilizeTokenOid, p2.mobilizeTokenOid);
    const quint32 tokenOid = p1.mobilizeTokenOid;
    ASSERT_TRUE(p1.serverCardByEngineOid.count(tokenOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(tokenOid));
    const int physicalTokenId = p1.serverCardByEngineOid[tokenOid];
    EXPECT_EQ(p2.serverCardByEngineOid[tokenOid], physicalTokenId);
    EXPECT_TRUE(p1.physicallyTappedCardIds.count(physicalTokenId));
    EXPECT_TRUE(p2.physicallyTappedCardIds.count(physicalTokenId));
    EXPECT_TRUE(p1.physicallyAttackingCardIds.count(physicalTokenId));
    EXPECT_TRUE(p2.physicallyAttackingCardIds.count(physicalTokenId));
    ASSERT_EQ(p1.latestAddedAttackAssignments.size(), 1u);
    ASSERT_EQ(p2.latestAddedAttackAssignments.size(), 1u);
    EXPECT_EQ(p1.latestAddedAttackAssignments.front().attacker_object_id(), tokenOid);
    EXPECT_EQ(p1.latestAddedAttackAssignments.front().defender().object_id(), jace->oid);
    EXPECT_EQ(p2.latestAddedAttackAssignments.front().defender().object_id(), jace->oid);

    QElapsedTimer toEndStep;
    toEndStep.start();
    while (!(p1.phase == ruled::v1::PHASE_ID_END_STEP && p1.stackDepth > 0) && toEndStep.elapsed() < 30000) {
        if (p1.priorityPlayer == p1.myId) {
            ASSERT_TRUE(pass(p1));
        } else if (p1.priorityPlayer == p2.myId) {
            ASSERT_TRUE(pass(p2));
        } else {
            p1.pump(25);
            p2.pump(25);
        }
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_END_STEP);
    ASSERT_GT(p1.stackDepth, 0);
    ASSERT_TRUE(pass(p1.priorityPlayer == p1.myId ? p1 : p2));
    ASSERT_TRUE(pass(p1.priorityPlayer == p1.myId ? p1 : p2));
    EXPECT_TRUE(p1.sawMobilizeTokenSacrificed && p2.sawMobilizeTokenSacrificed);
}

TEST_F(RuledE2ESmokeTest, TokenCopiesAndPopulatePreserveBothClientsPhysicalIdentity)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    OpeningDriver p1(true, QStringLiteral("copytokenp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("copytokenp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "copy game start"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "copy game start"));
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
    auto send = [&](OpeningDriver &sender, const ruled::v1::RuledCommand &command) {
        const auto before1 = p1.stateVersion;
        const auto before2 = p2.stateVersion;
        sender.sendRuled(command, QStringLiteral("issue 46 command"));
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto pass = [&](OpeningDriver &sender) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(sender, command);
    };
    auto put = [&](const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        dev->mutable_put_card_in_zone()->set_zone(zone);
        return send(p1, command);
    };
    auto cast = [&](const QString &name, quint32 oid) {
        const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, name);
        if (!action) {
            return false;
        }
        ruled::v1::RuledCommand command;
        auto *spell = command.mutable_cast_spell();
        spell->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        spell->mutable_source()->set_hand_index(action->hand_index());
        if (oid != 0) {
            const auto published = p1.latestLegal.valid_targets_by_hand_slot().find(action->hand_index() << 8);
            if (published == p1.latestLegal.valid_targets_by_hand_slot().end() ||
                published->second.groups_size() != 1) {
                return false;
            }
            const auto &group = published->second.groups(0);
            if (std::find(group.valid_permanent_ids().begin(), group.valid_permanent_ids().end(), oid) ==
                group.valid_permanent_ids().end()) {
                return false;
            }
            auto *target = spell->add_targets();
            target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
            target->set_object_id(oid);
            target->set_group_index(group.group_index());
        }
        return send(p1, command);
    };
    ASSERT_TRUE(put("Serra Angel", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_EQ(p1.battlefieldByPlayer[p1.myId].size(), 1u);
    const quint32 original = p1.battlefieldByPlayer[p1.myId][0].oid;
    const int originalPhysical = p1.serverCardByEngineOid.at(original);
    ASSERT_TRUE(put("Cackling Counterpart", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(put("Wake the Reflections", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(put("Unsummon", ruled::v1::DEV_ZONE_HAND));
    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_u(3);
    mana.mutable_dev_command()->mutable_add_mana()->set_w(1);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(1);
    ASSERT_TRUE(send(p1, mana));
    ASSERT_TRUE(cast(QStringLiteral("Cackling Counterpart"), original));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    ASSERT_EQ(p1.battlefieldByPlayer[p1.myId].size(), 2u);
    ASSERT_EQ(p2.battlefieldByPlayer[p1.myId].size(), 2u);
    const auto copied = std::find_if(p1.battlefieldByPlayer[p1.myId].begin(), p1.battlefieldByPlayer[p1.myId].end(),
                                     [original](const auto &permanent) { return permanent.oid != original; });
    ASSERT_NE(copied, p1.battlefieldByPlayer[p1.myId].end());
    const quint32 token = copied->oid;
    EXPECT_EQ(copied->power, 4);
    EXPECT_EQ(copied->toughness, 4);
    EXPECT_NE(p1.serverCardByEngineOid.at(token), originalPhysical);
    EXPECT_EQ(p1.serverCardByEngineOid.at(token), p2.serverCardByEngineOid.at(token));
    ASSERT_TRUE(cast(QStringLiteral("Wake the Reflections"), 0));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    ASSERT_TRUE(p1.pendingChoice.has_value());
    EXPECT_FALSE(p2.pendingChoice.has_value());
    EXPECT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_COPY_SOURCE);
    EXPECT_EQ(p1.pendingChoice->min(), 1u);
    ASSERT_EQ(p1.pendingChoice->candidate_object_ids_size(), 1);
    EXPECT_EQ(p1.pendingChoice->candidate_object_ids(0), token);
    ruled::v1::RuledCommand choose;
    choose.mutable_submit_resolution_choice()->add_chosen_object_ids(token);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, choose));
    ASSERT_EQ(p1.battlefieldByPlayer[p1.myId].size(), 3u);
    ASSERT_EQ(p2.battlefieldByPlayer[p1.myId].size(), 3u);
    EXPECT_EQ(p1.serverCardByEngineOid.at(original), originalPhysical);
    ASSERT_TRUE(cast(QStringLiteral("Unsummon"), token));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    ASSERT_EQ(p1.battlefieldByPlayer[p1.myId].size(), 2u);
    ASSERT_EQ(p2.battlefieldByPlayer[p1.myId].size(), 2u);
    EXPECT_EQ(p1.serverCardByEngineOid.at(original), originalPhysical);
    for (const auto &permanent : p1.battlefieldByPlayer[p1.myId]) {
        EXPECT_NE(permanent.oid, token);
        EXPECT_EQ(p1.serverCardByEngineOid.at(permanent.oid), p2.serverCardByEngineOid.at(permanent.oid));
    }
}

TEST_F(RuledE2ESmokeTest, EarthbendBadgeRowsAndGenerationBoundReturnReachBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    OpeningDriver p1(true, QStringLiteral("earthbendp1"), &transcript);
    OpeningDriver p2(false, QStringLiteral("earthbendp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "issue 150 game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "issue 150 game start (p2)"));
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
        return send(client, command, QStringLiteral("issue 150 pass"));
    };
    auto put = [&](const char *name, ruled::v1::DevZone zone = ruled::v1::DEV_ZONE_BATTLEFIELD) {
        ruled::v1::RuledCommand command;
        command.mutable_dev_command()->set_target_player_id(p1.myId);
        auto *placement = command.mutable_dev_command()->mutable_put_card_in_zone();
        placement->set_card_name(name);
        placement->set_zone(zone);
        placement->set_ready(true);
        return send(p1, command, QStringLiteral("earthbend put %1").arg(name));
    };
    auto find = [&](const OpeningDriver &client, const char *cardId) -> std::optional<OpeningDriver::Permanent> {
        const auto &objects = client.battlefieldByPlayer.at(p1.myId);
        const auto found = std::find_if(objects.begin(), objects.end(), [cardId](const auto &object) {
            return object.cardId == QLatin1String(cardId);
        });
        return found == objects.end() ? std::nullopt : std::optional(*found);
    };
    ASSERT_TRUE(put("Forest"));
    const auto forest = find(p1, "forest");
    ASSERT_TRUE(forest.has_value());
    const quint32 oid = forest->oid;
    ASSERT_TRUE(p1.serverCardByEngineOid.count(oid));
    const int physicalId = p1.serverCardByEngineOid.at(oid);
    const auto key = std::make_pair(p1.myId, physicalId);
    for (OpeningDriver *client : {&p1, &p2}) {
        ASSERT_TRUE(client->physicalRowAndPt.count(key));
        EXPECT_EQ(client->physicalRowAndPt.at(key).first, 2);
        EXPECT_TRUE(client->physicalRowAndPt.at(key).second.isEmpty());
    }
    for (const auto destination : {ruled::v1::DEV_ZONE_GRAVEYARD, ruled::v1::DEV_ZONE_EXILE}) {
        ASSERT_TRUE(put("Rebellious Captives"));
        const auto captives = find(p1, "rebellious_captives");
        ASSERT_TRUE(captives.has_value());
        ruled::v1::RuledCommand mana;
        mana.mutable_dev_command()->set_target_player_id(p1.myId);
        mana.mutable_dev_command()->mutable_add_mana()->set_c(6);
        ASSERT_TRUE(send(p1, mana, QStringLiteral("earthbend mana")));
        ruled::v1::RuledCommand activate;
        p1.setBattlefieldAbilitySource(activate.mutable_activate_ability(), captives->oid);
        activate.mutable_activate_ability()->set_ability_index(0);
        auto *target = activate.mutable_activate_ability()->add_targets();
        target->set_object_id(oid);
        target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
        ASSERT_TRUE(send(p1, activate, QStringLiteral("earthbend exhaust")));
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        for (OpeningDriver *client : {&p1, &p2}) {
            const auto animated = find(*client, "forest");
            ASSERT_TRUE(animated.has_value());
            EXPECT_TRUE(animated->creature && animated->haste);
            EXPECT_EQ(animated->power, 2);
            EXPECT_EQ(animated->toughness, 2);
            EXPECT_EQ(client->serverCardByEngineOid.at(oid), physicalId);
            ASSERT_TRUE(client->physicalRowAndPt.count(key));
            EXPECT_EQ(client->physicalRowAndPt.at(key).first, 0);
            EXPECT_EQ(client->physicalRowAndPt.at(key).second, QStringLiteral("2/2"));
        }
        const quint64 beforeGeneration = find(p1, "forest")->generation;
        auto move = [&](const char *name, ruled::v1::DevZone zone) {
            ruled::v1::RuledCommand command;
            command.mutable_dev_command()->set_target_player_id(p1.myId);
            auto *movement = command.mutable_dev_command()->mutable_move_card();
            movement->set_card_name(name);
            movement->set_zone(zone);
            return send(p1, command, QStringLiteral("earthbend move %1").arg(name));
        };
        ASSERT_TRUE(move("Rebellious Captives", ruled::v1::DEV_ZONE_GRAVEYARD));
        // Use real spells: their own stack-to-graveyard move shares the batch with
        // the land's departure and must not swap the two physical identities.
        const char *removal = destination == ruled::v1::DEV_ZONE_GRAVEYARD ? "Lightning Bolt" : "Swords to Plowshares";
        ASSERT_TRUE(put(removal, ruled::v1::DEV_ZONE_HAND));
        ruled::v1::RuledCommand removalMana;
        removalMana.mutable_dev_command()->set_target_player_id(p1.myId);
        auto *gift = removalMana.mutable_dev_command()->mutable_add_mana();
        gift->set_r(1);
        gift->set_w(1);
        ASSERT_TRUE(send(p1, removalMana, QStringLiteral("earthbend removal mana")));
        const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QString::fromLatin1(removal));
        ASSERT_NE(action, nullptr);
        ruled::v1::RuledCommand cast;
        cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        cast.mutable_cast_spell()->mutable_source()->set_hand_index(action->hand_index());
        auto *removalTarget = cast.mutable_cast_spell()->add_targets();
        removalTarget->set_object_id(oid);
        removalTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
        ASSERT_TRUE(send(p1, cast, QStringLiteral("earthbend cast removal")));
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        EXPECT_EQ(p1.stackDepth, 1);
        EXPECT_EQ(p2.stackDepth, 1);
        EXPECT_FALSE(find(p1, "forest").has_value());
        EXPECT_FALSE(find(p2, "forest").has_value());
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        for (OpeningDriver *client : {&p1, &p2}) {
            const auto returned = find(*client, "forest");
            ASSERT_TRUE(returned.has_value());
            EXPECT_EQ(returned->generation, beforeGeneration + 2);
            EXPECT_TRUE(returned->tapped);
            EXPECT_FALSE(returned->creature || returned->haste);
            EXPECT_EQ(client->serverCardByEngineOid.at(oid), physicalId);
            ASSERT_TRUE(client->physicalRowAndPt.count(key));
            EXPECT_EQ(client->physicalRowAndPt.at(key).first, 2);
            EXPECT_TRUE(client->physicalRowAndPt.at(key).second.isEmpty());
        }
    }
}

class TappedTokenDriver : public OpeningDriver
{
public:
    using OpeningDriver::OpeningDriver;
    bool sawTappedOrdinaryTokenCreated = false;
    quint32 tappedOrdinaryTokenOid = 0;
    void onRuledEvent(const ruled::v1::RuledEvent &ev) override
    {
        OpeningDriver::onRuledEvent(ev);
        if (ev.has_token_created()) {
            const auto &token = ev.token_created();
            if (token.card_id() == "robot_c_2_2") {
                sawTappedOrdinaryTokenCreated =
                    token.enters_tapped() && token.identity().name() == "Robot" && token.identity().pt() == "2/2";
                tappedOrdinaryTokenOid = token.object_id();
            }
        }
    }
};

TEST_F(RuledE2ESmokeTest, TappedOrdinaryTokenReachesBothClientsWithoutCombatState)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    TappedTokenDriver p1(true, QStringLiteral("tappedtokenp1"), &transcript);
    TappedTokenDriver p2(false, QStringLiteral("tappedtokenp2"), &transcript);
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(
        p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "issue 162 game start (p1)"));
    ASSERT_TRUE(
        p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "issue 162 game start (p2)"));
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

    auto send = [&](TappedTokenDriver &sender, const ruled::v1::RuledCommand &command, const QString &description) {
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
    auto pass = [&](TappedTokenDriver &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("issue 162 pass"));
    };
    auto findToken = [](const TappedTokenDriver &client, int controller,
                        quint32 objectId) -> std::optional<TappedTokenDriver::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto token = std::find_if(
            battlefield->second.begin(), battlefield->second.end(),
            [objectId](const TappedTokenDriver::Permanent &permanent) { return permanent.oid == objectId; });
        return token == battlefield->second.end() ? std::nullopt : std::optional(*token);
    };

    ruled::v1::RuledCommand putMoxite;
    putMoxite.mutable_dev_command()->set_target_player_id(p1.myId);
    auto *placement = putMoxite.mutable_dev_command()->mutable_put_card_in_zone();
    placement->set_card_name("Melded Moxite");
    placement->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
    placement->set_ready(true);
    ASSERT_TRUE(send(p1, putMoxite, QStringLiteral("issue 162 put Melded Moxite")));

    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_c(3);
    ASSERT_TRUE(send(p1, addMana, QStringLiteral("issue 162 add {3}")));

    const auto moxite = std::find_if(p1.battlefieldByPlayer[p1.myId].begin(), p1.battlefieldByPlayer[p1.myId].end(),
                                     [](const TappedTokenDriver::Permanent &permanent) {
                                         return permanent.cardId == QStringLiteral("melded_moxite");
                                     });
    ASSERT_NE(moxite, p1.battlefieldByPlayer[p1.myId].end());
    ruled::v1::RuledCommand activate;
    p1.setBattlefieldAbilitySource(activate.mutable_activate_ability(), moxite->oid);
    activate.mutable_activate_ability()->set_ability_index(0);
    ASSERT_TRUE(send(p1, activate, QStringLiteral("issue 162 activate Melded Moxite")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));

    ASSERT_TRUE(p1.sawTappedOrdinaryTokenCreated && p2.sawTappedOrdinaryTokenCreated);
    ASSERT_NE(p1.tappedOrdinaryTokenOid, 0u);
    ASSERT_EQ(p1.tappedOrdinaryTokenOid, p2.tappedOrdinaryTokenOid);
    const quint32 tokenOid = p1.tappedOrdinaryTokenOid;
    ASSERT_TRUE(p1.serverCardByEngineOid.count(tokenOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(tokenOid));
    const int physicalTokenId = p1.serverCardByEngineOid[tokenOid];
    EXPECT_EQ(p2.serverCardByEngineOid[tokenOid], physicalTokenId);
    EXPECT_TRUE(p1.physicallyTappedCardIds.count(physicalTokenId));
    EXPECT_TRUE(p2.physicallyTappedCardIds.count(physicalTokenId));
    EXPECT_FALSE(p1.physicallyAttackingCardIds.count(physicalTokenId));
    EXPECT_FALSE(p2.physicallyAttackingCardIds.count(physicalTokenId));
    const auto p1Robot = findToken(p1, p1.myId, tokenOid);
    const auto p2Robot = findToken(p2, p1.myId, tokenOid);
    ASSERT_TRUE(p1Robot.has_value() && p2Robot.has_value());
    EXPECT_EQ(p1Robot->oid, tokenOid);
    EXPECT_EQ(p2Robot->oid, tokenOid);
    EXPECT_TRUE(p1Robot->tapped && p2Robot->tapped);
}

} // namespace
} // namespace ruled_e2e
