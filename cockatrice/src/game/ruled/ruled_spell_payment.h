#ifndef RULED_SPELL_PAYMENT_H
#define RULED_SPELL_PAYMENT_H

#include <QString>
#include <QtGlobal>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

/// Local transaction state. All eligibility, remaining costs, and completion come from Rust.
/// Preview replies are correlated independently of the normal gameplay-command input lock.
class RuledSpellPayment
{
public:
    void begin();
    void clear();
    void invalidate();
    bool beginSubmission();
    quint64 transaction() const
    {
        return transactionId;
    }
    ruled::v1::PreviewSpellPayment request(ruled::v1::CastSpell cast);
    bool apply(const ruled::v1::SpellPaymentPreview &preview);
    bool select(quint32 oid, int kind);
    bool payMana(QChar symbol, quint32 groupId = 0);
    bool remove(quint32 oid);
    bool selected(quint32 oid) const;
    const ruled::v1::ConvokeCandidate *candidate(quint32 oid) const;
    static QString contributionLabel(int kind);

    bool active = false;
    bool pending = false;
    bool submitting = false;
    ruled::v1::SpellPaymentSelection selection;
    google::protobuf::RepeatedPtrField<ruled::v1::ManaSpendSelection> restrictedMana;
    ruled::v1::SpellPaymentPreview view;

private:
    quint64 transactionId = 0;
    quint64 revision = 0;
};

#endif
