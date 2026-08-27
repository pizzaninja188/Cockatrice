// Fork-owned. See ruled_broadcast_router.h.

#include "ruled_broadcast_router.h"

#include "ruled_batch_synchronizer.h"
#include "ruled_utils.h"
#include "server_abstract_player.h"
#include "server_card.h"
#include "server_cardzone.h"
#include "server_game.h"
#include "server_player.h"

#include <QHash>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/utility/zone_names.h>

RuledBroadcastRouter::RuledBroadcastRouter(Server_Game *_game, RuledBatchSynchronizer *_synchronizer)
    : game(_game), synchronizer(_synchronizer)
{
}

void RuledBroadcastRouter::resetForNewGame()
{
    lastBroadcastHandSlotMap.Clear();
    hasLastBroadcastHandSlotMap = false;
    lastBroadcastHandSlotParticipants.clear();
    pendingResolutionChoice.reset();
}

void RuledBroadcastRouter::broadcast(const ruled::v1::IpcResponse &resp, bool authoritative)
{
    if (!resp.has_batch()) {
        return;
    }
    if (authoritative) {
        updatePendingResolutionChoiceCache(resp);
    }
    ruled::v1::IpcResponse toSend;
    toSend.set_ok(resp.ok());
    toSend.set_error(resp.error());
    toSend.mutable_batch()->CopyFrom(resp.batch());
    appendServerObjectMaps(toSend);
    const ruled::v1::RuledEventBatch &batch = toSend.batch();
    for (auto *participant : game->getParticipants()) {
        GameEventStorage ges;
        const ruled::v1::RuledEventBatch filtered = redactBatchForParticipant(batch, participant);
        Event_RuledPayload ev;
        std::string bytes;
        filtered.SerializeToString(&bytes);
        ev.set_payload(bytes);
        ges.enqueueGameEvent(ev, -1, GameEventStorageItem::SendToPrivate, participant->getPlayerId());
        ges.sendToGame(game);
    }
}

void RuledBroadcastRouter::updatePendingResolutionChoiceCache(const ruled::v1::IpcResponse &response)
{
    pendingResolutionChoice.reset();
    if (!response.has_batch()) {
        return;
    }
    for (const auto &event : response.batch().events()) {
        if (!event.has_resolution_choice_required()) {
            continue;
        }
        const auto &choice = event.resolution_choice_required();
        pendingResolutionChoice.emplace();
        pendingResolutionChoice->CopyFrom(choice);
    }
}

void RuledBroadcastRouter::enqueuePendingResolutionChoiceForParticipant(Server_AbstractParticipant *participant,
                                                                        ResponseContainer &rc)
{
    if (!participant || !pendingResolutionChoice.has_value()) {
        return;
    }
    ruled::v1::RuledEventBatch snapshot;
    snapshot.add_events()->mutable_resolution_choice_required()->CopyFrom(*pendingResolutionChoice);
    const ruled::v1::RuledEventBatch filtered = redactBatchForParticipant(snapshot, participant);

    Event_RuledPayload event;
    std::string bytes;
    filtered.SerializeToString(&bytes);
    event.set_payload(bytes);
    rc.enqueuePostResponseItem(ServerMessage::GAME_EVENT_CONTAINER, game->prepareGameEvent(event, -1));
}

// Appends the server-built identity-map events to the outgoing batch: a
// BattlefieldObjectMap so clients can map their visible CardItem (Server_Card.id)
// back to the engine ObjectId that DeclareAttackers / DeclareBlockers expects, a
// HandSlotMap (zone_view hand/lib fields are cleared before broadcast), and a
// GraveyardObjectMap for graveyard spell targets. Rebuilt every batch from the latest sync.
void RuledBroadcastRouter::appendServerObjectMaps(ruled::v1::IpcResponse &toSend)
{
    {
        ruled::v1::RuledEvent mapEvent;
        auto *map = mapEvent.mutable_battlefield_object_map();
        for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
            if (!ab) {
                continue;
            }
            auto *pl = static_cast<Server_Player *>(ab);
            const RuledPlayerBinding &binding = synchronizer->playerBinding(pl->getPlayerId());
            const QHash<quint32, int> oidMap = binding.engineOidToServerCardId;
            Server_CardZone *tableZone = pl->getZones().value(ZoneNames::TABLE);
            int ordinal = 0;
            // Iterate the table zone in current order so `ordinal` matches the
            // controller-order seen in RuledPerPlayerView.battlefield.
            if (tableZone) {
                for (Server_Card *card : tableZone->getCards()) {
                    if (!card) {
                        continue;
                    }
                    QString tr = synchronizer->cardIdForName(card->getName());
                    quint32 engineOid = 0;
                    bool found = false;
                    for (auto it = oidMap.constBegin(); it != oidMap.constEnd(); ++it) {
                        if (it.value() == card->getId()) {
                            engineOid = it.key();
                            found = true;
                            break;
                        }
                    }
                    if (!found) {
                        ++ordinal;
                        continue;
                    }
                    auto *entry = map->add_entries();
                    entry->set_player_id(pl->getPlayerId());
                    entry->set_engine_object_id(engineOid);
                    entry->set_card_id(tr.toStdString());
                    entry->set_ordinal(static_cast<uint32_t>(ordinal));
                    entry->set_server_card_id(card->getId());
                    entry->set_summoning_sick(binding.isEngineOidSummoningSick(engineOid));
                    if (binding.isEngineOidHaste(engineOid)) {
                        entry->add_keywords("Haste");
                    }
                    if (binding.isEngineOidTrample(engineOid)) {
                        entry->add_keywords("Trample");
                    }
                    entry->set_is_creature(binding.isEngineOidCreature(engineOid));
                    ++ordinal;
                }
            }
            Server_CardZone *stackZone = pl->getZones().value(ZoneNames::STACK);
            if (stackZone) {
                int stackOrdinal = 0;
                for (Server_Card *stackCard : stackZone->getCards()) {
                    if (!stackCard) {
                        continue;
                    }
                    quint32 stackOid = 0;
                    bool foundStackOid = false;
                    for (auto it = synchronizer->ruledStackObjectIdToServerCardId.constBegin();
                         it != synchronizer->ruledStackObjectIdToServerCardId.constEnd(); ++it) {
                        if (it.value() == stackCard->getId()) {
                            stackOid = it.key();
                            foundStackOid = true;
                            break;
                        }
                    }
                    if (!foundStackOid) {
                        ++stackOrdinal;
                        continue;
                    }
                    QString tr = synchronizer->cardIdForName(stackCard->getName());
                    auto *entry = map->add_entries();
                    entry->set_player_id(pl->getPlayerId());
                    entry->set_engine_object_id(stackOid);
                    entry->set_card_id(tr.toStdString());
                    entry->set_ordinal(static_cast<uint32_t>(stackOrdinal));
                    entry->set_server_card_id(stackCard->getId());
                    entry->set_summoning_sick(false);
                    ++stackOrdinal;
                }
            }
        }
        // Only inject when we have something useful so trivial batches stay small.
        if (map->entries_size() > 0) {
            *toSend.mutable_batch()->add_events() = mapEvent;
        }
    }
    // Controller-private identity for face-down battlefield objects. Always include the complete
    // replacement, even when empty, so leave-zone and control-change batches prune stale lookup.
    {
        ruled::v1::RuledEvent faceDownEvent;
        auto *faceDownMap = faceDownEvent.mutable_face_down_object_map();
        for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
            if (!ab) {
                continue;
            }
            auto *player = static_cast<Server_Player *>(ab);
            const int playerId = player->getPlayerId();
            const RuledPlayerBinding &binding = synchronizer->playerBinding(playerId);
            for (auto it = binding.engineOidToServerCardId.constBegin();
                 it != binding.engineOidToServerCardId.constEnd(); ++it) {
                const quint32 oid = it.key();
                if (!binding.isEngineOidFaceDown(oid)) {
                    continue;
                }
                Server_Card *card = binding.findCardByEngineOid(player, oid);
                if (!card || !card->getZone() || card->getZone()->getName() != ZoneNames::TABLE) {
                    continue;
                }
                auto *entry = faceDownMap->add_entries();
                entry->set_controller_player_id(playerId);
                entry->set_engine_object_id(oid);
                entry->set_zone_change_generation(binding.engineOidZoneChangeGeneration(oid));
                entry->set_server_card_id(card->getId());
                const QString cardId = binding.engineOidUnderlyingCardId(oid);
                const QString cardName = synchronizer->faceDisplayName(cardId, 0);
                entry->set_card_name((cardName.isEmpty() ? card->getName() : cardName).toStdString());
            }
        }
        *toSend.mutable_batch()->add_events() = faceDownEvent;
    }
    // zone_view hand/lib fields are cleared before broadcast; publish hand index <-> Server_Card.id separately for
    // ruled UI intents. Injected only when the mapping actually changed since the last broadcast:
    // the client keeps the previous map when the event is absent and replaces it wholesale when it
    // is present, so re-sending an identical map on every priority pass and mana tap is pure waste.
    {
        ruled::v1::RuledEvent handEv;
        auto *hm = handEv.mutable_hand_slot_map();
        // Player order (QMap by id) and hand index order are both deterministic, which is what lets
        // the change check below compare serialized bytes instead of diffing entries.
        for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
            if (!ab) {
                continue;
            }
            auto *pl = static_cast<Server_Player *>(ab);
            Server_CardZone *handZone = pl->getZones().value(ZoneNames::HAND);
            if (!handZone) {
                continue;
            }
            const int pid = pl->getPlayerId();
            for (int i = 0; i < handZone->getCards().size(); ++i) {
                Server_Card *c = handZone->getCards().at(i);
                if (!c) {
                    continue;
                }
                auto *ent = hm->add_entries();
                ent->set_player_id(pid);
                ent->set_hand_index(static_cast<uint32_t>(i));
                ent->set_server_card_id(c->getId());
            }
        }
        // A joiner or reconnector starts with an empty client-side map, so a changed participant
        // set forces a re-send even when no hand moved.
        QSet<int> participantIds;
        for (auto it = game->getParticipants().constBegin(); it != game->getParticipants().constEnd(); ++it) {
            participantIds.insert(it.key());
        }
        const bool participantsChanged = participantIds != lastBroadcastHandSlotParticipants;
        const bool mapChanged =
            !hasLastBroadcastHandSlotMap || hm->SerializeAsString() != lastBroadcastHandSlotMap.SerializeAsString();
        if (participantsChanged || mapChanged) {
            lastBroadcastHandSlotMap.CopyFrom(*hm);
            hasLastBroadcastHandSlotMap = true;
            lastBroadcastHandSlotParticipants = participantIds;
            *toSend.mutable_batch()->add_events() = handEv;
        }
    }
    // Graveyard OID map: lets the client map engine OIDs in valid_graveyard_ids to
    // server card ids so graveyard cards can be clicked as spell targets.
    {
        ruled::v1::RuledEvent graveyardEv;
        auto *gm = graveyardEv.mutable_graveyard_object_map();
        for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
            if (!ab) {
                continue;
            }
            auto *pl = static_cast<Server_Player *>(ab);
            const QHash<quint32, int> gravOidMap =
                synchronizer->playerBinding(pl->getPlayerId()).graveyardEngineOidToServerCardId;
            for (auto it = gravOidMap.constBegin(); it != gravOidMap.constEnd(); ++it) {
                auto *entry = gm->add_entries();
                entry->set_player_id(pl->getPlayerId());
                entry->set_engine_object_id(it.key());
                entry->set_server_card_id(it.value());
            }
        }
        // This is a full replacement, including when empty. Omitting the event after the last
        // graveyard card leaves would retain the client's old Server_Card.id -> engine OID binding;
        // that stale identity can later make a repeated public-zone cast appear on the wrong pile.
        *toSend.mutable_batch()->add_events() = graveyardEv;
    }
    // Exile OID map: Adventure legal actions name an engine object, while clicks carry a
    // Server_Card id. Publish the binding for every public exile pile.
    {
        ruled::v1::RuledEvent exileEv;
        auto *map = exileEv.mutable_exile_object_map();
        for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
            if (!ab) {
                continue;
            }
            auto *player = static_cast<Server_Player *>(ab);
            const auto &oidMap = synchronizer->playerBinding(player->getPlayerId()).exileEngineOidToServerCardId;
            for (auto it = oidMap.constBegin(); it != oidMap.constEnd(); ++it) {
                auto *entry = map->add_entries();
                entry->set_player_id(player->getPlayerId());
                entry->set_engine_object_id(it.key());
                entry->set_server_card_id(it.value());
            }
        }
        // An empty map is meaningful: it clears client-side identity after the last exiled card
        // leaves. Unlike an omitted retained snapshot, this server-built map is a full replacement.
        *toSend.mutable_batch()->add_events() = exileEv;
    }
}

// Per-participant hidden-info redaction: keeps only the participant's own legal actions,
// drops LogMessage events not meant for them, and redacts/augments tier-3 resolution
// choice candidates by choice kind.
ruled::v1::RuledEventBatch RuledBroadcastRouter::redactBatchForParticipant(const ruled::v1::RuledEventBatch &batch,
                                                                           Server_AbstractParticipant *participant)
{
    ruled::v1::RuledEventBatch filtered;
    filtered.CopyFrom(batch);
    filtered.clear_legal_by_player();
    const auto it = batch.legal_by_player().find(participant->getPlayerId());
    if (it != batch.legal_by_player().end()) {
        (*filtered.mutable_legal_by_player())[participant->getPlayerId()] = it->second;
    }
    // Drop LogMessage events directed at a different player or explicitly hidden from this one.
    for (int ei = filtered.events_size() - 1; ei >= 0; --ei) {
        const auto &ev = filtered.events(ei);
        if (!ev.has_log()) {
            continue;
        }
        const auto &log = ev.log();
        if (log.has_visible_to_player_id() && log.visible_to_player_id() != participant->getPlayerId()) {
            filtered.mutable_events()->DeleteSubrange(ei, 1);
        } else if (log.has_hidden_from_player_id() && log.hidden_from_player_id() == participant->getPlayerId()) {
            filtered.mutable_events()->DeleteSubrange(ei, 1);
        }
    }
    {
        // Redact private candidates of a tier-3 resolution choice (CR 608) from everyone but the
        // deciding player, unless the engine explicitly authorizes a public reveal. Private kinds
        // expose a concealed zone (see isPrivateChoiceKind):
        // HAND_CARDS reveals a player's hand, LIBRARY_SEARCH their library, LIBRARY_TOP the top of
        // their library, MANIFEST_DREAD the top two, OPPONENT_HAND another player's hand, so only
        // the decider sees the candidate object ids / names by default. A choice carrying
        // ALL_PARTICIPANTS publishes those identities to every recipient, but the prompt and
        // eligibility mask remain exclusive to the decider.
        // For HAND_CARDS, inject candidate_server_card_ids for the deciding player
        // so the client can map engine OIDs to physical hand CardItems for the hand-click UI.
        // For library-image choices, inject by name-matching from the decider's deck zone
        // so the client can open the deck zone view and use deck-card click-to-pick (like Gifts Ungiven
        // search step). For REVEALED, inject from the non-deciding player's deck
        // so the client can render the revealed cards in a zone popup for the opponent's pick step.
        for (int ei = 0; ei < filtered.events_size(); ++ei) {
            if (!filtered.events(ei).has_resolution_choice_required()) {
                continue;
            }
            auto *rcr = filtered.mutable_events(ei)->mutable_resolution_choice_required();
            const bool requiresSourceZones = rcr->choice_kind() == ruled::v1::CHOICE_KIND_ZONE_SEARCH ||
                                             rcr->choice_kind() == ruled::v1::CHOICE_KIND_GRAVEYARD_CARDS;
            if (requiresSourceZones && (rcr->candidate_source_zones_size() != rcr->candidate_object_ids_size() ||
                                        rcr->candidate_card_ids_size() != rcr->candidate_object_ids_size() ||
                                        rcr->candidate_names_size() != rcr->candidate_object_ids_size())) {
                rcr->clear_candidate_object_ids();
                rcr->clear_candidate_card_ids();
                rcr->clear_candidate_names();
                rcr->clear_candidate_server_card_ids();
                rcr->clear_candidate_selectable();
                rcr->clear_candidate_source_zones();
                rcr->set_prompt_text("Resolution choice metadata is unavailable.");
            }
            const bool isDecider = rcr->deciding_player_id() == participant->getPlayerId();
            const bool isPublicReveal =
                rcr->reveal_audience() == ruled::v1::RESOLUTION_REVEAL_AUDIENCE_ALL_PARTICIPANTS;
            if (isPrivateChoiceKind(rcr->choice_kind()) && !isDecider && !isPublicReveal) {
                rcr->clear_candidate_object_ids();
                rcr->clear_candidate_card_ids();
                rcr->clear_candidate_names();
                rcr->clear_candidate_server_card_ids();
                rcr->clear_candidate_selectable();
                rcr->clear_candidate_source_zones();
                rcr->set_prompt_text("Opponent is making a resolution choice.");
            } else {
                if (isPublicReveal && !isDecider) {
                    rcr->clear_candidate_selectable();
                    rcr->set_prompt_text("Opponent is making a resolution choice.");
                }
                if (rcr->choice_kind() == ruled::v1::CHOICE_KIND_HAND_CARDS ||
                    rcr->choice_kind() == ruled::v1::CHOICE_KIND_COST_OBJECTS) {
                    // HandCards and CostObjects: populate physical ids so client card-click UI can
                    // match engine OIDs without inferring candidates from the visible battlefield.
                    const int deciderId = rcr->deciding_player_id();
                    auto *deciderPlayer = static_cast<Server_Player *>(game->getPlayers().value(deciderId));
                    if (deciderPlayer) {
                        for (int ci = 0; ci < rcr->candidate_object_ids_size(); ++ci) {
                            const quint32 oid = static_cast<quint32>(rcr->candidate_object_ids(ci));
                            Server_Card *sc =
                                synchronizer->playerBinding(deciderId).findCardByEngineOid(deciderPlayer, oid);
                            rcr->add_candidate_server_card_ids(sc ? sc->getId() : -1);
                        }
                    }
                } else if (rcr->choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH ||
                           rcr->choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_TOP ||
                           rcr->choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_LOOK ||
                           rcr->choice_kind() == ruled::v1::CHOICE_KIND_MANIFEST_DREAD ||
                           rcr->choice_kind() == ruled::v1::CHOICE_KIND_ZONE_SEARCH ||
                           rcr->choice_kind() == ruled::v1::CHOICE_KIND_GRAVEYARD_CARDS) {
                    // LibrarySearch / LibraryTop / LibraryLook / ManifestDread: assign each candidate a sequential
                    // index as its server card ID.
                    // Deck cards are not in engineOidToServerCardId (only battlefield/hand/stack are),
                    // so there is no server-side lookup available. Sequential indices give every
                    // physical card (including duplicate-named ones) a unique client-side ID.
                    // The client maps index i → engine OID via candidate_object_ids[i].
                    // NB: unique *within this candidate list only* — 0, 1, 2 … collide head-on with
                    // the real Server_Card ids of cards in hand and on the battlefield. Any client
                    // lookup keyed on these must first confirm the card is in the pick's own zone
                    // (RuledActions::isResolutionPickZoneCard).
                    for (int ci = 0; ci < rcr->candidate_names_size(); ++ci) {
                        rcr->add_candidate_server_card_ids(ci);
                    }
                } else if (rcr->choice_kind() == ruled::v1::CHOICE_KIND_REVEALED &&
                           rcr->candidate_server_card_ids_size() == 0) {
                    // RevealedCards: same sequential-index scheme for the same reason.
                    for (int ci = 0; ci < rcr->candidate_names_size(); ++ci) {
                        rcr->add_candidate_server_card_ids(ci);
                    }
                } else if (rcr->choice_kind() == ruled::v1::CHOICE_KIND_OPPONENT_HAND &&
                           rcr->candidate_server_card_ids_size() == 0) {
                    // OpponentHand candidates live in another player's hidden hand. Public reveal
                    // recipients receive only transient sequential popup ids; the persistent
                    // HandSlotMap and the hand's real Server_Card ids remain private.
                    for (int ci = 0; ci < rcr->candidate_names_size(); ++ci) {
                        rcr->add_candidate_server_card_ids(ci);
                    }
                }
            }
        }
    }

    // HandSlotMap is recipient-private: retain only the recipient's physical hand ids.
    for (int ei = 0; ei < filtered.events_size(); ++ei) {
        if (filtered.events(ei).has_hand_slot_map()) {
            auto *entries = filtered.mutable_events(ei)->mutable_hand_slot_map()->mutable_entries();
            for (int i = entries->size() - 1; i >= 0; --i) {
                if (entries->Get(i).player_id() != participant->getPlayerId()) {
                    entries->DeleteSubrange(i, 1);
                }
            }
        }
        if (filtered.events(ei).has_face_down_object_map()) {
            auto *entries = filtered.mutable_events(ei)->mutable_face_down_object_map()->mutable_entries();
            for (int i = entries->size() - 1; i >= 0; --i) {
                if (entries->Get(i).controller_player_id() != participant->getPlayerId()) {
                    entries->DeleteSubrange(i, 1);
                }
            }
        }
    }

    // Capture the explicitly authorized per-player values, clear every PER_PLAYER field
    // recursively by reflection (future classified fields therefore fail closed), then restore
    // only these reviewed cases.
    bool hasOwnLegalActions = false;
    ruled::v1::LegalActions ownLegalActions;
    const auto ownLegalIt = filtered.legal_by_player().find(participant->getPlayerId());
    if (ownLegalIt != filtered.legal_by_player().end()) {
        ownLegalActions.CopyFrom(ownLegalIt->second);
        hasOwnLegalActions = true;
    }
    QHash<int, QString> routedLogText;
    QHash<int, ruled::v1::ResolutionChoiceRequired> routedChoices;
    QHash<int, ruled::v1::TriggerNeedsTarget> routedTriggerChoices;
    QHash<int, ruled::v1::HandSlotMap> ownHandSlotMaps;
    QHash<int, ruled::v1::FaceDownObjectMap> ownFaceDownMaps;
    for (int ei = 0; ei < filtered.events_size(); ++ei) {
        const auto &event = filtered.events(ei);
        if (event.has_log()) {
            routedLogText.insert(ei, QString::fromStdString(event.log().text()));
        } else if (event.has_resolution_choice_required()) {
            routedChoices.insert(ei, event.resolution_choice_required());
        } else if (event.has_trigger_needs_target() &&
                   event.trigger_needs_target().controller_player_id() == participant->getPlayerId()) {
            routedTriggerChoices.insert(ei, event.trigger_needs_target());
        } else if (event.has_hand_slot_map()) {
            ownHandSlotMaps.insert(ei, event.hand_slot_map());
        } else if (event.has_face_down_object_map()) {
            ownFaceDownMaps.insert(ei, event.face_down_object_map());
        }
    }

    clearRuledFieldsByVisibility(&filtered, ruled::v1::FIELD_VISIBILITY_PER_PLAYER);
    if (hasOwnLegalActions) {
        (*filtered.mutable_legal_by_player())[participant->getPlayerId()] = ownLegalActions;
    }
    for (auto logIt = routedLogText.constBegin(); logIt != routedLogText.constEnd(); ++logIt) {
        filtered.mutable_events(logIt.key())->mutable_log()->set_text(logIt.value().toStdString());
    }
    for (auto choiceIt = routedChoices.constBegin(); choiceIt != routedChoices.constEnd(); ++choiceIt) {
        auto *choice = filtered.mutable_events(choiceIt.key())->mutable_resolution_choice_required();
        choice->set_prompt_text(choiceIt.value().prompt_text());
        choice->mutable_candidate_object_ids()->CopyFrom(choiceIt.value().candidate_object_ids());
        choice->mutable_candidate_card_ids()->CopyFrom(choiceIt.value().candidate_card_ids());
        choice->mutable_candidate_names()->CopyFrom(choiceIt.value().candidate_names());
        choice->mutable_candidate_server_card_ids()->CopyFrom(choiceIt.value().candidate_server_card_ids());
        choice->mutable_candidate_selectable()->CopyFrom(choiceIt.value().candidate_selectable());
        choice->mutable_candidate_source_zones()->CopyFrom(choiceIt.value().candidate_source_zones());
        if (choiceIt.value().deciding_player_id() == participant->getPlayerId()) {
            choice->mutable_resolution_branches()->CopyFrom(choiceIt.value().resolution_branches());
            choice->mutable_combat_defender_options()->CopyFrom(choiceIt.value().combat_defender_options());
        }
    }
    for (auto triggerIt = routedTriggerChoices.constBegin(); triggerIt != routedTriggerChoices.constEnd();
         ++triggerIt) {
        auto *trigger = filtered.mutable_events(triggerIt.key())->mutable_trigger_needs_target();
        trigger->mutable_targets()->CopyFrom(triggerIt.value().targets());
        trigger->mutable_modes()->CopyFrom(triggerIt.value().modes());
    }
    for (auto handMapIt = ownHandSlotMaps.constBegin(); handMapIt != ownHandSlotMaps.constEnd(); ++handMapIt) {
        filtered.mutable_events(handMapIt.key())->mutable_hand_slot_map()->CopyFrom(handMapIt.value());
    }
    for (auto faceDownIt = ownFaceDownMaps.constBegin(); faceDownIt != ownFaceDownMaps.constEnd(); ++faceDownIt) {
        filtered.mutable_events(faceDownIt.key())->mutable_face_down_object_map()->CopyFrom(faceDownIt.value());
    }

    clearRuledFieldsByVisibility(&filtered, ruled::v1::FIELD_VISIBILITY_SERVER_ONLY);
    for (int ei = filtered.events_size() - 1; ei >= 0; --ei) {
        if (filtered.events(ei).ev_case() == ruled::v1::RuledEvent::EV_NOT_SET) {
            filtered.mutable_events()->DeleteSubrange(ei, 1);
        }
    }
    return filtered;
}
