#include "ruled_e2e_observed_state.h"
namespace ruled_e2e
{
void ObservedState::observePhysicalEvent(const GameEvent &ev)
{
    if (ev.HasExtension(Event_GameStateChanged::ext)) {
        const auto &gsc = ev.GetExtension(Event_GameStateChanged::ext);
        if (gsc.has_game_started()) {
            gameStarted = gsc.game_started();
        }
        for (const auto &player : gsc.player_list()) {
            for (const auto &zone : player.zone_list()) {
                if (zone.name() != ZoneNames::TABLE) {
                    continue;
                }
                const int playerId = player.properties().player_id();
                for (auto it = physicalRowAndPt.begin(); it != physicalRowAndPt.end();) {
                    if (it->first.first == playerId) {
                        it = physicalRowAndPt.erase(it);
                    } else {
                        ++it;
                    }
                }
                for (const auto &card : zone.card_list()) {
                    physicalRowAndPt[{playerId, card.id()}] = {card.y(), QString::fromStdString(card.pt())};
                }
            }
        }
    }
    if (ev.HasExtension(Event_CreateToken::ext)) {
        const auto &created = ev.GetExtension(Event_CreateToken::ext);
        physicalCreateTokenEvents.push_back(created);
        if (created.zone_name() == ZoneNames::TABLE) {
            physicalRowAndPt[{ev.player_id(), created.card_id()}] = {created.y(), QString::fromStdString(created.pt())};
        }
    }
    if (ev.HasExtension(Event_RevealCards::ext)) {
        physicalRevealEvents.push_back(ev.GetExtension(Event_RevealCards::ext));
    }
    if (ev.HasExtension(Event_MoveCard::ext)) {
        const auto &mc = ev.GetExtension(Event_MoveCard::ext);
        physicalMoveEvents.push_back(mc);
        const QString from = QString::fromStdString(mc.start_zone());
        const QString to = QString::fromStdString(mc.target_zone());
        const QString name = QString::fromStdString(mc.card_name());
        // ZoneNames, not literals: Cockatrice's exile zone is spelled "rfg".

        const QLatin1String exile(ZoneNames::EXILE);
        const QLatin1String table(ZoneNames::TABLE);

        const auto oldKey = std::make_pair(mc.start_player_id(), mc.card_id());
        const QString oldPt = physicalRowAndPt.count(oldKey) ? physicalRowAndPt.at(oldKey).second : QString();
        if (from == table) {
            physicalRowAndPt.erase(oldKey);
        }
        if (to == table) {
            physicalRowAndPt[{mc.target_player_id(), mc.new_card_id()}] = {mc.y(), oldPt};
        }
    }
    if (ev.HasExtension(Event_SetCardAttr::ext)) {
        const auto &attr = ev.GetExtension(Event_SetCardAttr::ext);
        if (attr.attribute() == AttrPT) {
            physicalRowAndPt[{ev.player_id(), attr.card_id()}].second = QString::fromStdString(attr.attr_value());
        }
        if (attr.attribute() == AttrTapped) {
            if (attr.attr_value() == "1") {
                physicallyTappedCardIds.insert(attr.card_id());
            } else {
                physicallyTappedCardIds.erase(attr.card_id());
            }
        } else if (attr.attribute() == AttrAttacking) {
            if (attr.attr_value() == "1") {
                physicallyAttackingCardIds.insert(attr.card_id());
            } else {
                physicallyAttackingCardIds.erase(attr.card_id());
            }
        } else if (attr.attribute() == AttrAnnotation) {
            annotationByServerCardId[attr.card_id()] = QString::fromStdString(attr.attr_value());
        }
    }
}
void ObservedState::observeRuledEvent(const ruled::v1::RuledEvent &ev, const std::function<void(const QString &)> &log)
{
    if (ev.has_phase_changed()) {
        const ruled::v1::PhaseId newPhase = ev.phase_changed().phase_id();
        if (newPhase != phase) {
            log(QStringLiteral("phase: %1 (active %2)")
                    .arg(QString::fromStdString(ruled::v1::PhaseId_Name(newPhase)))
                    .arg(ev.phase_changed().active_player_id()));
        }
        phase = newPhase;
        activePlayer = ev.phase_changed().active_player_id();
    } else if (ev.has_priority_changed()) {
        priorityPlayer = ev.priority_changed().player_id();
    } else if (ev.has_stack_pushed()) {
        ++stackDepth;
        const auto &sp = ev.stack_pushed();
        const QString cardId = QString::fromStdString(sp.card_id());
        log(QStringLiteral("stack push oid %1 card '%2' targets %3")
                .arg(sp.object_id())
                .arg(cardId)
                .arg(sp.targets_size()));
    } else if (ev.has_stack_resolved()) {
        stackDepth = std::max(0, stackDepth - 1);
    } else if (ev.has_stack_object_countered()) {
        stackDepth = std::max(0, stackDepth - 1);
        counteredStackObjectIds.insert(ev.stack_object_countered().object_id());
    } else if (ev.has_life_changed()) {
        const auto &lc = ev.life_changed();
        lifeByPlayer[lc.player_id()] = lc.new_total();
        log(QStringLiteral("life: player %1 -> %2 (delta %3)").arg(lc.player_id()).arg(lc.new_total()).arg(lc.delta()));
    } else if (ev.has_attackers_added()) {
        latestAddedAttackAssignments.assign(ev.attackers_added().assignments().begin(),
                                            ev.attackers_added().assignments().end());
    } else if (ev.has_attackers_preview()) {
        latestAttackPreviewAssignments.assign(ev.attackers_preview().assignments().begin(),
                                              ev.attackers_preview().assignments().end());
    } else if (ev.has_attackers_declared()) {
        if (ev.attackers_declared().assignments_size() > 0) {
            latestDeclaredAttackAssignments.assign(ev.attackers_declared().assignments().begin(),
                                                   ev.attackers_declared().assignments().end());
            log(QStringLiteral("attackers declared: %1 creature(s)").arg(ev.attackers_declared().assignments_size()));
        }
    } else if (ev.has_battlefield_object_map()) {
        for (const auto &entry : ev.battlefield_object_map().entries()) {
            serverCardByEngineOid[entry.engine_object_id()] = entry.server_card_id();
        }
    } else if (ev.has_graveyard_object_map()) {
        for (const auto &entry : ev.graveyard_object_map().entries()) {
            serverCardByEngineOid[entry.engine_object_id()] = entry.server_card_id();
            graveyardOwnerByEngineOid[entry.engine_object_id()] = entry.player_id();
        }
    } else if (ev.has_exile_object_map()) {
        for (const auto &entry : ev.exile_object_map().entries()) {
            serverCardByEngineOid[entry.engine_object_id()] = entry.server_card_id();
        }
    } else if (ev.has_trigger_needs_target()) {
        const auto &trigger = ev.trigger_needs_target();
        if (trigger.controller_player_id() == myId)
            pendingTriggerTarget = trigger;
    } else if (ev.has_trigger_order_required()) {
        const auto &tor = ev.trigger_order_required();
        if (tor.deciding_player_id() == myId && tor.candidates_size() > 0) {
            pendingTriggerOrder = tor;
            log(QStringLiteral("trigger order required: %1 candidates").arg(tor.candidates_size()));
        }
    } else if (ev.has_resolution_choice_required()) {
        const auto &rcr = ev.resolution_choice_required();
        lastResolutionChoice = rcr;
        if (rcr.deciding_player_id() == myId &&
            (rcr.candidate_object_ids_size() > 0 ||
             (rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH && rcr.min() == 0) ||
             rcr.choice_kind() == ruled::v1::CHOICE_KIND_MANA_PAYMENT ||
             rcr.choice_kind() == ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH ||
             rcr.choice_kind() == ruled::v1::CHOICE_KIND_ATTACKING_TOKEN_DEFENDER)) {
            pendingChoice = rcr;
            log(QStringLiteral("resolution choice: kind %1 min %2 max %3 ordered %4 candidates %5")
                    .arg(QString::fromStdString(ruled::v1::ChoiceKind_Name(rcr.choice_kind())))
                    .arg(rcr.min())
                    .arg(rcr.max())
                    .arg(rcr.ordered())
                    .arg(rcr.candidate_object_ids_size()));
        }
    } else if (ev.has_cards_revealed()) {
        revealEvents.push_back(ev.cards_revealed());
    } else if (ev.has_active_public_reveal_snapshot()) {
        activePublicReveals.assign(ev.active_public_reveal_snapshot().reveals().begin(),
                                   ev.active_public_reveal_snapshot().reveals().end());
    } else if (ev.has_zone_view()) {
        for (const ruled::v1::RuledPerPlayerView &pp : ev.zone_view().per_player()) {
            auto &bf = battlefieldByPlayer[pp.player_id()];
            if (!ev.zone_view().battlefields_unchanged()) {
                bf.clear();
                for (const auto &battlefieldObject : pp.battlefield_objects()) {
                    Permanent perm;
                    perm.cardId = QString::fromStdString(battlefieldObject.card_id());
                    if (perm.cardId.isEmpty() && !battlefieldObject.face_down()) {
                        perm.cardId = QString::fromStdString(battlefieldObject.effective_display_name())
                                          .toLower()
                                          .replace(QLatin1Char(' '), QLatin1Char('_'));
                    }
                    perm.oid = battlefieldObject.object_id();
                    perm.tapped = battlefieldObject.tapped();
                    perm.creature = battlefieldObject.is_creature();
                    perm.planeswalker = battlefieldObject.is_planeswalker();
                    perm.battle = battlefieldObject.is_battle();
                    perm.sick = battlefieldObject.summoning_sick();
                    perm.power = static_cast<int>(battlefieldObject.power());
                    perm.toughness = static_cast<int>(battlefieldObject.toughness());
                    perm.faceIndex = static_cast<int>(battlefieldObject.face_up_index());
                    perm.faceDown = battlefieldObject.face_down();
                    perm.generation = battlefieldObject.zone_change_generation();
                    perm.loyalty =
                        battlefieldObject.is_planeswalker() ? static_cast<int>(battlefieldObject.loyalty()) : -1;
                    perm.defense = battlefieldObject.is_battle() ? static_cast<int>(battlefieldObject.defense()) : -1;
                    perm.countersAnnotation = QString::fromStdString(battlefieldObject.counters_annotation());
                    perm.battleProtector = battlefieldObject.has_battle_protector_player_id()
                                               ? battlefieldObject.battle_protector_player_id()
                                               : -1;
                    perm.firstAbilityActivatable = battlefieldObject.activated_abilities_size() > 0 &&
                                                   battlefieldObject.activated_abilities(0).activatable();
                    for (const auto &ability : battlefieldObject.activated_abilities()) {
                        perm.abilityIndices.push_back(ability.ability_index());
                    }
                    perm.roomDoorCount = std::min(2, battlefieldObject.room_doors_size());
                    for (int door = 0; door < perm.roomDoorCount; ++door) {
                        const auto &publishedDoor = battlefieldObject.room_doors(door);
                        if (publishedDoor.face_index() < perm.roomDoors.size()) {
                            perm.roomDoors[publishedDoor.face_index()] = publishedDoor.unlocked();
                        }
                    }

                    if (battlefieldObject.has_attachment_recipient() &&
                        battlefieldObject.attachment_recipient().recipient_case() ==
                            ruled::v1::AttachmentRecipient::kObjectId) {
                        perm.attachmentObjectId = battlefieldObject.attachment_recipient().object_id();
                    }
                    if (battlefieldObject.has_attachment_recipient() &&
                        battlefieldObject.attachment_recipient().recipient_case() ==
                            ruled::v1::AttachmentRecipient::kPlayerId) {
                        perm.attachmentPlayerId = battlefieldObject.attachment_recipient().player_id();
                    }
                    perm.haste = std::find(battlefieldObject.keywords().begin(), battlefieldObject.keywords().end(),
                                           "Haste") != battlefieldObject.keywords().end();
                    perm.reach = std::find(battlefieldObject.keywords().begin(), battlefieldObject.keywords().end(),
                                           "Reach") != battlefieldObject.keywords().end();
                    perm.flying = std::find(battlefieldObject.keywords().begin(), battlefieldObject.keywords().end(),
                                            "Flying") != battlefieldObject.keywords().end();
                    perm.indestructible =
                        std::find(battlefieldObject.keywords().begin(), battlefieldObject.keywords().end(),
                                  "Indestructible") != battlefieldObject.keywords().end();
                    bf.push_back(perm);
                }
            }
            if (oppId < 0 && myId >= 0 && pp.player_id() != myId) {
                oppId = pp.player_id();
            }
        }
    } else if (ev.has_hand_slot_map()) {
        handServerCardBySlot.clear();
        std::map<int, int> counts;
        for (const auto &entry : ev.hand_slot_map().entries()) {
            ++counts[entry.player_id()];
            if (entry.player_id() == myId) {
                handServerCardBySlot[static_cast<int>(entry.hand_index())] = entry.server_card_id();
            }
        }
        for (const auto &kv : counts) {
            handSizeByPlayer[kv.first] = kv.second;
            if (oppId < 0 && myId >= 0 && kv.first != myId) {
                oppId = kv.first;
            }
        }
    } else if (ev.has_mana_pool_updated()) {
        const auto &mp = ev.mana_pool_updated();
        int restrictedBlue = 0;
        for (const auto &group : mp.restricted_groups()) {
            restrictedBlue += static_cast<int>(group.u());
        }
        restrictedBlueByPlayer[mp.player_id()] = restrictedBlue;
        if (mp.player_id() == myId) {
            myPool.w = mp.w();
            myPool.u = mp.u();
            myPool.b = mp.b();
            myPool.r = mp.r();
            myPool.g = mp.g();
            myPool.c = mp.c();
        }
    } else if (ev.has_log()) {
        const QString text = QString::fromStdString(ev.log().text());
        log(QStringLiteral("gamelog: %1").arg(text.left(160)));
    }
}
bool ObservedState::observeLegalActions(const ruled::v1::RuledEventBatch &batch)
{
    const auto it = batch.legal_by_player().find(myId);
    if (it == batch.legal_by_player().end())
        return false;
    labels.clear();
    for (const auto &label : it->second.labels())
        labels.append(QString::fromStdString(label));
    latestLegal = it->second;
    return true;
}
} // namespace ruled_e2e
