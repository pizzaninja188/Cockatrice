#include "ruled_reveal_state.h"

#include <QSignalBlocker>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

QStringList RuledRevealState::Entry::cardNames() const
{
    QStringList names;
    for (const auto &card : cards)
        names.append(card.name);
    return names;
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

bool RuledRevealState::beginChoice(const ruled::v1::CardsRevealed &reveal, const QVector<int> &choiceCardIds)
{
    const QString id = QString::fromStdString(reveal.reveal_id());
    const auto existing = entries_.constFind(id);
    if (existing != entries_.cend()) {
        if (existing->owner != reveal.zone_owner_player_id() || existing->sourceZone != reveal.source_zone() ||
            existing->sourceObjectId != reveal.source_object_id() ||
            existing->sourceDescription != QString::fromStdString(reveal.source_description()) ||
            existing->cards.size() != reveal.cards_size())
            return false;
        for (int i = 0; i < reveal.cards_size(); ++i) {
            const auto &card = reveal.cards(i);
            if (existing->cards[i] != Card{card.object_id(), card.zone_change_generation(),
                                           QString::fromStdString(card.card_id()),
                                           QString::fromStdString(card.card_name())})
                return false;
        }
    }
    QSignalBlocker block(this);
    const bool replaced = id != choiceId_;
    if (id != choiceId_)
        completeChoice();
    const bool added = publish(reveal);
    auto it = entries_.find(id);
    if (it == entries_.end())
        return false;
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
        it->phase = Phase::Completed;
        it->choiceCardIds.clear();
    }
    choiceId_.clear();
    emit changed();
}

void RuledRevealState::applyActiveSnapshot(const ruled::v1::ActivePublicRevealSnapshot &snapshot)
{
    QSignalBlocker block(this);
    QSet<QString> next;
    bool updated = false;
    for (const auto &reveal : snapshot.reveals()) {
        updated = publish(reveal) || updated;
        const QString id = QString::fromStdString(reveal.reveal_id());
        auto it = entries_.find(id);
        if (it == entries_.end())
            continue;
        next.insert(id);
        if (it->phase != Phase::Active) {
            it->phase = Phase::Active;
            updated = true;
        }
    }
    for (const QString &id : active_ - next) {
        auto it = entries_.find(id);
        if (it != entries_.end())
            it->phase = Phase::Completed;
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
    order_.clear();
    seen_.clear();
    active_.clear();
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
