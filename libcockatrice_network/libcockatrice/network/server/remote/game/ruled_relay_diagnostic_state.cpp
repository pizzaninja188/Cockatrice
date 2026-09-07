#include "ruled_batch_synchronizer.h"

#include <QJsonArray>
#include <QJsonObject>
#include <algorithm>
#include <libcockatrice/protocol/ruled_diagnostics.h>

QJsonObject RuledBatchSynchronizer::diagnosticSnapshot() const
{
    QJsonArray players, stack, catalog, pending;
    auto playerIds = playerBindings.keys();
    std::sort(playerIds.begin(), playerIds.end());
    for (int playerId : playerIds) {
        const auto &binding = playerBindings[playerId];
        const auto identities = [&](const QHash<quint32, int> &mapping) {
            QJsonArray rows;
            auto ids = mapping.keys();
            std::sort(ids.begin(), ids.end());
            for (const auto oid : ids) {
                const auto cardId = binding.engineOidToUnderlyingCardId.value(oid);
                rows.append(
                    QJsonObject{{"engine_object_id", qint64(oid)},
                                {"server_card_id", mapping.value(oid)},
                                {"zone_change_generation",
                                 binding.engineOidToZoneChangeGeneration.contains(oid)
                                     ? QJsonValue(QString::number(binding.engineOidToZoneChangeGeneration.value(oid)))
                                     : QJsonValue()},
                                {"card_definition_id", cardId.isEmpty() ? QJsonValue() : QJsonValue(cardId)},
                                {"known_name", cardId.isEmpty() ? QJsonValue() : QJsonValue(cardNameForId(cardId))},
                                {"face_down", binding.engineOidToFaceDown.value(oid)},
                                {"summoning_sick", binding.engineOidToSummoningSick.value(oid)},
                                {"haste", binding.engineOidToHaste.value(oid)},
                                {"creature", binding.engineOidToCreature.value(oid)},
                                {"trample", binding.engineOidToTrample.value(oid)}});
            }
            return rows;
        };
        QJsonArray hand, grave, emblems;
        auto markerIds = binding.staticEmblemServerCardIds.keys();
        std::sort(markerIds.begin(), markerIds.end());
        for (auto markerId : markerIds) {
            emblems.append(QJsonObject{{"runtime_marker_id", qint64(markerId)},
                                       {"server_card_id", binding.staticEmblemServerCardIds.value(markerId)}});
        }
        for (auto oid : binding.handEngineOidsInOrder)
            hand.append(qint64(oid));
        for (auto oid : binding.graveyardEngineOidsOldestFirst)
            grave.append(qint64(oid));
        players.append(QJsonObject{{"player_id", playerId},
                                   {"public_and_hand_identities", identities(binding.engineOidToServerCardId)},
                                   {"library_identities", identities(binding.libraryEngineOidToServerCardId)},
                                   {"graveyard_identities", identities(binding.graveyardEngineOidToServerCardId)},
                                   {"exile_identities", identities(binding.exileEngineOidToServerCardId)},
                                   {"static_emblem_identities", emblems},
                                   {"hand_engine_oids_in_order", hand},
                                   {"graveyard_engine_oids_oldest_first", grave},
                                   {"private_zones_synced", binding.privateZonesSynced},
                                   {"battlefield_synced", binding.battlefieldSynced},
                                   {"enduring_story_server_card_id", binding.enduringStoryServerCardId}});
    }
    auto stackIds = ruledStackObjectIdToServerCardId.keys();
    std::sort(stackIds.begin(), stackIds.end());
    for (auto oid : stackIds) {
        QJsonArray targets;
        for (auto target : ruledStackTargetsByObjectId.value(oid))
            targets.append(qint64(target));
        stack.append(QJsonObject{{"engine_object_id", qint64(oid)},
                                 {"server_card_id", ruledStackObjectIdToServerCardId.value(oid)},
                                 {"caster_player_id", ruledStackObjectIdToCasterPlayerId.value(oid, -1)},
                                 {"targets", targets},
                                 {"description", ruledEngineStackPushDescriptionsByObjectId.value(oid)},
                                 {"is_copy", ruledStackCopyObjectIds.contains(oid)}});
    }
    auto cardIds = ruledCardCatalogById.keys();
    std::sort(cardIds.begin(), cardIds.end());
    for (const auto &cardId : cardIds)
        catalog.append(RuledDiagnostics::decode(ruledCardCatalogById[cardId]));
    for (const auto &visual : ruledPendingCastVisualQueue) {
        QJsonArray targets;
        for (auto target : visual.targetOids)
            targets.append(qint64(target));
        pending.append(QJsonObject{{"known_name", visual.cardName},
                                   {"server_card_id", visual.serverCardId},
                                   {"caster_player_id", visual.casterPlayerId},
                                   {"targets", targets}});
    }
    return {{"format_version", 1},
            {"privacy", "server_only"},
            {"priority_player_id", ruledPriorityPlayer},
            {"players", players},
            {"stack", stack},
            {"card_catalog", catalog},
            {"pending_cast_visuals", pending}};
}
