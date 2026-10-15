#include "ruled_reveal_state.h"

#include <QSignalBlocker>
#include <algorithm>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

namespace
{

bool matchesReceipt(const RuledRevealState::Entry &entry, const ruled::v1::CardsRevealed &reveal)
{
    if (entry.id != QString::fromStdString(reveal.reveal_id()) || entry.owner != reveal.zone_owner_player_id() ||
        entry.sourceZone != reveal.source_zone() || entry.sourceObjectId != reveal.source_object_id() ||
        entry.sourceDescription != QString::fromStdString(reveal.source_description()) ||
        entry.cards.size() != reveal.cards_size()) {
        return false;
    }
    for (int i = 0; i < reveal.cards_size(); ++i) {
        const auto &card = reveal.cards(i);
        if (entry.cards[i] != RuledRevealState::Card{card.object_id(), card.zone_change_generation(),
                                                     QString::fromStdString(card.card_id()),
                                                     QString::fromStdString(card.card_name())}) {
            return false;
        }
    }
    return true;
}

} // namespace

QStringList RuledRevealState::Entry::cardNames() const
{
    QStringList names;
    for (const auto &card : cards)
        names.append(card.name);
    return names;
}

bool RuledRevealState::Entry::hasInteractiveChoice() const
{
    return phase == Phase::Choice && choiceCardIds.size() == cards.size() &&
           std::any_of(choiceCardIds.cbegin(), choiceCardIds.cend(), [](int cardId) { return cardId >= 0; });
}

int RuledRevealState::Entry::choiceCardIdForIndex(int revealCardIndex) const
{
    if (!hasInteractiveChoice() || revealCardIndex < 0 || revealCardIndex >= choiceCardIds.size())
        return -1;
    return choiceCardIds[revealCardIndex];
}

const RuledRevealState::Entry *RuledRevealState::choice() const
{
    const auto it = entries_.constFind(choiceId_);
    return it == entries_.cend() ? nullptr : &it.value();
}

bool RuledRevealState::publish(const ruled::v1::CardsRevealed &reveal)
{
    const QString id = QString::fromStdString(reveal.reveal_id());
    if (id.isEmpty() || reveal.cards_size() == 0 || seen_.contains(id))
        return false;
    Entry entry;
    entry.id = id;
    entry.owner = reveal.zone_owner_player_id();
    entry.sourceZone = reveal.source_zone();
    entry.sourceObjectId = reveal.source_object_id();
    entry.sourceDescription = QString::fromStdString(reveal.source_description());
    for (const auto &card : reveal.cards()) {
        if (card.card_name().empty() || card.card_id().empty())
            return false;
        entry.cards.append({card.object_id(), card.zone_change_generation(), QString::fromStdString(card.card_id()),
                            QString::fromStdString(card.card_name())});
    }
    entries_.insert(id, std::move(entry));
    order_.append(id);
    seen_.insert(id);
    emit changed();
    return true;
}

bool RuledRevealState::matchesReceipt(const ruled::v1::CardsRevealed &reveal) const
{
    const auto it = entries_.constFind(QString::fromStdString(reveal.reveal_id()));
    return it != entries_.cend() && ::matchesReceipt(it.value(), reveal);
}

bool RuledRevealState::beginChoice(const ruled::v1::CardsRevealed &reveal, const QVector<int> &choiceCardIds)
{
    const QString id = QString::fromStdString(reveal.reveal_id());
    const auto existing = entries_.constFind(id);
    if (existing != entries_.cend() && !::matchesReceipt(existing.value(), reveal))
        return false;
    QSignalBlocker block(this);
    const bool replaced = id != choiceId_;
    if (id != choiceId_)
        completeChoice();
    const bool added = publish(reveal);
    auto it = entries_.find(id);
    if (it == entries_.end())
        return false;
    // A new validated offer may continue a retained reveal that was completed in an earlier
    // prompt. Only an authoritative active-snapshot omission retires the old occurrence.
    retired_.remove(id);
    if (it->phase != Phase::Choice || it->choiceCardIds != choiceCardIds) {
        it->phase = Phase::Choice;
        it->choiceCardIds = choiceCardIds;
        choiceId_ = id;
        block.unblock();
        emit changed();
    } else if (replaced || added) {
        block.unblock();
        emit changed();
    }
    return true;
}

void RuledRevealState::completeChoice()
{
    if (choiceId_.isEmpty())
        return;
    auto it = entries_.find(choiceId_);
    if (it != entries_.end()) {
        it->phase = active_.contains(choiceId_) ? Phase::Active : Phase::Completed;
        it->choiceCardIds.clear();
    }
    choiceId_.clear();
    emit changed();
}

void RuledRevealState::applyPublicHands(const ruled::v1::ZoneViewSync &snapshot)
{
    QMap<int, QVector<Card>> next;
    for (const auto &player : snapshot.per_player()) {
        if (!player.has_public_hand())
            continue;
        QVector<Card> cards;
        cards.reserve(player.public_hand().cards_size());
        for (const auto &card : player.public_hand().cards()) {
            if (card.card_id().empty() || card.card_name().empty())
                continue;
            cards.append({card.object_id(), card.zone_change_generation(), QString::fromStdString(card.card_id()),
                          QString::fromStdString(card.card_name())});
        }
        next.insert(player.player_id(), std::move(cards));
    }
    if (next == publicHands_)
        return;
    publicHands_ = std::move(next);
    emit changed();
}

void RuledRevealState::applyActiveSnapshot(const ruled::v1::ActivePublicRevealSnapshot &snapshot)
{
    QSignalBlocker block(this);
    QSet<QString> next;
    bool updated = false;
    for (const auto &reveal : snapshot.reveals()) {
        const QString id = QString::fromStdString(reveal.reveal_id());
        if (id.isEmpty() || retired_.contains(id) || next.contains(id))
            continue;
        bool added = false;
        auto it = entries_.find(id);
        if (it == entries_.end()) {
            added = publish(reveal);
            it = entries_.find(id);
        } else if (!::matchesReceipt(it.value(), reveal)) {
            continue;
        }
        if (it == entries_.end())
            continue;
        next.insert(id);
        if (id == choiceId_ && it->phase == Phase::Choice) {
            // This snapshot confirms that the same exact offer remains live. Its chooser's
            // physical popup IDs are carried by the validated ResolutionChoiceRequired event.
        } else if (it->phase != Phase::Active || !it->choiceCardIds.isEmpty()) {
            it->phase = Phase::Active;
            it->choiceCardIds.clear();
            updated = true;
        }
        updated = added || updated;
    }
    for (const QString &id : active_ - next) {
        auto it = entries_.find(id);
        if (it != entries_.end()) {
            it->phase = Phase::Completed;
            it->choiceCardIds.clear();
        }
        retired_.insert(id);
        if (choiceId_ == id)
            choiceId_.clear();
        updated = true;
    }
    if (!choiceId_.isEmpty() && !next.contains(choiceId_)) {
        auto it = entries_.find(choiceId_);
        if (it != entries_.end()) {
            it->phase = Phase::Completed;
            it->choiceCardIds.clear();
        }
        retired_.insert(choiceId_);
        choiceId_.clear();
        updated = true;
    }
    active_ = std::move(next);
    block.unblock();
    if (updated)
        emit changed();
}

void RuledRevealState::dismiss(const QString &id)
{
    const auto it = entries_.find(id);
    if (it == entries_.end() || it->phase != Phase::Completed)
        return;
    entries_.erase(it);
    order_.removeAll(id);
    emit changed();
}

void RuledRevealState::clear()
{
    entries_.clear();
    publicHands_.clear();
    order_.clear();
    seen_.clear();
    active_.clear();
    retired_.clear();
    choiceId_.clear();
    emit changed();
}

void RuledRevealState::setWindowsSuppressed(bool suppressed)
{
    if (suppressed_ == suppressed)
        return;
    suppressed_ = suppressed;
    emit changed();
}

void RuledRevealState::discardCompleted()
{
    for (const auto &id : entries_.keys()) {
        if (entries_[id].phase == Phase::Completed)
            dismiss(id);
    }
}
