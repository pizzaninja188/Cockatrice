#ifndef COCKATRICE_RULED_PUBLIC_ZONE_ORDER_PLAN_H
#define COCKATRICE_RULED_PUBLIC_ZONE_ORDER_PLAN_H

#include <QHash>
#include <QList>
#include <QString>
#include <algorithm>
#include <libcockatrice/protocol/pb/serverinfo_zone.pb.h>
#include <libcockatrice/utility/zone_names.h>
#include <optional>

namespace RuledPublicZoneOrderDetail
{
template <typename Card> std::optional<QList<Card *>> permutation(const QList<Card *> &cards, const QList<int> &ids)
{
    if (cards.size() != ids.size()) {
        return std::nullopt;
    }
    QHash<int, Card *> remaining;
    for (Card *card : cards) {
        if (!card || remaining.contains(card->getId())) {
            return std::nullopt;
        }
        remaining.insert(card->getId(), card);
    }
    QList<Card *> ordered;
    for (int id : ids) {
        Card *card = remaining.take(id);
        if (!card) {
            return std::nullopt;
        }
        ordered.append(card);
    }
    return ordered;
}

template <typename Card>
std::optional<QList<Card *>> snapshot(bool ruled,
                                      bool contentsKnown,
                                      const QString &zoneName,
                                      const QList<Card *> &cards,
                                      const ServerInfo_Zone &info)
{
    if (!ruled || !contentsKnown || zoneName != QLatin1String(ZoneNames::GRAVE) || info.name() != ZoneNames::GRAVE ||
        info.type() != ServerInfo_Zone::PublicZone || info.with_coords() || !info.has_card_count() ||
        info.card_count() != info.card_list_size() || info.card_count() != cards.size()) {
        return std::nullopt;
    }
    QList<int> ids;
    for (const auto &card : info.card_list()) {
        if (!card.has_id()) {
            return std::nullopt;
        }
        ids.append(card.id());
    }
    // Physical snapshots already use newest-first order; card x is not a pile position.
    return permutation(cards, ids);
}

template <typename Card>
std::optional<QList<Card *>> view(const QList<Card *> &cards, const QList<int> &originIds, int numberCards)
{
    if (numberCards < -1) {
        return std::nullopt;
    }
    const auto count = numberCards == -1 ? originIds.size() : std::min(qsizetype(numberCards), originIds.size());
    // Known public views display the origin's prefix, including reversed views.
    // A changed visible membership invalidates the window; never synthesize physical cards.
    return permutation(cards, originIds.mid(0, count));
}
} // namespace RuledPublicZoneOrderDetail

#endif
