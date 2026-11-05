#include "ruled_public_zone_order.h"

#include <libcockatrice/game/board/card_state.h>
#include <libcockatrice/game/zones/card_zone_logic.h>
#include <libcockatrice/game/zones/view_zone_logic.h>

#include "ruled_public_zone_order_plan.h"

#include <QPointer>

void RuledPublicZoneOrder::apply(CardZoneLogic *zone, const ServerInfo_Zone &info)
{
    if (!zone) {
        return;
    }
    const auto ordered =
        RuledPublicZoneOrderDetail::snapshot(true, zone->contentsKnown(), zone->getName(), zone->getCards(), info);
    if (!ordered) {
        return;
    }
    QList<int> ids;
    for (CardState *card : *ordered) {
        ids.append(card->getId());
    }
    struct ViewUpdate
    {
        QPointer<ZoneViewZoneLogic> logic;
        std::optional<QList<CardState *>> ordered;
        bool changed;
    };
    QList<ViewUpdate> updates;
    for (ZoneViewZoneLogic *logic : zone->getViews()) {
        if (!logic || logic->getOriginalZone() != zone ||
            (logic->getRevealZone() && !logic->getWriteableRevealZone())) {
            continue;
        }
        auto mirror = RuledPublicZoneOrderDetail::view(logic->getCards(), ids, logic->getNumberCards());
        const bool changed = mirror && *mirror != logic->getCards();
        updates.append({logic, std::move(mirror), changed});
    }
    const bool changed = *ordered != zone->getCards();
    // Commit every list before emitting layout signals. Membership, card state and counts stay intact.
    for (qsizetype i = 0; i < ordered->size(); ++i) {
        zone->cards[i] = ordered->at(i);
    }
    for (const auto &update : updates) {
        if (update.logic && update.ordered) {
            for (qsizetype i = 0; i < update.ordered->size(); ++i) {
                update.logic->cards[i] = update.ordered->at(i);
            }
        }
    }
    if (changed) {
        emit zone->reorganizeCards();
        emit zone->updateGraphics();
    }
    for (const auto &update : updates) {
        if (!update.logic) {
            continue;
        }
        if (!update.ordered) {
            emit update.logic->closeView();
        } else if (update.changed) {
            emit update.logic->reorganizeCards();
            emit update.logic->updateGraphics();
        }
    }
}
