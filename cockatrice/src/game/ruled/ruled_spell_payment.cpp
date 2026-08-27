#include "ruled_spell_payment.h"

#include <limits>

void RuledSpellPayment::begin()
{
    clear();
    active = true;
}

void RuledSpellPayment::clear()
{
    ++transactionId;
    revision = 0;
    active = pending = submitting = false;
    selection.Clear();
    restrictedMana.Clear();
    view.Clear();
}

void RuledSpellPayment::invalidate()
{
    if (!active || submitting)
        return;
    ++revision;
    pending = true;
}

bool RuledSpellPayment::beginSubmission()
{
    if (!active || pending || submitting || !view.valid() || !view.complete() || view.selection_changed())
        return false;
    submitting = true;
    return true;
}

ruled::v1::PreviewSpellPayment RuledSpellPayment::request(ruled::v1::CastSpell cast)
{
    ruled::v1::PreviewSpellPayment query;
    query.set_transaction_id(transactionId);
    query.set_revision(++revision);
    *cast.mutable_payment() = selection;
    *cast.mutable_restricted_mana() = restrictedMana;
    *query.mutable_cast_spell() = std::move(cast);
    pending = true;
    return query;
}

bool RuledSpellPayment::apply(const ruled::v1::SpellPaymentPreview &preview)
{
    if (!active || submitting || preview.transaction_id() != transactionId || preview.revision() != revision ||
        !pending)
        return false;
    pending = false;
    // A queued mana update may immediately follow a sanitized preview. Keep its explanation
    // visible for this transaction, while completion/validity always come from the newest reply.
    const auto previousNotice = view.valid() ? view.error() : std::string{};
    view = preview;
    if (view.valid() && view.error().empty())
        view.set_error(previousNotice);
    if (preview.valid()) {
        selection = preview.selection();
        restrictedMana = preview.restricted_mana();
    }
    return true;
}

bool RuledSpellPayment::selected(quint32 oid) const
{
    for (const auto &c : selection.convoke())
        if (c.object().object_id() == oid)
            return true;
    return false;
}

const ruled::v1::ConvokeCandidate *RuledSpellPayment::candidate(quint32 oid) const
{
    if (!active || pending || submitting || !view.valid())
        return nullptr;
    for (const auto &c : view.candidates())
        if (c.object().object_id() == oid)
            return &c;
    return nullptr;
}

bool RuledSpellPayment::remove(quint32 oid)
{
    if (!active || pending || submitting)
        return false;
    for (int i = 0; i < selection.convoke_size(); ++i) {
        if (selection.convoke(i).object().object_id() != oid)
            continue;
        selection.mutable_convoke()->DeleteSubrange(i, 1);
        invalidate();
        return true;
    }
    return false;
}

bool RuledSpellPayment::select(quint32 oid, int kind)
{
    const auto *c = candidate(oid);
    if (!c || selected(oid))
        return false;
    bool allowed = false;
    for (int option : c->options())
        allowed |= option == kind;
    if (!allowed)
        return false;
    auto *chosen = selection.add_convoke();
    *chosen->mutable_object() = c->object();
    chosen->set_kind(static_cast<ruled::v1::ConvokePaymentKind>(kind));
    invalidate();
    return true;
}

bool RuledSpellPayment::payMana(QChar symbol, quint32 groupId)
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
    invalidate();
    return true;
}

QString RuledSpellPayment::contributionLabel(int kind)
{
    const QString symbols = QStringLiteral("WUBRG1");
    return kind >= 1 && kind <= symbols.size() ? QStringLiteral("{%1}").arg(symbols.at(kind - 1)) : QString{};
}
