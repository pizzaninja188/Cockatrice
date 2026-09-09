#include "ruled_preparation_display.h"
#include "ruled_actions.h"
#include "../abstract_game.h"
#include "../board/card_item.h"
#include "../player/player.h"
#include "../player/player_manager.h"
#include "../zones/logic/card_zone_logic.h"
#include "../zones/view_zone.h"
#include <QSet>
#include <libcockatrice/utility/zone_names.h>

RuledPreparationDisplay::RuledPreparationDisplay(AbstractGame *game, QObject *parent)
    : QObject(parent), game(game)
{
}

void RuledPreparationDisplay::remove(quint32 oid)
{
    CardItem *card = displayed.take(oid);
    if (!card) {
        return;
    }
    if (auto *zone = card->getZone()) {
        const int position = zone->getCards().indexOf(card);
        if (position >= 0) {
            // Preserve an open view across a full replacement, including an empty snapshot.
            zone->takeCard(position, card->getId(), false);
        }
    }
    card->deleteLater();
}

void RuledPreparationDisplay::reconcile(const QVector<RuledClientHost::PreparationCopy> &copies)
{
    QSet<quint32> desired;
    for (const auto &copy : copies) {
        desired.insert(copy.objectId);
    }
    for (const auto oid : displayed.keys()) {
        if (!desired.contains(oid)) {
            remove(oid);
        }
    }
    if (!RuledActions::isRuledGame(game)) {
        return;
    }
    for (const auto &copy : copies) {
        Player *player = game->getPlayerManager()->getPlayers().value(copy.playerId, nullptr);
        CardZoneLogic *zone = player ? player->getZone(QLatin1String(ZoneNames::EXILE)) : nullptr;
        if (!zone) {
            continue;
        }
        CardItem *card = displayed.value(copy.objectId);
        if (card && (card->getZone() != zone || card->getId() != copy.serverCardId ||
                     !zone->getCards().contains(card))) {
            remove(copy.objectId);
            card = nullptr;
        }
        if (!card) {
            // A restored physical snapshot may already have installed this exact binding.
            card = zone->getCards().findCard(copy.serverCardId);
            if (!card) {
                card = new CardItem(player, nullptr, CardRef{copy.displayName}, copy.serverCardId);
                zone->addCard(card, true, qBound(0, copy.zoneIndex, static_cast<int>(zone->getCards().size())));
            }
            displayed.insert(copy.objectId, card);
        }
        card->setCardRef(CardRef{copy.displayName});
        card->setAnnotation(QStringLiteral("Prepare spell copy"));
        card->setDestroyOnZoneChange(true);
        for (auto *view : zone->getViews()) {
            if (CardItem *mirror = view->getLogic()->getCards().findCard(copy.serverCardId)) {
                mirror->setCardRef(card->getCardRef());
                copyViewState(card, mirror);
            }
        }
    }
}

void RuledPreparationDisplay::copyViewState(const CardItem *source, CardItem *view)
{
    if (RuledActions::isRuledGameForCard(source) && source->getZone() &&
        source->getZone()->getName() == QLatin1String(ZoneNames::EXILE)) {
        view->setAnnotation(source->getAnnotation());
        view->setDestroyOnZoneChange(source->getDestroyOnZoneChange());
    }
}
