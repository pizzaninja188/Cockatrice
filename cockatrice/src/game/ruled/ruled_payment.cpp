#include "ruled_payment.h"

#include <QHash>
#include <algorithm>
#include <atomic>
#include <limits>
#include <utility>

void RuledPayment::begin(bool guardSanitized)
{
    clear();
    active = true;
    guardSanitizedPayment = guardSanitized;
    // Opening the action is intentional, including when the engine reports a zero cost.
    // No pool mana is staged by begin(); a nonzero cost still needs explicit contributions.
    submissionArmed = true;
}

void RuledPayment::clear()
{
    retiredOptimisticManaCounterIds.append(takeQueuedManaCounterIds());
    if (!submitting)
        for (const auto &entry : optimisticManaCounters)
            retiredOptimisticManaCounterIds.append(entry.counterId);
    // Suspended payments restore older instances. IDs must remain unique across those restores
    // so a delayed preview for one mana ability cannot complete a later activation.
    static std::atomic<quint64> nextTransactionId{0};
    transactionId = ++nextTransactionId;
    revision = 0;
    active = pending = submitting = false;
    guardSanitizedPayment = submissionArmed = false;
    selection.Clear();
    restrictedMana.Clear();
    view.Clear();
    optimisticManaCounters.clear();
}

RuledPayment RuledPayment::suspend()
{
    auto saved = std::move(*this);
    *this = RuledPayment{};
    return saved;
}

void RuledPayment::invalidate()
{
    if (!active || submitting)
        return;
    ++revision;
    pending = true;
}

bool RuledPayment::beginSubmission()
{
    if (!active || pending || submitting || !view.valid() || !view.complete() || view.selection_changed() ||
        (guardSanitizedPayment && !submissionArmed))
        return false;
    // These pips arrived after the previewed selection. They are not part of the command and
    // must remain floating, even when no further pool snapshot changes their counter.
    retiredOptimisticManaCounterIds.append(takeQueuedManaCounterIds());
    submitting = true;
    return true;
}

ruled::v1::PreviewPayment RuledPayment::request(ruled::v1::CastSpell cast)
{
    ruled::v1::RuledCommand command;
    *command.mutable_cast_spell() = std::move(cast);
    return requestAction(std::move(command));
}

void RuledPayment::writePayment(ruled::v1::RuledCommand &command) const
{
    if (command.has_cast_spell()) {
        *command.mutable_cast_spell()->mutable_payment() = selection;
        *command.mutable_cast_spell()->mutable_restricted_mana() = restrictedMana;
    } else if (command.has_activate_ability()) {
        *command.mutable_activate_ability()->mutable_payment() = selection;
        *command.mutable_activate_ability()->mutable_restricted_mana() = restrictedMana;
    } else if (command.has_submit_resolution_choice()) {
        *command.mutable_submit_resolution_choice()->mutable_payment() = selection;
        *command.mutable_submit_resolution_choice()->mutable_restricted_mana() = restrictedMana;
    } else if (command.has_execute_permanent_action()) {
        *command.mutable_execute_permanent_action()->mutable_payment() = selection;
        *command.mutable_execute_permanent_action()->mutable_restricted_mana() = restrictedMana;
    }
}

ruled::v1::PreviewPayment RuledPayment::requestAction(ruled::v1::RuledCommand command)
{
    ruled::v1::PreviewPayment query;
    query.set_transaction_id(transactionId);
    query.set_revision(++revision);
    writePayment(command);
    if (command.has_cast_spell())
        *query.mutable_cast_spell() = command.cast_spell();
    else if (command.has_activate_ability())
        *query.mutable_activate_ability() = command.activate_ability();
    else if (command.has_submit_resolution_choice())
        *query.mutable_resolution_choice() = command.submit_resolution_choice();
    else if (command.has_execute_permanent_action())
        *query.mutable_execute_permanent_action() = command.execute_permanent_action();
    pending = true;
    return query;
}

bool RuledPayment::apply(const ruled::v1::PaymentPreview &preview)
{
    if (!active || submitting || preview.transaction_id() != transactionId || preview.revision() != revision ||
        !pending)
        return false;
    pending = false;
    // A queued mana update may immediately follow a sanitized preview. Keep its explanation
    // visible for this transaction, while completion/validity always come from the newest reply.
    const auto previousNotice = view.valid() ? view.error() : std::string{};
    view = preview;
    if (guardSanitizedPayment && preview.selection_changed())
        submissionArmed = false;
    if (view.valid() && view.error().empty())
        view.set_error(previousNotice);
    if (preview.valid()) {
        selection = preview.selection();
        restrictedMana = preview.restricted_mana();
        reconcileOptimisticManaCounters();
    }
    return true;
}

bool RuledPayment::selected(quint32 oid) const
{
    for (const auto &object : selection.waterbend())
        if (object.object_id() == oid)
            return true;
    for (const auto &c : selection.convoke())
        if (c.object().object_id() == oid)
            return true;
    return false;
}

const ruled::v1::ObjectPaymentCandidate *RuledPayment::candidate(quint32 oid) const
{
    if (!active || pending || submitting || !view.valid())
        return nullptr;
    for (const auto &c : view.candidates())
        if (c.object().object_id() == oid)
            return &c;
    return nullptr;
}

bool RuledPayment::remove(quint32 oid)
{
    if (!active || pending || submitting)
        return false;
    for (int i = 0; i < selection.waterbend_size(); ++i) {
        if (selection.waterbend(i).object_id() == oid) {
            selection.mutable_waterbend()->DeleteSubrange(i, 1);
            submissionArmed = true;
            invalidate();
            return true;
        }
    }
    for (int i = 0; i < selection.convoke_size(); ++i) {
        if (selection.convoke(i).object().object_id() != oid)
            continue;
        selection.mutable_convoke()->DeleteSubrange(i, 1);
        invalidate();
        return true;
    }
    return false;
}

bool RuledPayment::select(quint32 oid, int kind)
{
    const auto *c = candidate(oid);
    if (!c || selected(oid))
        return false;
    bool allowed = false;
    for (int option : c->options())
        allowed |= option == kind;
    if (!allowed)
        return false;
    submissionArmed = true;
    if (kind == ruled::v1::OBJECT_PAYMENT_KIND_WATERBEND) {
        *selection.add_waterbend() = c->object();
        invalidate();
        return true;
    }
    auto *chosen = selection.add_convoke();
    *chosen->mutable_object() = c->object();
    chosen->set_kind(static_cast<ruled::v1::ObjectPaymentKind>(kind));
    invalidate();
    return true;
}

bool RuledPayment::payMana(QChar symbol, quint32 groupId, int optimisticCounterId)
{
    if (!active || submitting)
        return false;
    const int index = QStringLiteral("WUBRGC").indexOf(symbol.toUpper());
    if (index < 0)
        return false;
    google::protobuf::Message *message = selection.mutable_mana();
    if (groupId) {
        ruled::v1::ManaSpendSelection *group = nullptr;
        for (auto &r : restrictedMana)
            if (r.restriction_group_id() == groupId)
                group = &r;
        if (!group) {
            group = restrictedMana.Add();
            group->set_restriction_group_id(groupId);
        }
        message = group;
    }
    const auto *field = message->GetDescriptor()->FindFieldByName(QString(symbol.toLower()).toStdString());
    const auto *reflection = message->GetReflection();
    const auto amount = reflection->GetUInt32(*message, field);
    if (amount == std::numeric_limits<quint32>::max())
        return false;
    reflection->SetUInt32(message, field, amount + 1);
    if (optimisticCounterId >= 0)
        optimisticManaCounters.append({optimisticCounterId, symbol.toUpper(), groupId});
    submissionArmed = true;
    invalidate();
    return true;
}

bool RuledPayment::stageMana(QChar symbol, quint32 groupId, int optimisticCounterId)
{
    if (!active || submitting || !QStringLiteral("WUBRGC").contains(symbol.toUpper()))
        return false;
    if (pending || !queuedMana.isEmpty()) {
        queuedMana.append({symbol.toUpper(), groupId, optimisticCounterId});
        return true;
    }
    return payMana(symbol, groupId, optimisticCounterId);
}

RuledPayment *RuledPayment::producedManaRecipient(RuledPayment *suspended)
{
    if (active && !submitting)
        return this;
    // A mana ability resolves before its acknowledgement restores the outer payment. Its
    // output belongs to that waiting payment, never to the ability that just produced it.
    return suspended && suspended->active && !suspended->submitting ? suspended : nullptr;
}

QVector<int> RuledPayment::takeQueuedManaCounterIds()
{
    QVector<int> result;
    for (const auto &entry : queuedMana)
        if (entry.counterId >= 0)
            result.append(entry.counterId);
    queuedMana.clear();
    return result;
}

int RuledPayment::optimisticManaCounterSpendCount(int counterId) const
{
    if (submitting) {
        return 0;
    }
    return std::count_if(optimisticManaCounters.cbegin(), optimisticManaCounters.cend(),
                         [counterId](const auto &entry) { return entry.counterId == counterId; }) +
           std::count_if(queuedMana.cbegin(), queuedMana.cend(),
                         [counterId](const auto &entry) { return entry.counterId == counterId; });
}

int RuledPayment::restrictedManaSpendCount(quint32 groupId, QChar symbol) const
{
    symbol = symbol.toUpper();
    if (submitting || groupId == 0 || !QStringLiteral("WUBRGC").contains(symbol))
        return 0;
    int count = std::count_if(queuedMana.cbegin(), queuedMana.cend(), [groupId, symbol](const auto &entry) {
        return entry.groupId == groupId && entry.symbol == symbol;
    });
    for (const auto &group : restrictedMana) {
        if (group.restriction_group_id() != groupId)
            continue;
        const auto *field = group.GetDescriptor()->FindFieldByName(QString(symbol.toLower()).toStdString());
        count += static_cast<int>(group.GetReflection()->GetUInt32(group, field));
    }
    return count;
}

QVector<int> RuledPayment::takeRetiredOptimisticManaCounterIds()
{
    QVector<int> result;
    result.swap(retiredOptimisticManaCounterIds);
    return result;
}

QVector<int> RuledPayment::takeAllOptimisticManaCounterIds()
{
    QVector<int> result;
    result.reserve(optimisticManaCounters.size() + retiredOptimisticManaCounterIds.size());
    for (const auto &entry : optimisticManaCounters)
        result.append(entry.counterId);
    result.append(retiredOptimisticManaCounterIds);
    result.append(takeQueuedManaCounterIds());
    optimisticManaCounters.clear();
    retiredOptimisticManaCounterIds.clear();
    return result;
}

void RuledPayment::reconcileOptimisticManaCounters()
{
    QHash<QString, int> retainedCounts;
    const auto addMana = [&retainedCounts](const auto &mana, quint32 groupId) {
        for (const auto &[symbol, amount] :
             {std::pair{'W', mana.w()}, std::pair{'U', mana.u()}, std::pair{'B', mana.b()}, std::pair{'R', mana.r()},
              std::pair{'G', mana.g()}, std::pair{'C', mana.c()}}) {
            retainedCounts.insert(QStringLiteral("%1:%2").arg(groupId).arg(QLatin1Char(symbol)),
                                  static_cast<int>(amount));
        }
    };
    addMana(selection.mana(), 0);
    for (const auto &restricted : restrictedMana)
        addMana(restricted, restricted.restriction_group_id());

    QVector<OptimisticManaCounter> retained;
    retained.reserve(optimisticManaCounters.size());
    for (const auto &entry : optimisticManaCounters) {
        const QString key = QStringLiteral("%1:%2").arg(entry.groupId).arg(entry.symbol);
        int &remaining = retainedCounts[key];
        if (remaining > 0) {
            --remaining;
            retained.append(entry);
        } else {
            retiredOptimisticManaCounterIds.append(entry.counterId);
        }
    }
    optimisticManaCounters.swap(retained);
}

QString RuledPayment::contributionLabel(int kind)
{
    if (kind == ruled::v1::OBJECT_PAYMENT_KIND_WATERBEND)
        return QStringLiteral("{1}");
    const QString symbols = QStringLiteral("WUBRG1");
    return kind >= 1 && kind <= symbols.size() ? QStringLiteral("{%1}").arg(symbols.at(kind - 1)) : QString{};
}
