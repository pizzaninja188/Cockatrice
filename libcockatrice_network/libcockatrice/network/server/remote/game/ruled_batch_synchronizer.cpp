// Fork-owned. See ruled_batch_synchronizer.h.

#include "ruled_batch_synchronizer.h"

#include "ruled_game_session.h"
#include "ruled_utils.h"
#include "server_abstract_player.h"
#include "server_card.h"
#include "server_cardzone.h"
#include "server_counter.h"
#include "server_game.h"
#include "server_player.h"

#include <QDebug>
#include <QSet>
#include <algorithm>
#include <libcockatrice/protocol/pb/command_move_card.pb.h>
#include <libcockatrice/protocol/pb/event_attach_card.pb.h>
#include <libcockatrice/protocol/pb/event_flip_card.pb.h>
#include <libcockatrice/protocol/pb/event_game_say.pb.h>
#include <libcockatrice/protocol/pb/event_set_card_attr.pb.h>
#include <libcockatrice/protocol/pb/event_set_counter.pb.h>
#include <libcockatrice/utility/ruled_debug.h>
#include <libcockatrice/utility/zone_names.h>

namespace
{

QString normalizeRuledCardName(const QString &name)
{
    return name.trimmed().toLower().replace(QLatin1Char('_'), QLatin1Char(' '));
}

QHash<quint32, int> authoritativeBattlefieldGridRows(const ruled::v1::RuledEventBatch &batch)
{
    QHash<quint32, int> rows;
    for (const auto &event : batch.events()) {
        if (!event.has_zone_view() || event.zone_view().battlefields_unchanged()) {
            continue;
        }
        for (const auto &player : event.zone_view().per_player()) {
            for (const auto &object : player.battlefield_objects()) {
                rows.insert(static_cast<quint32>(object.object_id()),
                            ruledBattlefieldGridY(object.is_creature(), object.is_land()));
            }
        }
    }
    return rows;
}

QHash<quint32, int> authoritativeBattlefieldDisplayPlayers(const ruled::v1::RuledEventBatch &batch)
{
    QHash<quint32, int> players;
    for (const auto &event : batch.events()) {
        if (!event.has_zone_view() || event.zone_view().battlefields_unchanged()) {
            continue;
        }
        for (const auto &view : event.zone_view().per_player()) {
            for (const auto &object : view.battlefield_objects()) {
                const int displayPlayer =
                    object.has_battle_protector_player_id() ? object.battle_protector_player_id() : view.player_id();
                players.insert(static_cast<quint32>(object.object_id()), displayPlayer);
            }
        }
    }
    return players;
}

// The engine's per-player battlefield is its authoritative CR 110.2 control index. Cockatrice's
// table layout is also a multiplayer spatial affordance, so a Battle is rendered on the table of
// the opponent chosen to protect it (CR 310.11a). This relay-only projection moves no rules state:
// every object retains its public controller/owner/protector fields and clients receive the
// unmodified engine ZoneViewSync.
ruled::v1::ZoneViewSync physicalBattlefieldZoneView(const ruled::v1::ZoneViewSync &engineView)
{
    ruled::v1::ZoneViewSync physicalView = engineView;
    for (auto &view : *physicalView.mutable_per_player()) {
        view.clear_battlefield_objects();
    }
    for (const auto &sourceView : engineView.per_player()) {
        for (const auto &object : sourceView.battlefield_objects()) {
            const int targetPlayer =
                object.has_battle_protector_player_id() ? object.battle_protector_player_id() : sourceView.player_id();
            ruled::v1::RuledPerPlayerView *targetView = nullptr;
            for (auto &candidate : *physicalView.mutable_per_player()) {
                if (candidate.player_id() == targetPlayer) {
                    targetView = &candidate;
                    break;
                }
            }
            if (!targetView) {
                for (auto &candidate : *physicalView.mutable_per_player()) {
                    if (candidate.player_id() == sourceView.player_id()) {
                        targetView = &candidate;
                        break;
                    }
                }
            }
            if (targetView) {
                *targetView->add_battlefield_objects() = object;
            }
        }
    }
    return physicalView;
}

QString withoutRuledCopyAnnotation(const QString &annotation)
{
    QStringList kept;
    for (const QString &line : annotation.split(QLatin1Char('\n'))) {
        if (!line.trimmed().startsWith(QStringLiteral("Copy: "))) {
            kept.append(line);
        }
    }
    return kept.join(QLatin1Char('\n')).trimmed();
}

QString withoutRuledEnchantingAnnotation(const QString &annotation)
{
    QStringList kept;
    for (const QString &line : annotation.split(QLatin1Char('\n'))) {
        if (!line.trimmed().startsWith(QStringLiteral("Enchanting: "))) {
            kept.append(line);
        }
    }
    return kept.join(QLatin1Char('\n')).trimmed();
}

Server_CardZone *ruledCanonicalStackZone(Server_Game *game)
{
    if (!game) {
        return nullptr;
    }
    Server_AbstractPlayer *canonicalStackOwner = nullptr;
    for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
        if (!ab) {
            continue;
        }
        if (!canonicalStackOwner || ab->getPlayerId() < canonicalStackOwner->getPlayerId()) {
            canonicalStackOwner = ab;
        }
    }
    return canonicalStackOwner ? canonicalStackOwner->getZones().value(ZoneNames::STACK) : nullptr;
}

/// Mirror a rules-engine decision onto the physical Cockatrice zones, reporting a refusal.
///
/// The engine has already advanced its own state by the time the relay mirrors it, so a refused
/// move leaves the rules and the presentation permanently disagreeing — and the stack, unlike the
/// battlefield and hand, has no zone-view reconciliation to heal it. The only symptom is a card
/// that visually never arrived, which is indistinguishable from a rendering bug from the outside.
/// Every one of these must be loud: this exact silence is what made a rejected flashback move look
/// like an empty stack window for three rounds of debugging.
///
/// Returns true when the move was applied.
bool ruledApplyMove(Server_AbstractPlayer *mover,
                    GameEventStorage &ges,
                    Server_CardZone *from,
                    Server_CardZone *to,
                    const CardToMove &cardToMove,
                    int x,
                    int y,
                    const char *what)
{
    if (!mover || !from || !to) {
        qWarning() << "Ruled:" << what << "could not move card" << cardToMove.card_id()
                   << "- missing player or zone (from" << (from ? from->getName() : QStringLiteral("<null>")) << "to"
                   << (to ? to->getName() : QStringLiteral("<null>")) << ")";
        return false;
    }
    const Response::ResponseCode result =
        mover->moveCard(ges, from, QList<const CardToMove *>() << &cardToMove, to, x, y, true);
    RULED_TRACE("relay") << what << ": moveCard " << from->getName() << " -> " << to->getName()
                         << " cardId=" << cardToMove.card_id() << " result=" << static_cast<int>(result)
                         << " (1 = RespOk, 11 = RespContextError)";
    if (result != Response::RespOk) {
        qWarning() << "Ruled:" << what << "move rejected with code" << static_cast<int>(result) << "moving card"
                   << cardToMove.card_id() << "from" << from->getName() << "(player" << from->getPlayer()->getPlayerId()
                   << ") to" << to->getName() << "(player" << to->getPlayer()->getPlayerId()
                   << ") - the engine and the physical zones are now out of sync";
        return false;
    }
    return true;
}

/// Bind `StackPushed` to the cockatrice `Server_Card` that actually sits on the shared stack.
/// Walk from the end of the zone list toward the bottom so the most recently moved spell wins
/// when multiple copies share a name. Returns nullptr when no name match is found — callers
/// must not fall back to an unrelated card, since abilities have no physical card on the stack.
Server_Card *ruledPhysicalSpellOnCanonicalStack(Server_CardZone *stackZone, const QString &normalizedPushedName)
{
    if (!stackZone) {
        return nullptr;
    }
    const QList<Server_Card *> &cards = stackZone->getCards();
    for (int i = cards.size() - 1; i >= 0; --i) {
        Server_Card *c = cards.at(i);
        if (!c) {
            continue;
        }
        if (normalizeRuledCardName(c->getName()) == normalizedPushedName) {
            return c;
        }
    }
    return nullptr;
}

/// Legacy casual fallback: the engine refused the session for a non-card reason, so the
/// physical decks (left unshuffled for engine-ordered play) get a normal shuffle instead.
void shuffleMainDeckForRuledFallback(Server_AbstractPlayer *player)
{
    if (Server_CardZone *deckZone = player->getZones().value(ZoneNames::DECK)) {
        deckZone->shuffle();
    }
}

int expectedMainboardSizeForStartupSync(Server_Game *game,
                                        int playerId,
                                        const QList<QPair<int, QStringList>> &deckByPlayer)
{
    for (const QPair<int, QStringList> &row : deckByPlayer) {
        if (row.first == playerId) {
            return static_cast<int>(row.second.size());
        }
    }
    if (Server_AbstractPlayer *player = game->getPlayer(playerId)) {
        if (const Server_CardZone *deckZone = player->getZones().value(ZoneNames::DECK)) {
            return static_cast<int>(deckZone->getCards().size());
        }
    }
    return 60;
}

} // namespace

RuledBatchSynchronizer::RuledBatchSynchronizer(Server_Game *_game, RuledGameSession *_session)
    : game(_game), session(_session)
{
}

RuledPlayerBinding &RuledBatchSynchronizer::playerBinding(int playerId)
{
    return playerBindings[playerId];
}

void RuledBatchSynchronizer::resetForNewGame()
{
    playerBindings.clear();
    ruledEngineStackPushDescriptionsByObjectId.clear();
    ruledStackObjectIdToServerCardId.clear();
    ruledStackObjectIdToCasterPlayerId.clear();
    ruledStackTargetsByObjectId.clear();
    ruledStackCopyObjectIds.clear();
    ruledPendingCastVisualQueue.clear();
}

int RuledBatchSynchronizer::priorityPlayer() const
{
    return ruledPriorityPlayer;
}

void RuledBatchSynchronizer::setPriorityPlayer(int playerId)
{
    ruledPriorityPlayer = playerId;
}

void RuledBatchSynchronizer::applyAcceptedCommandVisuals(int playerId, const ruled::v1::RuledCommand &ruledCmd)
{
    if (Server_AbstractPlayer *cmdPlayer = game->getPlayer(playerId)) {
        Server_CardZone *handZone = cmdPlayer->getZones().value(ZoneNames::HAND);
        if (ruledCmd.has_play_land()) {
            Server_CardZone *tableZone = cmdPlayer->getZones().value(ZoneNames::TABLE);
            const auto &source = ruledCmd.play_land().source();
            Server_CardZone *sourceZone = nullptr;
            Server_Card *card = nullptr;
            if (source.location_case() == ruled::v1::LandSource::kHandIndex) {
                const int handIndex = static_cast<int>(source.hand_index());
                sourceZone = handZone;
                card = playerBinding(playerId).findHandCardByEngineIndex(static_cast<Server_Player *>(cmdPlayer),
                                                                         handIndex);
            } else if (source.location_case() == ruled::v1::LandSource::kExileObjectId) {
                const quint32 oid = static_cast<quint32>(source.exile_object_id());
                for (Server_AbstractPlayer *candidate : game->getPlayers()) {
                    auto *owner = dynamic_cast<Server_Player *>(candidate);
                    if (!owner) {
                        continue;
                    }
                    card = playerBinding(owner->getPlayerId()).findExileCardByEngineOid(owner, oid);
                    if (card) {
                        sourceZone = owner->getZones().value(ZoneNames::EXILE);
                        break;
                    }
                }
            }
            if (sourceZone && tableZone && card) {
                // CR 712: an MDFC land (a pathway) enters as the chosen face. Rename the physical
                // card to that face's Oracle name before it moves to the battlefield, so the
                // move event reveals the active face and the client shows its art (cards.xml has
                // a separate entry per face). The catalog maps both face names to the same engine
                // id, so later zone-view reconciliation still resolves this permanent.
                const int faceIndex = static_cast<int>(ruledCmd.play_land().face_index());
                if (faceIndex > 0) {
                    const QString activeName = faceDisplayName(cardIdForName(card->getName()), faceIndex);
                    if (!activeName.isEmpty() && activeName != card->getName()) {
                        card->setCardRef(CardRef{activeName});
                    }
                }
                CardToMove cardToMove;
                cardToMove.set_card_id(card->getId());
                GameEventStorage moveGes;
                // Cockatrice table uses 3 rows; lands belong on the bottom row (grid y = 2).
                static constexpr int RULED_LAND_GRID_Y = 2;
                if (ruledApplyMove(cmdPlayer, moveGes, sourceZone, tableZone, cardToMove, -1, RULED_LAND_GRID_Y,
                                   "playLand")) {
                    moveGes.sendToGame(game);
                }
            }
        } else if (ruledCmd.has_cast_spell() ||
                   (ruledCmd.has_submit_resolution_choice() && ruledCmd.submit_resolution_choice().has_cast_spell())) {
            const auto &acceptedCast =
                ruledCmd.has_cast_spell() ? ruledCmd.cast_spell() : ruledCmd.submit_resolution_choice().cast_spell();
            // Route all spells to the canonical (lowest player-id) stack zone so every
            // client's stack window shows the complete stack without a split view.
            // Resolution uses ruledStackObjectIdToCasterPlayerId to send the card to the
            // correct destination zone regardless of which physical zone it sat in.
            Server_CardZone *stackZone = ruledCanonicalStackZone(game);
            // CR 702.34: a flashback cast comes from the caster's graveyard, and
            // hand_card_index indexes that zone instead. Sourcing it from the hand would move
            // an unrelated hand card to the stack — and that card, not the flashback spell,
            // would then be the one exiled on resolution.
            //
            // The index cannot be used against the physical graveyard pile directly: the
            // engine's graveyard is oldest-first while the Cockatrice pile is newest-first, so
            // the binding resolves the engine slot to the real card.
            const auto &source = acceptedCast.source();
            Server_CardZone *sourceZone = nullptr;
            Server_Card *card = nullptr;
            QString sourceLabel = QStringLiteral("missing");
            if (source.location_case() == ruled::v1::CastSource::kHandIndex) {
                const int handIndex = static_cast<int>(source.hand_index());
                sourceZone = handZone;
                sourceLabel = QStringLiteral("hand:%1").arg(handIndex);
                card = playerBinding(playerId).findHandCardByEngineIndex(static_cast<Server_Player *>(cmdPlayer),
                                                                         handIndex);
            } else if (source.location_case() == ruled::v1::CastSource::kGraveyardObjectId ||
                       source.location_case() == ruled::v1::CastSource::kExileObjectId) {
                const bool fromGraveyard = source.location_case() == ruled::v1::CastSource::kGraveyardObjectId;
                const quint32 oid =
                    static_cast<quint32>(fromGraveyard ? source.graveyard_object_id() : source.exile_object_id());
                sourceLabel = QStringLiteral("%1:%2")
                                  .arg(fromGraveyard ? QStringLiteral("graveyard") : QStringLiteral("exile"))
                                  .arg(oid);
                for (Server_AbstractPlayer *candidate : game->getPlayers()) {
                    auto *owner = dynamic_cast<Server_Player *>(candidate);
                    if (!owner) {
                        continue;
                    }
                    card = fromGraveyard ? playerBinding(owner->getPlayerId()).findGraveyardCardByEngineOid(owner, oid)
                                         : playerBinding(owner->getPlayerId()).findExileCardByEngineOid(owner, oid);
                    if (card) {
                        sourceZone = owner->getZones().value(fromGraveyard ? ZoneNames::GRAVE : ZoneNames::EXILE);
                        break;
                    }
                }
            }
            RULED_TRACE("relay") << "cast source=" << sourceLabel
                                 << " sourceZone=" << (sourceZone ? sourceZone->getName() : QStringLiteral("<null>"))
                                 << " sourceZoneSize=" << (sourceZone ? sourceZone->getCards().size() : -1)
                                 << " resolvedCard=" << (card ? card->getName() : QStringLiteral("<none>"))
                                 << " serverCardId=" << (card ? card->getId() : -1);
            if (sourceZone && stackZone && card) {
                const int faceIndex = static_cast<int>(acceptedCast.face_index());
                if (faceIndex > 0) {
                    const QString cardId = cardIdForName(card->getName());
                    const QString activeName = faceDisplayName(cardId, faceIndex);
                    if (!activeName.isEmpty() && activeName != card->getName()) {
                        card->setCardRef(CardRef{activeName});
                    }
                }
                PendingRuledCastVisual pending;
                pending.cardName = card ? card->getName() : QString();
                pending.serverCardId = card ? card->getId() : -1;
                pending.casterPlayerId = playerId;
                for (int ti = 0; ti < acceptedCast.targets_size(); ++ti) {
                    pending.targetOids.append(static_cast<quint32>(acceptedCast.targets(ti).object_id()));
                }
                // Modal targets are grouped on the atomic command for rules resolution, but
                // Cockatrice's visual arrow/binding layer consumes one flat target list.
                for (const auto &mode : acceptedCast.selected_modes()) {
                    for (const auto &target : mode.targets()) {
                        pending.targetOids.append(static_cast<quint32>(target.object_id()));
                    }
                }
                ruledPendingCastVisualQueue.append(pending);
                CardToMove cardToMove;
                cardToMove.set_card_id(card->getId());
                GameEventStorage moveGes;
                if (ruledApplyMove(cmdPlayer, moveGes, sourceZone, stackZone, cardToMove, -1, 0, "cast")) {
                    moveGes.sendToGame(game);
                }
            }
        }
    }
}

void RuledBatchSynchronizer::applyStackResolvedEvent(const ruled::v1::StackResolved &stackResolved,
                                                     const QHash<quint32, int> &battlefieldGridRows,
                                                     const QHash<quint32, int> &battlefieldDisplayPlayers)
{
    const quint32 resolvedOid = static_cast<quint32>(stackResolved.object_id());
    // A copy has no stack-zone Server_Card to move. Permanent spell copies materialize through an
    // earlier TokenCreated event; returning here preserves that newly minted physical token and
    // also prevents the name fallback from moving the original spell with the same card_id/name.
    if (ruledStackCopyObjectIds.remove(resolvedOid)) {
        ruledEngineStackPushDescriptionsByObjectId.remove(resolvedOid);
        return;
    }
    const QString engineStackDescription = ruledEngineStackPushDescriptionsByObjectId.value(resolvedOid);

    auto tryResolveCardOnStack = [this, &stackResolved, &battlefieldGridRows, &battlefieldDisplayPlayers](
                                     Server_AbstractPlayer *ab, Server_CardZone *stackZone, Server_Card *card) -> bool {
        if (!ab || !stackZone || !card) {
            return false;
        }
        // The engine sets a destination on every resolve; an unspecified value means
        // engine/server skew. Default to graveyard (CR 608.3: only permanent spells
        // go to the battlefield).
        const ruled::v1::StackResolveDestination dest = stackResolved.destination();
        if (dest != ruled::v1::STACK_RESOLVE_DESTINATION_BATTLEFIELD &&
            dest != ruled::v1::STACK_RESOLVE_DESTINATION_GRAVEYARD &&
            dest != ruled::v1::STACK_RESOLVE_DESTINATION_EXILE &&
            dest != ruled::v1::STACK_RESOLVE_DESTINATION_LIBRARY) {
            qWarning() << "Ruled: StackResolved for object" << stackResolved.object_id()
                       << "has no destination; defaulting to graveyard";
        }
        const bool goesToBattlefield = (dest == ruled::v1::STACK_RESOLVE_DESTINATION_BATTLEFIELD);
        const bool goesToExile = (dest == ruled::v1::STACK_RESOLVE_DESTINATION_EXILE);
        const bool goesToLibrary = (dest == ruled::v1::STACK_RESOLVE_DESTINATION_LIBRARY);
        const quint32 resolvedOidLocal = static_cast<quint32>(stackResolved.object_id());
        const int casterPid = ruledStackObjectIdToCasterPlayerId.value(resolvedOidLocal, -1);
        Server_AbstractPlayer *destPlayer = ab;
        const auto displayPlayerIt = battlefieldDisplayPlayers.constFind(resolvedOidLocal);
        if (goesToBattlefield && displayPlayerIt != battlefieldDisplayPlayers.constEnd()) {
            if (Server_AbstractPlayer *displayPlayer = game->getPlayer(*displayPlayerIt)) {
                destPlayer = displayPlayer;
            }
        } else if (casterPid >= 0) {
            if (Server_AbstractPlayer *cp = game->getPlayer(casterPid)) {
                destPlayer = cp;
            }
        }
        if (goesToLibrary && stackResolved.has_owner_player_id()) {
            if (Server_AbstractPlayer *owner = game->getPlayer(stackResolved.owner_player_id())) {
                destPlayer = owner;
            }
        }
        const char *targetZoneName = goesToBattlefield ? ZoneNames::TABLE
                                     : goesToExile     ? ZoneNames::EXILE
                                     : goesToLibrary   ? ZoneNames::DECK
                                                       : ZoneNames::GRAVE;
        Server_CardZone *targetZone = destPlayer->getZones().value(targetZoneName);
        if (!targetZone) {
            return false;
        }

        CardToMove cardToMove;
        cardToMove.set_card_id(card->getId());
        cardToMove.set_face_down(goesToLibrary);
        GameEventStorage moveGes;
        int targetY = 0;
        if (goesToBattlefield) {
            const auto rowIt = battlefieldGridRows.constFind(resolvedOidLocal);
            if (rowIt != battlefieldGridRows.constEnd()) {
                targetY = *rowIt;
            } else {
                targetY = 1;
                qWarning() << "ruled StackResolved battlefield entry missing authoritative row for oid"
                           << resolvedOidLocal;
            }
        }
        // Battlefield: -1 means "find a free grid column". Library order is replaced by the
        // authoritative ZoneView immediately afterwards, so append the card to the concealed
        // pool first. Graveyard and exile render their newest card at position 0.
        const int targetX = (goesToBattlefield || goesToLibrary) ? -1 : 0;
        if (ruledApplyMove(ab, moveGes, stackZone, targetZone, cardToMove, targetX, targetY, "stackResolved")) {
            if (!goesToBattlefield) {
                const QString cardId = cardIdForName(card->getName());
                const QString physicalDisplayName = faceDisplayName(cardId, 0);
                if (!physicalDisplayName.isEmpty() && physicalDisplayName != card->getName()) {
                    card->setCardRef(CardRef{physicalDisplayName});
                }
                card->setAnnotation(
                    withoutRuledEnchantingAnnotation(withoutRuledCopyAnnotation(card->getAnnotation())));
            }
            if (goesToLibrary) {
                // Cross-player moves can reissue Server_Card.id. Register the post-move identity
                // before the ZoneView reconcile so duplicate-name library cards cannot swap OIDs.
                card->setFaceDown(true);
                playerBinding(destPlayer->getPlayerId()).registerLibraryEngineOid(resolvedOidLocal, card->getId());
            } else if (goesToExile) {
                playerBinding(destPlayer->getPlayerId()).exileEngineOidToServerCardId.insert(resolvedOidLocal, card->getId());
            } else if (!goesToBattlefield) {
                playerBinding(destPlayer->getPlayerId()).graveyardEngineOidToServerCardId.insert(resolvedOidLocal, card->getId());
            }
            moveGes.sendToGame(game);
            return true;
        }
        return false;
    };

    // Multiplayer: each player has their own Cockatrice stack zone. Prefer the physical card that was mapped when
    // this object was pushed (cast_spell → stack_pushed); never pop "first non-empty stack in player iteration order".
    const auto mappedIdIt = ruledStackObjectIdToServerCardId.constFind(resolvedOid);
    RULED_TRACE("relay") << "stackResolved: oid=" << resolvedOid << " engineDescription='" << engineStackDescription
                         << "' mappedServerCardId="
                         << (mappedIdIt != ruledStackObjectIdToServerCardId.constEnd() ? mappedIdIt.value() : -1)
                         << " (-1 = no cast mapping, will fall back to name match)";
    if (mappedIdIt != ruledStackObjectIdToServerCardId.constEnd()) {
        const int serverCardId = mappedIdIt.value();
        for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
            if (!ab) {
                continue;
            }
            Server_CardZone *stackZone = ab->getZones().value(ZoneNames::STACK);
            if (!stackZone) {
                continue;
            }
            if (Server_Card *card = stackZone->getCard(serverCardId, nullptr, false)) {
                if (tryResolveCardOnStack(ab, stackZone, card)) {
                    return;
                }
            }
        }
    }

    // Fallback when no cast mapping exists (tokens / older batches): top of first non-empty stack.
    // Only fires when the top card's name matches the engine description — abilities that have no
    // physical card on the stack must not accidentally resolve an unrelated card here.
    for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
        if (!ab) {
            continue;
        }
        Server_CardZone *stackZone = ab->getZones().value(ZoneNames::STACK);
        if (!stackZone || stackZone->getCards().isEmpty()) {
            continue;
        }
        Server_Card *card = stackZone->getCards().last();
        if (!card) {
            continue;
        }
        if (!engineStackDescription.isEmpty() &&
            normalizeRuledCardName(card->getName()) != normalizeRuledCardName(engineStackDescription)) {
            break;
        }
        if (tryResolveCardOnStack(ab, stackZone, card)) {
            return;
        }
        break;
    }
}

void RuledBatchSynchronizer::applyStackObjectCounteredEvent(const ruled::v1::StackObjectCountered &countered)
{
    const quint32 objectId = static_cast<quint32>(countered.object_id());
    ruledStackCopyObjectIds.remove(objectId);
    ruledStackTargetsByObjectId.remove(objectId);
    ruledStackObjectIdToServerCardId.remove(objectId);
    ruledStackObjectIdToCasterPlayerId.remove(objectId);
    ruledEngineStackPushDescriptionsByObjectId.remove(objectId);
}

RuledBatchSynchronizer::BatchApplyResult RuledBatchSynchronizer::applyBatch(const ruled::v1::IpcResponse &resp)
{
    BatchApplyResult result;
    if (!resp.has_batch()) {
        return result;
    }
    const ruled::v1::RuledEventBatch &batch = resp.batch();
    const QHash<quint32, int> battlefieldGridRows = authoritativeBattlefieldGridRows(batch);
    const QHash<quint32, int> battlefieldDisplayPlayers = authoritativeBattlefieldDisplayPlayers(batch);

    // One named method per pass. The pass order is load-bearing — never merge or reorder:
    // the catalog must be indexed before anything resolves a card name through it, the
    // pre-batch oid capture feeds PermanentMoved translation, tokens must exist before
    // the zone-view sync binds battlefield slots, PermanentMoved must run before zone views
    // reconcile hand/library counts, and attachment restore plus life/mana/combat
    // translation need the fresh post-zone-view oid maps.

    // Mid-game catalog refresh. Almost every batch carries no CardCatalog and leaves the index
    // untouched; a batch that does carries the whole catalog and replaces it.
    indexCardCatalogEvents(batch);
    applyDevCardConjures(batch, battlefieldGridRows, battlefieldDisplayPlayers, result);

    // Capture the pre-batch engine_oid -> Server_Card map per player. The engine has
    // already removed dead permanents from its battlefield, so the upcoming zone-view
    // sync will rebuild the map without them. We need the *prior* mapping to translate
    // PermanentMoved events into moveCard(...) calls.
    QHash<int, QHash<quint32, int>> preBatchOidMaps;
    for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
        if (!ab) {
            continue;
        }
        preBatchOidMaps.insert(ab->getPlayerId(), playerBinding(ab->getPlayerId()).engineOidToServerCardId);
    }

    applyTokenCreations(batch, battlefieldGridRows);
    applyPermanentMoves(batch, preBatchOidMaps, battlefieldGridRows, battlefieldDisplayPlayers);
    applyPhaseStackAndZoneViews(batch, battlefieldGridRows, battlefieldDisplayPlayers, result);
    applyFaceDisplays(batch, result);
    applyAttachmentRestores(batch);
    applyLifeManaAndCombatEvents(batch);
    return result;
}

// A dev command conjured a card that was in no decklist (see DevCardConjured), so there is no
// physical Server_Card behind the engine's new object. Mint one before the zone-view sync below,
// for the same reason applyTokenCreations runs early: the reconcile matches engine slots to
// physical cards and abandons the whole sync if it cannot.
//
// Runs before applyTokenCreations and applyPermanentMoves because a card conjured by this batch
// may also be moved by it.
void RuledBatchSynchronizer::applyDevCardConjures(const ruled::v1::RuledEventBatch &batch,
                                                  const QHash<quint32, int> &battlefieldGridRows,
                                                  const QHash<quint32, int> &battlefieldDisplayPlayers,
                                                  BatchApplyResult &result)
{
    GameEventStorage conjureGes;
    bool conjureGesHasEvents = false;
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (!e.has_dev_card_conjured()) {
            continue;
        }
        const auto &dc = e.dev_card_conjured();
        Server_AbstractPlayer *owner = game->getPlayer(dc.owner_player_id());
        if (!owner) {
            continue;
        }
        const bool toBattlefield = dc.zone() == ruled::v1::DEV_ZONE_BATTLEFIELD;
        int battlefieldGridY = ruledBattlefieldGridY(dc.is_creature(), false);
        if (toBattlefield) {
            const auto rowIt = battlefieldGridRows.constFind(static_cast<quint32>(dc.object_id()));
            if (rowIt != battlefieldGridRows.constEnd()) {
                battlefieldGridY = *rowIt;
            } else {
                qWarning() << "ruled dev battlefield conjure missing authoritative row for oid" << dc.object_id();
            }
        }
        Server_AbstractPlayer *physicalHolder = owner;
        if (toBattlefield) {
            const auto holderIt = battlefieldDisplayPlayers.constFind(static_cast<quint32>(dc.object_id()));
            if (holderIt != battlefieldDisplayPlayers.constEnd()) {
                if (Server_AbstractPlayer *candidate = game->getPlayer(*holderIt)) {
                    physicalHolder = candidate;
                }
            }
        }
        const bool created = playerBinding(physicalHolder->getPlayerId())
                                 .createRuledDevCard(static_cast<Server_Player *>(physicalHolder), dc.object_id(),
                                                     QString::fromStdString(dc.card_name()), battlefieldGridY,
                                                     toBattlefield, conjureGes);
        if (!created) {
            continue;
        }
        if (toBattlefield) {
            conjureGesHasEvents = true;
        } else {
            // A hand conjure broadcasts no creation event of its own — that would reveal the card
            // to the opponent. Flagging the hand as changed makes processRuledPayload issue the
            // ordinary full-state resync, which redacts private zones per recipient.
            result.handOrLibraryChanged = true;
        }
    }
    if (conjureGesHasEvents) {
        conjureGes.sendToGame(game);
    }
}

// Tokens (CR 111) appear on the engine battlefield with no physical card behind them. Mint
// one on the controller's table for each TokenCreated event before the zone-view sync,
// so that sync can bind the engine battlefield slot to a real Server_Card by ObjectId. Runs
// before PermanentMoved: a token created this batch cannot also have died this batch.
void RuledBatchSynchronizer::applyTokenCreations(const ruled::v1::RuledEventBatch &batch,
                                                 const QHash<quint32, int> &battlefieldGridRows)
{
    GameEventStorage tokenCreateGes;
    bool tokenCreateGesHasEvents = false;
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (!e.has_token_created()) {
            continue;
        }
        const auto &tc = e.token_created();
        if (!tc.has_identity()) {
            continue;
        }
        const quint32 objectId = static_cast<quint32>(tc.object_id());
        const bool identityIsLand = std::any_of(tc.identity().types().begin(), tc.identity().types().end(),
                                                [](const std::string &type) { return type == "Land"; });
        int battlefieldGridY = ruledBattlefieldGridY(tc.identity().is_creature(), identityIsLand);
        const auto rowIt = battlefieldGridRows.constFind(objectId);
        if (rowIt != battlefieldGridRows.constEnd()) {
            battlefieldGridY = *rowIt;
        } else {
            qWarning() << "ruled token creation missing authoritative row for oid" << objectId;
        }
        if (Server_AbstractPlayer *controller = game->getPlayer(tc.controller_player_id())) {
            playerBinding(tc.controller_player_id())
                .createRuledToken(static_cast<Server_Player *>(controller), objectId, tc.identity(), battlefieldGridY,
                                  tc.enters_tapped(), tokenCreateGes);
            tokenCreateGesHasEvents = true;
        }
    }
    if (tokenCreateGesHasEvents) {
        tokenCreateGes.sendToGame(game);
    }
}

// Apply every PermanentMoved before zone_view. Hand discards are already absent from the
// engine hand list in the sync that follows, so the server must move the physical card
// first or applyRuledEngineZoneView's deck+hand pool counts disagree with the engine.
void RuledBatchSynchronizer::applyPermanentMoves(const ruled::v1::RuledEventBatch &batch,
                                                 const QHash<int, QHash<quint32, int>> &preBatchOidMaps,
                                                 const QHash<quint32, int> &battlefieldGridRows,
                                                 const QHash<quint32, int> &battlefieldDisplayPlayers)
{
    GameEventStorage permanentMoveGes;
    bool permanentMoveGesHasEvents = false;
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (!e.has_permanent_moved()) {
            continue;
        }
        const auto &pm = e.permanent_moved();
        const int ownerId = pm.owner_player_id();
        const quint32 oid = static_cast<quint32>(pm.object_id());
        Server_AbstractPlayer *owner = game->getPlayer(ownerId);
        if (!owner) {
            continue;
        }
        Server_Card *card = nullptr;
        // A permanent is registered under the player who *controls* it, which is not always its
        // owner (reanimation). Search every seat, owner first so the ordinary no-control-change
        // case behaves exactly as before. Getting this wrong is not a miss but a mis-hit: the
        // card_id fallback further down would happily pull a same-named card out of the owner's
        // deck instead.
        QList<int> searchOrder;
        searchOrder.append(ownerId);
        for (int pid : game->getPlayers().keys()) {
            if (pid != ownerId) {
                searchOrder.append(pid);
            }
        }
        for (int pid : searchOrder) {
            if (card) {
                break;
            }
            Server_AbstractPlayer *holder = game->getPlayer(pid);
            if (!holder) {
                continue;
            }
            const auto preIt = preBatchOidMaps.constFind(pid);
            if (preIt != preBatchOidMaps.constEnd()) {
                const auto cardIdIt = preIt->constFind(oid);
                if (cardIdIt != preIt->constEnd()) {
                    for (const char *zn : {ZoneNames::TABLE, ZoneNames::HAND, ZoneNames::STACK, ZoneNames::DECK}) {
                        Server_CardZone *z = holder->getZones().value(zn);
                        if (!z) {
                            continue;
                        }
                        if (Server_Card *c = z->getCard(*cardIdIt, nullptr, false)) {
                            card = c;
                            break;
                        }
                    }
                }
            }
            // ReturnFromGraveyard: the card may be in the graveyard zone, not in the
            // battlefield/hand OID map. Try the graveyard map maintained by the seat's binding.
            // Reanimate reads the *opponent's* graveyard, so this is not owner-scoped either.
            if (!card) {
                if (auto *sp = qobject_cast<Server_Player *>(holder)) {
                    if (Server_Card *c = playerBinding(pid).findGraveyardCardByEngineOid(sp, oid)) {
                        card = c;
                    }
                }
            }
            // CR 610.3 temporary-exile returns originate in the public exile pile. Resolve the
            // exact engine oid before any card-name fallback: two same-name cards may be exiled at
            // once and only the generation linked to this source is allowed to return.
            if (!card) {
                if (auto *sp = qobject_cast<Server_Player *>(holder)) {
                    if (Server_Card *c = playerBinding(pid).findExileCardByEngineOid(sp, oid)) {
                        card = c;
                    }
                }
            }
        }
        if (!card) {
            // A spell leaving the stack (e.g. countered) physically lives on the single shared
            // canonical stack zone (owned by the lowest player-id), not in this owner's own zones,
            // and stack cards are never in the per-player engine-oid map (only battlefield + hand
            // are — see RuledPlayerBinding::applyRuledEngineZoneView). Resolve it through the global
            // stack-object map and search every player's stack zone; the move below routes it to
            // the owner's destination. This makes countered spells generic — no name special-case.
            //
            // MUST run before the mill fallback: a countered spell's card_id (e.g. "lightning_bolt")
            // would otherwise name-match a *different* copy sitting in the owner's library, moving
            // that to the graveyard and leaving the real stacked card stranded as a ghost.
            const auto stackCardIdIt = ruledStackObjectIdToServerCardId.constFind(oid);
            if (stackCardIdIt != ruledStackObjectIdToServerCardId.constEnd()) {
                for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
                    if (!ab) {
                        continue;
                    }
                    if (Server_CardZone *stackZone = ab->getZones().value(ZoneNames::STACK)) {
                        if (Server_Card *c = stackZone->getCard(*stackCardIdIt, nullptr, false)) {
                            card = c;
                            break;
                        }
                    }
                }
            }
        }
        if (!card && pm.has_source_library_position()) {
            Server_CardZone *deck = owner->getZones().value(ZoneNames::DECK);
            const int position = static_cast<int>(pm.source_library_position());
            if (!deck || position < 0 || position >= deck->getCards().size()) {
                qWarning().noquote() << "ruled indexed library PermanentMoved out of range: oid" << oid << "position"
                                     << position << "owner" << ownerId;
                continue;
            }
            Server_Card *indexed = deck->getCards().at(position);
            const QString wantCardId = QString::fromStdString(pm.card_id());
            if (!indexed || (!wantCardId.isEmpty() && cardIdForName(indexed->getName()) != wantCardId)) {
                qWarning().noquote() << "ruled indexed library PermanentMoved identity mismatch: oid" << oid
                                     << "position" << position << "expected" << wantCardId;
                continue;
            }
            card = indexed;
        }
        if (!card) {
            // Legacy fallback for a library object whose identity predates a complete private-zone
            // sync. Current zone views carry object ids and normally resolve the exact card above.
            const QString wantCardId = QString::fromStdString(pm.card_id());
            if (!wantCardId.isEmpty()) {
                if (Server_CardZone *deck = owner->getZones().value(ZoneNames::DECK)) {
                    for (Server_Card *c : deck->getCards()) {
                        if (cardIdForName(c->getName()) == wantCardId) {
                            card = c;
                            break;
                        }
                    }
                }
            }
        }
        if (!card) {
            qWarning().noquote() << "ruled PermanentMoved unresolved: oid" << oid << "card_id"
                                 << QString::fromStdString(pm.card_id()) << "owner" << ownerId;
            continue;
        }
        Server_CardZone *startZone = card->getZone();
        if (!startZone) {
            continue;
        }
        // `destX` is the insert position, not a grid column, for the zones without coords.
        // A pile zone renders only its *front* card (PileZone::paint draws index 0), so the most
        // recently added card must go to position 0 — that is what the freeform client sends for
        // GRAVE/EXILE. Passing -1 appends to the far end (server_abstract_player.cpp turns it into
        // `getCards().size()`), which leaves the oldest card showing forever no matter how much
        // is milled. TABLE uses -1 to mean "find a free grid column"; hand/library order is not
        // rendered as a pile, so they keep the append behaviour.
        const char *destZone = ZoneNames::GRAVE;
        int destX = 0;
        switch (pm.destination()) {
            case ruled::v1::PermanentMoved::DESTINATION_HAND:
                destZone = ZoneNames::HAND;
                destX = -1;
                break;
            case ruled::v1::PermanentMoved::DESTINATION_LIBRARY:
                destZone = ZoneNames::DECK;
                destX = -1;
                break;
            case ruled::v1::PermanentMoved::DESTINATION_EXILE:
                destZone = ZoneNames::EXILE;
                destX = 0;
                break;
            case ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD:
                destZone = ZoneNames::TABLE;
                destX = -1;
                break;
            case ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD:
            default:
                destZone = ZoneNames::GRAVE;
                destX = 0;
                break;
        }
        // CR 110.2 vs CR 400.3: a permanent enters the battlefield under its *controller*, but
        // every other zone belongs to its owner — so only the battlefield destination follows
        // controller_player_id. The engine always sets that field (proto3 scalars carry no
        // presence and player id 0 is valid), and it equals the owner when control is unchanged.
        Server_AbstractPlayer *destPlayer = owner;
        if (pm.destination() == ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD) {
            const auto displayPlayerIt = battlefieldDisplayPlayers.constFind(oid);
            const int destinationPlayerId =
                displayPlayerIt != battlefieldDisplayPlayers.constEnd() ? *displayPlayerIt : pm.controller_player_id();
            if (Server_AbstractPlayer *displayPlayer = game->getPlayer(destinationPlayerId)) {
                destPlayer = displayPlayer;
            }
        }
        Server_CardZone *targetZone = destPlayer->getZones().value(destZone);
        if (!targetZone) {
            continue;
        }
        // CR 400.7: transform/flip status, a chosen MDFC face, and a copy snapshot do not carry to
        // another zone. Restore the physical catalog display before moveCard serializes the event
        // so every client receives the underlying card's name, image, hover details, and no stale
        // copy annotation. face_display_names[0] already preserves whole-card display for split and
        // Adventure cards, so this normalization is safe for every layout.
        if (pm.destination() != ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD) {
            const QString cardId = QString::fromStdString(pm.card_id());
            const QString resolvedCardId = cardId.isEmpty() ? cardIdForName(card->getName()) : cardId;
            const QString physicalDisplayName = faceDisplayName(resolvedCardId, 0);
            if (!physicalDisplayName.isEmpty() && physicalDisplayName != card->getName()) {
                card->setCardRef(CardRef{physicalDisplayName});
            }
            const QString annotationWithoutCopy = withoutRuledCopyAnnotation(card->getAnnotation());
            const QString annotationWithoutAttachment = withoutRuledEnchantingAnnotation(annotationWithoutCopy);
            if (annotationWithoutAttachment != card->getAnnotation()) {
                card->setAnnotation(annotationWithoutAttachment);
            }
        }
        // Hidden zones (the library) address cards by position index in moveCard/getCard, not by
        // intrinsic card id. Mill moves come out of the deck, so pass the card's current position;
        // public/private zones (table, hand) use the card id. Position is recomputed per event
        // because each successful move re-indexes the hidden zone.
        int moveCardId = card->getId();
        if (startZone->getType() == ServerInfo_Zone::HiddenZone) {
            moveCardId = startZone->getCards().indexOf(card);
            if (moveCardId < 0) {
                continue;
            }
        }
        CardToMove cardToMove;
        cardToMove.set_card_id(moveCardId);
        cardToMove.set_face_down(pm.face_down());
        // Move through the seat that physically holds the card, not the owner: for a permanent
        // controlled by someone else the card sits on the *controller's* table, and moveCard
        // builds its event from startzone->getPlayer().
        Server_AbstractPlayer *mover = startZone->getPlayer() ? startZone->getPlayer() : owner;
        int destY = 0;
        if (pm.destination() == ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD) {
            const auto rowIt = battlefieldGridRows.constFind(oid);
            if (rowIt != battlefieldGridRows.constEnd()) {
                destY = *rowIt;
            } else {
                destY = 1;
                qWarning() << "ruled PermanentMoved battlefield entry missing authoritative row for oid" << oid;
            }
        }
        if (ruledApplyMove(mover, permanentMoveGes, startZone, targetZone, cardToMove, destX, destY,
                           "permanentMoved")) {
            permanentMoveGesHasEvents = true;
            // Capture the post-move physical identity before any positional reconciliation.
            // Tokens destroyed on departure are no longer in the destination zone.
            if (targetZone->getCards().contains(card)) {
                auto &binding = playerBinding(destPlayer->getPlayerId());
                if (pm.destination() == ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD) {
                    binding.graveyardEngineOidToServerCardId.insert(oid, card->getId());
                } else if (pm.destination() == ruled::v1::PermanentMoved::DESTINATION_EXILE) {
                    binding.exileEngineOidToServerCardId.insert(oid, card->getId());
                }
            }
            // A cross-player move reissues Server_Card::id from the destination player's space
            // (server_abstract_player.cpp), and the engine oid is absent from the destination
            // seat's binding until the next zone-view sync. Register it now: otherwise
            // applyRuledEngineZoneView falls back to matching by card_id, which can silently
            // swap the oid<->Server_Card pairing between two identical permanents.
            if (destPlayer != mover && pm.destination() == ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD) {
                playerBinding(destPlayer->getPlayerId()).registerEngineOid(oid, card->getId());
            }
        }
    }
    if (permanentMoveGesHasEvents) {
        permanentMoveGes.sendToGame(game);
    }
}

// A full authoritative battlefield snapshot assigns each permanent to its current controller's
// per-player view. When layer 2 changes control without changing zones, move the already-bound
// physical card between those players' TABLE zones before either binding reconciles its list.
// This preserves the Server_Card identity while letting the ordinary zone-view pass rebuild both
// seats' oid maps against their new physical contents.
void RuledBatchSynchronizer::applyBattlefieldControllerTransfers(const ruled::v1::ZoneViewSync &zoneView,
                                                                 BatchApplyResult &result)
{
    if (zoneView.battlefields_unchanged()) {
        return;
    }

    GameEventStorage moveGes;
    bool moveGesHasEvents = false;
    for (const auto &view : zoneView.per_player()) {
        Server_AbstractPlayer *targetPlayer = game->getPlayer(view.player_id());
        Server_CardZone *targetTable = targetPlayer ? targetPlayer->getZones().value(ZoneNames::TABLE) : nullptr;
        if (!targetPlayer || !targetTable) {
            continue;
        }

        for (const auto &object : view.battlefield_objects()) {
            const quint32 oid = static_cast<quint32>(object.object_id());
            Server_Card *card = findBattlefieldCardByEngineOid(oid, view.player_id());
            if (!card || !card->getZone() || card->getZone()->getName() != ZoneNames::TABLE ||
                !card->getZone()->getPlayer()) {
                continue;
            }
            Server_CardZone *startTable = card->getZone();
            Server_AbstractPlayer *mover = startTable->getPlayer();
            if (mover == targetPlayer) {
                continue;
            }

            // moveCard rejects an attached card moving onto a table. Temporarily clear every
            // relationship touched by the transfer so the post-zone-view attachment pass emits
            // fresh cross-player Event_AttachCard identities for both clients.
            Server_Card *oldParent = card->getParentCard();
            const QList<Server_Card *> oldChildren = card->getAttachedCards();
            if (oldParent) {
                card->setParentCard(nullptr);
            }
            for (Server_Card *child : oldChildren) {
                child->setParentCard(nullptr);
            }

            const int y = card->getY();
            const int x = targetTable->getFreeGridColumn(-1, y, card->getName(), y != 2);
            CardToMove cardToMove;
            cardToMove.set_card_id(card->getId());
            if (!ruledApplyMove(mover, moveGes, startTable, targetTable, cardToMove, x, y,
                                "battlefieldControllerChanged")) {
                if (oldParent) {
                    card->setParentCard(oldParent);
                }
                for (Server_Card *child : oldChildren) {
                    child->setParentCard(card);
                }
                continue;
            }

            playerBinding(targetPlayer->getPlayerId()).registerEngineOid(oid, card->getId());
            moveGesHasEvents = true;
            result.battlefieldOrderChanged = true;
        }
    }
    if (moveGesHasEvents) {
        moveGes.sendToGame(game);
    }
}

// Phase / priority / stack push+resolve / zone view + tap sync.
// Tap state propagates from the engine on every batch — declare attackers, mana
// payment, and untap all use this path (no longer gated on an explicit untap event).
void RuledBatchSynchronizer::applyPhaseStackAndZoneViews(const ruled::v1::RuledEventBatch &batch,
                                                         const QHash<quint32, int> &battlefieldGridRows,
                                                         const QHash<quint32, int> &battlefieldDisplayPlayers,
                                                         BatchApplyResult &result)
{
    GameEventStorage tapSyncGes;
    bool batchHasUntapPhase = false;
    // CR 701.20: permanents the engine reported as genuinely becoming untapped in this batch —
    // an untap effect, the untap step, or the CR 605 mana-ability undo. The binding applies these
    // regardless of the untap-step guard, so they must be gathered before any zone view is
    // applied (the event may appear after the zone_view events in the batch).
    QSet<quint32> engineUntappedOids;
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (e.has_phase_changed() && e.phase_changed().phase_id() == ruled::v1::PHASE_ID_UNTAP) {
            batchHasUntapPhase = true;
        }
        if (e.has_permanents_untapped()) {
            for (const auto oid : e.permanents_untapped().object_ids()) {
                engineUntappedOids.insert(static_cast<quint32>(oid));
            }
        }
    }
    // PermanentsUntapped is an authoritative state edge, not merely a hint that relaxes the
    // zone-view reconciliation guard. Canonical settlement may coalesce the battlefield snapshot
    // away after an untap step, and mid-turn untap effects / mana undo can do the same. Drive every
    // already-bound physical card directly so both clients receive an idempotent AttrTapped=0 even
    // when the batch carries no battlefield replacement.
    for (const quint32 oid : engineUntappedOids) {
        Server_Card *card = findBattlefieldCardByEngineOid(oid);
        if (!card || !card->getZone() || !card->getZone()->getPlayer()) {
            continue;
        }
        card->setTapped(false);
        Event_SetCardAttr untapEvent;
        untapEvent.set_zone_name(std::string(ZoneNames::TABLE));
        untapEvent.set_card_id(card->getId());
        untapEvent.set_attribute(AttrTapped);
        untapEvent.set_attr_value("0");
        tapSyncGes.enqueueGameEvent(untapEvent, card->getZone()->getPlayer()->getPlayerId());
        result.tapStateEventsQueued = true;
    }
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (e.has_phase_changed()) {
            const int newActive = e.phase_changed().active_player_id();
            if (newActive >= 0 && game->getActivePlayer() != newActive) {
                game->setActivePlayer(newActive);
            }
            const int mappedPhase = ruledPhaseToCockatricePhase(e.phase_changed().phase_id());
            if (mappedPhase >= 0 && game->getActivePhase() != mappedPhase) {
                game->setActivePhase(mappedPhase);
            }
            result.phaseChanged = true;
        }
        if (e.has_priority_changed()) {
            const int newPriority = e.priority_changed().player_id();
            if (newPriority >= 0 && newPriority != ruledPriorityPlayer) {
                ruledPriorityPlayer = newPriority;
                if (Server_AbstractPlayer *prioPlayer = game->getPlayer(newPriority)) {
                    Event_GameSay priorityEvent;
                    const QString playerName = QString::fromStdString(prioPlayer->getUserInfo()->name());
                    priorityEvent.set_message(QStringLiteral("%1 gains priority.").arg(playerName).toStdString());
                    game->sendGameEventContainer(game->prepareGameEvent(priorityEvent, -1));
                }
            }
        }
        if (!e.has_zone_view()) {
            if (e.has_stack_pushed()) {
                const auto &sp = e.stack_pushed();
                const quint32 pushedOid = static_cast<quint32>(sp.object_id());
                // CR 707.10: a spell copy (Twincast/Fork) has no physical card on the shared stack.
                // Record its display name (for the prompt/log) but never bind it to a Server_Card —
                // its card_id matches the original, so binding would steal the original's physical
                // card. The resolve handler treats it as a no-op move (see ruledStackCopyObjectIds).
                if (sp.is_copy()) {
                    ruledStackCopyObjectIds.insert(pushedOid);
                    QString copyName = QString::fromStdString(sp.description());
                    if (!sp.card_id().empty()) {
                        const QString catalogName = cardNameForId(QString::fromStdString(sp.card_id()));
                        if (!catalogName.isEmpty()) {
                            copyName = catalogName;
                        }
                    }
                    ruledEngineStackPushDescriptionsByObjectId.insert(pushedOid, copyName);
                    continue;
                }
                // Physical binding matches on display names. For spells, resolve the engine
                // card id through the catalog (authoritative Oracle name) rather than trusting
                // the free-form description; abilities carry no card_id and keep the description.
                QString pushedName = QString::fromStdString(sp.description());
                if (!sp.card_id().empty()) {
                    const QString catalogName = cardNameForId(QString::fromStdString(sp.card_id()));
                    if (!catalogName.isEmpty()) {
                        pushedName = catalogName;
                    }
                }
                ruledEngineStackPushDescriptionsByObjectId.insert(pushedOid, pushedName);
                const QString normalizedPushedName = normalizeRuledCardName(pushedName);
                QList<PendingRuledCastVisual>::iterator bindIt = ruledPendingCastVisualQueue.end();
                for (auto it = ruledPendingCastVisualQueue.begin(); it != ruledPendingCastVisualQueue.end(); ++it) {
                    if (normalizeRuledCardName(it->cardName) == normalizedPushedName) {
                        bindIt = it;
                        break;
                    }
                }
                // CastSpell always enqueues one pending entry before applyBatch; if the engine
                // stack description and Cockatrice printing names differ, name match fails and clients
                // never get BattlefieldObjectMap rows for stack oids (spell targeting arrows break).
                if (bindIt == ruledPendingCastVisualQueue.end() && !ruledPendingCastVisualQueue.isEmpty()) {
                    bindIt = ruledPendingCastVisualQueue.begin();
                }
                if (bindIt != ruledPendingCastVisualQueue.end()) {
                    ruledStackTargetsByObjectId.insert(pushedOid, bindIt->targetOids);
                    if (bindIt->serverCardId >= 0) {
                        ruledStackObjectIdToServerCardId.insert(pushedOid, bindIt->serverCardId);
                    }
                    if (bindIt->casterPlayerId >= 0) {
                        ruledStackObjectIdToCasterPlayerId.insert(pushedOid, bindIt->casterPlayerId);
                    }
                    ruledPendingCastVisualQueue.erase(bindIt);
                } else {
                    QVector<quint32> tlist;
                    tlist.reserve(sp.targets_size());
                    for (int ti = 0; ti < sp.targets_size(); ++ti) {
                        tlist.append(static_cast<quint32>(sp.targets(ti).object_id()));
                    }
                    if (!tlist.isEmpty()) {
                        ruledStackTargetsByObjectId.insert(pushedOid, tlist);
                    }
                }
                // Authoritative: map engine stack oid to the physical card.
                // All spells now go into the canonical zone so always look there.
                {
                    Server_CardZone *spellZone = ruledCanonicalStackZone(game);
                    Server_Card *phys =
                        spellZone ? ruledPhysicalSpellOnCanonicalStack(spellZone, normalizedPushedName) : nullptr;
                    if (phys) {
                        ruledStackObjectIdToServerCardId.insert(pushedOid, phys->getId());
                    }
                    RULED_TRACE("relay") << "stackPushed: oid=" << pushedOid
                                         << " cardId=" << QString::fromStdString(sp.card_id()) << " name='"
                                         << pushedName << "'"
                                         << " stackZoneSize=" << (spellZone ? spellZone->getCards().size() : -1)
                                         << " physicalCardOnStack=" << (phys ? phys->getId() : -1)
                                         << " boundServerCardId="
                                         << ruledStackObjectIdToServerCardId.value(pushedOid, -1);
                }
            }
            if (e.has_stack_resolved()) {
                applyStackResolvedEvent(e.stack_resolved(), battlefieldGridRows, battlefieldDisplayPlayers);
            }
            if (e.has_stack_object_countered()) {
                applyStackObjectCounteredEvent(e.stack_object_countered());
            }
            continue;
        }
        const ruled::v1::ZoneViewSync physicalView = physicalBattlefieldZoneView(e.zone_view());
        applyBattlefieldControllerTransfers(physicalView, result);
        for (const auto &p : physicalView.per_player()) {
            // Untap-step "reset" applies only to the active player's view; NAP may stay tapped.
            // Every other legitimate mid-turn untap arrives as an explicit PermanentsUntapped oid
            // (CR 605 mana undo, untap effects), which the binding honors per-object.
            const bool perPlayerAllowUntap = batchHasUntapPhase && p.player_id() == game->getActivePlayer();
            if (Server_AbstractPlayer *ab = game->getPlayer(p.player_id())) {
                const RuledPlayerBinding::RuledZoneSyncResult sync =
                    playerBinding(p.player_id())
                        .applyRuledEngineZoneView(static_cast<Server_Player *>(ab), p, &tapSyncGes, perPlayerAllowUntap,
                                                  &engineUntappedOids, e.zone_view().battlefields_unchanged());
                result.handOrLibraryChanged = result.handOrLibraryChanged || sync.handOrLibraryChanged;
                result.battlefieldOrderChanged = result.battlefieldOrderChanged || sync.battlefieldOrderChanged;
                result.publicZoneOrderChanged = result.publicZoneOrderChanged || sync.publicZoneOrderChanged;
                result.tapStateEventsQueued = result.tapStateEventsQueued || sync.tapStateChanged;
                result.zoneViewApplied = true;
            }
        }
    }
    if (result.tapStateEventsQueued) {
        tapSyncGes.sendToGame(game);
    }
}

Server_Card *RuledBatchSynchronizer::findBattlefieldCardByEngineOid(quint32 oid, int preferredControllerId)
{
    const auto findForPlayer = [this, oid](int playerId) -> Server_Card * {
        Server_AbstractPlayer *abstractPlayer = game->getPlayer(playerId);
        if (!abstractPlayer) {
            return nullptr;
        }
        auto *player = static_cast<Server_Player *>(abstractPlayer);
        return playerBinding(playerId).findCardByEngineOid(player, oid);
    };
    if (preferredControllerId >= 0) {
        if (Server_Card *card = findForPlayer(preferredControllerId)) {
            return card;
        }
    }
    for (Server_AbstractPlayer *abstractPlayer : game->getPlayers().values()) {
        if (!abstractPlayer || abstractPlayer->getPlayerId() == preferredControllerId) {
            continue;
        }
        if (Server_Card *card = findForPlayer(abstractPlayer->getPlayerId())) {
            return card;
        }
    }
    return nullptr;
}

void RuledBatchSynchronizer::applyFaceDisplays(const ruled::v1::RuledEventBatch &batch, BatchApplyResult &result)
{
    const auto applyName = [this, &result](quint32 oid, int controllerId, const QString &cardId, int faceIndex,
                                           const QString &effectiveDisplayName) {
        Server_Card *card = findBattlefieldCardByEngineOid(oid, controllerId);
        if (!card) {
            return;
        }
        const QString resolvedCardId = cardId.isEmpty() ? cardIdForName(card->getName()) : cardId;
        const QString activeName =
            effectiveDisplayName.isEmpty() ? faceDisplayName(resolvedCardId, faceIndex) : effectiveDisplayName;
        if (!activeName.isEmpty() && activeName != card->getName()) {
            card->setCardRef(CardRef{activeName});
            result.battlefieldDisplayChanged = true;
        }
    };

    for (const auto &event : batch.events()) {
        if (event.has_face_changed()) {
            const auto &changed = event.face_changed();
            applyName(static_cast<quint32>(changed.object_id()), changed.controller_player_id(), QString(),
                      static_cast<int>(changed.face_up_index()), QString());
        }
        if (!event.has_zone_view() || event.zone_view().battlefields_unchanged()) {
            continue;
        }
        for (const auto &view : event.zone_view().per_player()) {
            for (const auto &object : view.battlefield_objects()) {
                // Server_Card retains the underlying physical identity while its face-down flag
                // makes the public wire event anonymous. Controller-only display comes from the
                // FaceDownObjectMap; never overwrite the shared CardRef with the generic 2/2.
                if (object.face_down()) {
                    continue;
                }
                applyName(static_cast<quint32>(object.object_id()), view.player_id(),
                          QString::fromStdString(object.card_id()), static_cast<int>(object.face_up_index()),
                          QString::fromStdString(object.effective_display_name()));
            }
        }
    }
}

// Post-zone-view pass: restore physical object attachments from the engine's typed recipient.
// Player-attached Auras remain unparented in their controller's ordinary battlefield row; their
// public identity is rendered through the replaceable Enchanting annotation. Missing recipients
// clear stale physical parents. The full replacement handles reconnect and both transition
// directions; object recipients still emit Event_AttachCard for the legacy stacked presentation.
void RuledBatchSynchronizer::applyAttachmentRestores(const ruled::v1::RuledEventBatch &batch)
{
    {
        GameEventStorage attachRestoreGes;
        bool attachRestoreGesHasEvents = false;
        for (int ei = 0; ei < batch.events_size(); ++ei) {
            const auto &e = batch.events(ei);
            if (!e.has_zone_view()) {
                continue;
            }
            for (const auto &p : e.zone_view().per_player()) {
                if (p.battlefield_objects_size() == 0) {
                    continue;
                }
                Server_AbstractPlayer *ownerAb = game->getPlayer(p.player_id());
                if (!ownerAb) {
                    continue;
                }
                auto *ownerPlayer = static_cast<Server_Player *>(ownerAb);
                for (const auto &battlefieldObject : p.battlefield_objects()) {
                    const quint32 attachedOid = static_cast<quint32>(battlefieldObject.object_id());
                    Server_Card *attachedCard =
                        playerBinding(p.player_id()).findCardByEngineOid(ownerPlayer, attachedOid);
                    if (!attachedCard || !attachedCard->getZone()) {
                        continue;
                    }
                    const bool hasObjectRecipient = battlefieldObject.has_attachment_recipient() &&
                                                    battlefieldObject.attachment_recipient().recipient_case() ==
                                                        ruled::v1::AttachmentRecipient::kObjectId;
                    if (!hasObjectRecipient) {
                        if (attachedCard->getParentCard()) {
                            playerBinding(p.player_id()).unattachRuledCard(ownerPlayer, attachedCard, attachRestoreGes);
                            attachRestoreGesHasEvents = true;
                        }
                        continue;
                    }
                    const quint32 targetOid =
                        static_cast<quint32>(battlefieldObject.attachment_recipient().object_id());
                    Server_Card *targetCard = nullptr;
                    for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
                        if (!ab) {
                            continue;
                        }
                        targetCard = playerBinding(ab->getPlayerId())
                                         .findCardByEngineOid(static_cast<Server_Player *>(ab), targetOid);
                        if (targetCard) {
                            break;
                        }
                    }
                    if (!targetCard || !targetCard->getZone()) {
                        if (attachedCard->getParentCard()) {
                            playerBinding(p.player_id()).unattachRuledCard(ownerPlayer, attachedCard, attachRestoreGes);
                            attachRestoreGesHasEvents = true;
                        }
                        continue;
                    }
                    // Avoid redundant events when the server already knows about this attachment.
                    if (attachedCard->getParentCard() == targetCard) {
                        continue;
                    }
                    if (attachedCard->getParentCard()) {
                        playerBinding(p.player_id()).unattachRuledCard(ownerPlayer, attachedCard, attachRestoreGes);
                        attachRestoreGesHasEvents = true;
                    }
                    attachedCard->setParentCard(targetCard);
                    // Match cmdAttachCard: an attached card leaves the grid (x = -1) and is drawn
                    // against its parent.
                    const int attachedOldX = attachedCard->getX();
                    attachedCard->setCoords(-1, attachedCard->getY());
                    attachedCard->getZone()->updateCardCoordinates(attachedCard, attachedOldX, attachedCard->getY());
                    Event_AttachCard attachEv;
                    attachEv.set_start_zone(attachedCard->getZone()->getName().toStdString());
                    attachEv.set_card_id(attachedCard->getId());
                    attachEv.set_target_player_id(targetCard->getZone()->getPlayer()->getPlayerId());
                    attachEv.set_target_zone(targetCard->getZone()->getName().toStdString());
                    attachEv.set_target_card_id(targetCard->getId());
                    attachRestoreGes.enqueueGameEvent(attachEv, attachedCard->getZone()->getPlayer()->getPlayerId());
                    attachRestoreGesHasEvents = true;
                }
            }
        }
        if (attachRestoreGesHasEvents) {
            attachRestoreGes.sendToGame(game);
        }
    }
}

// Combat-related events that depend on the engine OID map (LifeChanged,
// AttackersDeclared) and stack resolution side effects that synthesize standard
// Cockatrice events for clients. PermanentMoved is handled earlier (before zone_view).
void RuledBatchSynchronizer::applyLifeManaAndCombatEvents(const ruled::v1::RuledEventBatch &batch)
{
    GameEventStorage combatGes;
    bool combatGesHasEvents = false;
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (e.has_face_changed()) {
            const auto &changed = e.face_changed();
            Server_Card *card = findBattlefieldCardByEngineOid(static_cast<quint32>(changed.object_id()),
                                                               changed.controller_player_id());
            if (card && card->getZone()) {
                card->setFaceDown(changed.face_down());
                // A face-up change publishes both state and display identity atomically. Merely
                // sending AttrFaceDown=0 leaves the existing client CardItem with its anonymous
                // face-down CardRef until a later full game-state refresh.
                Event_FlipCard faceEv;
                faceEv.set_zone_name(card->getZone()->getName().toStdString());
                faceEv.set_card_id(card->getId());
                faceEv.set_face_down(changed.face_down());
                if (!changed.face_down()) {
                    faceEv.set_card_name(card->getName().toStdString());
                    faceEv.set_card_provider_id(card->getProviderId().toStdString());
                }
                combatGes.enqueueGameEvent(faceEv, card->getZone()->getPlayer()->getPlayerId());
                combatGesHasEvents = true;
            }
        }
        if (e.has_life_changed()) {
            const auto &lc = e.life_changed();
            Server_AbstractPlayer *target = game->getPlayer(lc.player_id());
            if (!target) {
                continue;
            }
            auto *targetPlayer = static_cast<Server_Player *>(target);
            // Life is stored on the per-player counter id 0 ("life"). Update both server
            // state and broadcast a SetCounter event so clients render the change.
            const auto &allCounters = targetPlayer->getCounters();
            Server_Counter *lifeCounter = allCounters.value(0, nullptr);
            if (!lifeCounter || lifeCounter->getName() != QStringLiteral("life")) {
                // Fall back: search by name. Counter ids are stable in practice but be defensive.
                for (Server_Counter *c : allCounters) {
                    if (c && c->getName() == QStringLiteral("life")) {
                        lifeCounter = c;
                        break;
                    }
                }
            }
            if (!lifeCounter) {
                continue;
            }
            lifeCounter->setCount(lc.new_total());
            Event_SetCounter ev;
            ev.set_counter_id(lifeCounter->getId());
            ev.set_value(lifeCounter->getCount());
            combatGes.enqueueGameEvent(ev, target->getPlayerId());
            combatGesHasEvents = true;
        }
        if (e.has_mana_pool_updated()) {
            const auto &mp = e.mana_pool_updated();
            Server_AbstractPlayer *target = game->getPlayer(mp.player_id());
            if (!target) {
                continue;
            }
            auto *targetPlayer = static_cast<Server_Player *>(target);
            // CR 106: the engine is the sole owner of the mana pool. Mirror its absolute snapshot
            // onto the player's single-letter mana counters (w/u/b/r/g/x). Cockatrice's existing
            // upper general counter is named "x" and displayed as Colorless; the lower one is the
            // unrelated storm counter. Because the snapshot is absolute,
            // this one handler covers production (mana abilities), payment (pay_mana), and the
            // empty-on-step/phase-change case — so no separate server-side pool clear is needed.
            const QHash<QString, int> desired = {
                {QStringLiteral("w"), static_cast<int>(mp.w())}, {QStringLiteral("u"), static_cast<int>(mp.u())},
                {QStringLiteral("b"), static_cast<int>(mp.b())}, {QStringLiteral("r"), static_cast<int>(mp.r())},
                {QStringLiteral("g"), static_cast<int>(mp.g())}, {QStringLiteral("x"), static_cast<int>(mp.c())},
            };
            for (Server_Counter *counter : targetPlayer->getCounters()) {
                if (!counter) {
                    continue;
                }
                const auto it = desired.constFind(counter->getName().trimmed().toLower());
                if (it == desired.constEnd() || counter->getCount() == it.value()) {
                    continue;
                }
                counter->setCount(it.value());
                Event_SetCounter ev;
                ev.set_counter_id(counter->getId());
                ev.set_value(it.value());
                combatGes.enqueueGameEvent(ev, target->getPlayerId());
                combatGesHasEvents = true;
            }
        }
        if (e.has_attackers_declared()) {
            const auto &ad = e.attackers_declared();
            Server_AbstractPlayer *attacker = game->getPlayer(ad.attacking_player_id());
            if (!attacker) {
                continue;
            }
            auto *attackerPlayer = static_cast<Server_Player *>(attacker);
            Server_CardZone *tableZone = attackerPlayer->getZones().value(ZoneNames::TABLE);
            if (tableZone) {
                for (Server_Card *card : tableZone->getCards()) {
                    if (!card || !card->getAttacking()) {
                        continue;
                    }
                    card->setAttacking(false);
                    Event_SetCardAttr clearEv;
                    clearEv.set_zone_name(std::string(ZoneNames::TABLE));
                    clearEv.set_card_id(card->getId());
                    clearEv.set_attribute(AttrAttacking);
                    clearEv.set_attr_value("0");
                    combatGes.enqueueGameEvent(clearEv, attacker->getPlayerId());
                    combatGesHasEvents = true;
                }
            }
            for (const auto &assignment : ad.assignments()) {
                const quint32 oid = static_cast<quint32>(assignment.attacker_object_id());
                Server_Card *card = playerBinding(ad.attacking_player_id()).findCardByEngineOid(attackerPlayer, oid);
                if (!card) {
                    continue;
                }
                card->setAttacking(true);
                Event_SetCardAttr attEv;
                attEv.set_zone_name(std::string(ZoneNames::TABLE));
                attEv.set_card_id(card->getId());
                attEv.set_attribute(AttrAttacking);
                attEv.set_attr_value("1");
                combatGes.enqueueGameEvent(attEv, attacker->getPlayerId());
                combatGesHasEvents = true;
            }
        }
        if (e.has_attackers_added()) {
            for (const auto &assignment : e.attackers_added().assignments()) {
                const quint32 oid = static_cast<quint32>(assignment.attacker_object_id());
                for (Server_AbstractPlayer *candidate : game->getPlayers().values()) {
                    if (!candidate) {
                        continue;
                    }
                    auto *candidatePlayer = static_cast<Server_Player *>(candidate);
                    Server_Card *card =
                        playerBinding(candidatePlayer->getPlayerId()).findCardByEngineOid(candidatePlayer, oid);
                    if (!card) {
                        continue;
                    }
                    card->setAttacking(true);
                    Event_SetCardAttr attackingEvent;
                    attackingEvent.set_zone_name(std::string(ZoneNames::TABLE));
                    attackingEvent.set_card_id(card->getId());
                    attackingEvent.set_attribute(AttrAttacking);
                    attackingEvent.set_attr_value("1");
                    combatGes.enqueueGameEvent(attackingEvent, candidatePlayer->getPlayerId());
                    combatGesHasEvents = true;
                    break;
                }
            }
        }
        if (e.has_stack_resolved()) {
            const quint32 resolvedOid = static_cast<quint32>(e.stack_resolved().object_id());
            // A countered spell leaves the stack for its owner's graveyard via a PermanentMoved
            // event the engine emits (handled generically in the first pass, above) — no per-card
            // special-case here. Just retire this stack object's bookkeeping.
            ruledStackTargetsByObjectId.remove(resolvedOid);
            ruledStackObjectIdToServerCardId.remove(resolvedOid);
            ruledStackObjectIdToCasterPlayerId.remove(resolvedOid);
            ruledEngineStackPushDescriptionsByObjectId.remove(resolvedOid);
        }
        // CR 303.4: an aura entering the battlefield attaches to its enchant target. Translate the
        // engine AuraAttached event into Event_AttachCard so the Cockatrice client stacks the aura
        // underneath the enchanted permanent (the visual layout used in freeform too).
        if (e.has_aura_attached()) {
            const auto &aa = e.aura_attached();
            if (!aa.has_attachment_recipient() ||
                aa.attachment_recipient().recipient_case() != ruled::v1::AttachmentRecipient::kObjectId) {
                continue;
            }
            const quint32 auraOid = static_cast<quint32>(aa.aura_object_id());
            const quint32 enchantedOid = static_cast<quint32>(aa.attachment_recipient().object_id());
            Server_Card *auraCard = nullptr;
            Server_Card *enchantedCard = nullptr;
            for (Server_AbstractPlayer *ab : game->getPlayers().values()) {
                if (!ab) {
                    continue;
                }
                auto *pl = static_cast<Server_Player *>(ab);
                RuledPlayerBinding &binding = playerBinding(pl->getPlayerId());
                if (!auraCard) {
                    auraCard = binding.findCardByEngineOid(pl, auraOid);
                }
                if (!enchantedCard) {
                    enchantedCard = binding.findCardByEngineOid(pl, enchantedOid);
                }
            }
            // Skip when the server already has this attachment: a repeated AuraAttached (e.g. in a
            // reconnect's initial batch) would otherwise re-broadcast Event_AttachCard and make the
            // client replay the attach.
            if (auraCard && enchantedCard && auraCard->getZone() && auraCard->getParentCard() != enchantedCard) {
                auraCard->setParentCard(enchantedCard);
                // Match cmdAttachCard: an attached card leaves the grid (x = -1) and is drawn
                // against its parent. Without this it keeps a grid column of its own.
                const int auraOldX = auraCard->getX();
                auraCard->setCoords(-1, auraCard->getY());
                auraCard->getZone()->updateCardCoordinates(auraCard, auraOldX, auraCard->getY());
                Event_AttachCard attachEv;
                attachEv.set_start_zone(auraCard->getZone()->getName().toStdString());
                attachEv.set_card_id(auraCard->getId());
                if (enchantedCard->getZone()) {
                    attachEv.set_target_player_id(enchantedCard->getZone()->getPlayer()->getPlayerId());
                    attachEv.set_target_zone(enchantedCard->getZone()->getName().toStdString());
                    attachEv.set_target_card_id(enchantedCard->getId());
                }
                combatGes.enqueueGameEvent(attachEv, auraCard->getZone()->getPlayer()->getPlayerId());
                combatGesHasEvents = true;
            }
        }
    }
    if (combatGesHasEvents) {
        combatGes.sendToGame(game);
    }
}

void RuledBatchSynchronizer::revealFaceDownPermanentsOnConcede(int concedingPlayerId, GameEventStorage &ges)
{
    int remainingPlayers = 0;
    for (Server_AbstractPlayer *player : game->getPlayers().values()) {
        if (player && !player->getConceded()) {
            ++remainingPlayers;
        }
    }
    const bool gameEnding = remainingPlayers <= 1;
    for (Server_AbstractPlayer *player : game->getPlayers().values()) {
        if (!player || (!gameEnding && player->getPlayerId() != concedingPlayerId)) {
            continue;
        }
        Server_CardZone *table = player->getZones().value(ZoneNames::TABLE);
        if (!table) {
            continue;
        }
        for (Server_Card *card : table->getCards()) {
            if (!card || !card->getFaceDown()) {
                continue;
            }
            card->setFaceDown(false);
            Event_SetCardAttr event;
            event.set_zone_name(std::string(ZoneNames::TABLE));
            event.set_card_id(card->getId());
            event.set_attribute(AttrFaceDown);
            event.set_attr_value("0");
            ges.enqueueGameEvent(event, player->getPlayerId());
        }
    }
}

QString RuledBatchSynchronizer::cardIdForName(const QString &cardName) const
{
    return ruledCardIdByLowerName.value(cardName.trimmed().toLower());
}

QString RuledBatchSynchronizer::cardNameForId(const QString &cardId) const
{
    const auto it = ruledCardCatalogById.constFind(cardId);
    return it == ruledCardCatalogById.constEnd() ? QString() : QString::fromStdString(it->name());
}

QString RuledBatchSynchronizer::faceDisplayName(const QString &cardId, int faceIndex) const
{
    const auto it = ruledCardCatalogById.constFind(cardId);
    if (it == ruledCardCatalogById.constEnd()) {
        return QString();
    }
    if (faceIndex >= 0 && faceIndex < it->face_display_names_size()) {
        return QString::fromStdString(it->face_display_names(faceIndex));
    }
    return QString::fromStdString(it->name());
}

// Index every CardCatalog event in `batch` into the name/id lookups the zone reconcile resolves
// physical cards through. Returns true if the batch carried a catalog at all.
//
// A catalog event always carries the whole catalog, so a batch that has one fully replaces the
// index; a batch with none leaves it untouched. That distinction is why the clear is inside the
// loop rather than above it — most batches carry no catalog and must not wipe the index.
bool RuledBatchSynchronizer::indexCardCatalogEvents(const ruled::v1::RuledEventBatch &batch)
{
    bool sawCatalog = false;
    for (int ei = 0; ei < batch.events_size(); ++ei) {
        const auto &e = batch.events(ei);
        if (!e.has_card_catalog()) {
            continue;
        }
        if (!sawCatalog) {
            ruledCardCatalogById.clear();
            ruledCardIdByLowerName.clear();
            sawCatalog = true;
        }
        for (const auto &entry : e.card_catalog().entries()) {
            const QString cardId = QString::fromStdString(entry.card_id());
            ruledCardCatalogById.insert(cardId, entry);
            ruledCardIdByLowerName.insert(QString::fromStdString(entry.name()).trimmed().toLower(), cardId);
            // CR 709/712/715: a multi-face card also resolves to its id by either face's Oracle
            // name (e.g. "Fire" / "Ice" -> "fire_ice"), mirroring the engine's name index.
            for (const auto &faceName : entry.face_names()) {
                ruledCardIdByLowerName.insert(QString::fromStdString(faceName).trimmed().toLower(), cardId);
            }
        }
    }
    return sawCatalog;
}

void RuledBatchSynchronizer::applyStartupBatch(const ruled::v1::IpcResponse &resp,
                                               const QList<QPair<int, QStringList>> &deckByPlayer)
{
    if (!resp.has_batch()) {
        return;
    }

    // The catalog must be indexed before any zone-view application below: syncing
    // physical zones resolves card names through it.
    indexCardCatalogEvents(resp.batch());
    if (ruledCardCatalogById.isEmpty()) {
        qWarning() << "applyStartupBatch: no CardCatalog in startup batch — "
                      "is tricerules-server rebuilt from this tree? Zone sync will not resolve names.";
    }

    int startupActivePlayer = -1;
    int startupMappedPhase = -1;
    int startupPriorityPlayer = -1;
    bool startupZoneViewApplied = false;
    for (int ei = 0; ei < resp.batch().events_size(); ++ei) {
        const auto &e = resp.batch().events(ei);
        if (e.has_phase_changed()) {
            startupActivePlayer = e.phase_changed().active_player_id();
            startupMappedPhase = ruledPhaseToCockatricePhase(e.phase_changed().phase_id());
        }
        if (e.has_priority_changed()) {
            startupPriorityPlayer = e.priority_changed().player_id();
        }
        if (e.has_zone_view() && !startupZoneViewApplied) {
            const auto &z = e.zone_view();
            for (int pi = 0; pi < z.per_player_size(); ++pi) {
                const auto &p = z.per_player(pi);
                const int mainN = expectedMainboardSizeForStartupSync(game, p.player_id(), deckByPlayer);
                const int needLib = mainN - p.hand_cards_size();
                const int libCount = p.library_cards_size();
                // The startup view is what seeds each player's physical deck and hand, so the
                // engine never marks it unchanged (its per-session cache starts empty). Seeing
                // the flag here means a version-skewed sidecar; treat it exactly like a count
                // mismatch rather than starting a ruled game on unseeded zones.
                if (z.battlefields_unchanged() || p.private_zones_unchanged() || libCount != needLib) {
                    qWarning() << "Ruled zone sync: player" << p.player_id() << "expected" << needLib
                               << "library cards, library_cards has" << libCount
                               << "entries — is tricerules-server up to date? "
                                  "(RulesRelay read was fixed; rebuild + restart the Rust side from this repo.)";
                    for (Server_AbstractPlayer *pl : game->getPlayers().values()) {
                        shuffleMainDeckForRuledFallback(pl);
                    }
                    session->abort();
                    return;
                }
            }
            const ruled::v1::ZoneViewSync physicalView = physicalBattlefieldZoneView(e.zone_view());
            BatchApplyResult startupResult;
            applyBattlefieldControllerTransfers(physicalView, startupResult);
            for (const auto &p : physicalView.per_player()) {
                if (Server_AbstractPlayer *ab = game->getPlayer(p.player_id())) {
                    playerBinding(p.player_id())
                        .applyRuledEngineZoneView(static_cast<Server_Player *>(ab), p, nullptr, true, nullptr,
                                                  e.zone_view().battlefields_unchanged());
                }
            }
            startupZoneViewApplied = true;
        }
    }
    if (startupActivePlayer >= 0 && game->getActivePlayer() != startupActivePlayer) {
        game->setActivePlayer(startupActivePlayer);
    }
    if (startupMappedPhase >= 0 && game->getActivePhase() != startupMappedPhase) {
        game->setActivePhase(startupMappedPhase);
    }
    if (startupPriorityPlayer >= 0) {
        ruledPriorityPlayer = startupPriorityPlayer;
    }
}
