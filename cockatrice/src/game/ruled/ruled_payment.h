#ifndef RULED_PAYMENT_H
#define RULED_PAYMENT_H

#include <QString>
#include <QVector>
#include <QtGlobal>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

/// Local transaction state. All eligibility, remaining costs, and completion come from Rust.
/// Preview replies are correlated independently of the normal gameplay-command input lock.
class RuledPayment
{
public:
    void begin(bool guardSanitizedPayment = false);
    void clear();
    void invalidate();
    bool beginSubmission();
    quint64 transaction() const
    {
        return transactionId;
    }
    ruled::v1::PreviewPayment request(ruled::v1::CastSpell cast);
    ruled::v1::PreviewPayment requestAction(ruled::v1::RuledCommand command);
    void writePayment(ruled::v1::RuledCommand &command) const;
    bool apply(const ruled::v1::PaymentPreview &preview);
    bool select(quint32 oid, int kind);
    bool payMana(QChar symbol, quint32 groupId = 0, int optimisticCounterId = -1);
    [[nodiscard]] int optimisticManaCounterSpendCount(int counterId) const;
    QVector<int> takeRetiredOptimisticManaCounterIds();
    QVector<int> takeAllOptimisticManaCounterIds();
    bool remove(quint32 oid);
    bool selected(quint32 oid) const;
    const ruled::v1::ObjectPaymentCandidate *candidate(quint32 oid) const;
    static QString contributionLabel(int kind);

    bool active = false;
    bool pending = false;
    bool submitting = false;
    ruled::v1::PaymentSelection selection;
    google::protobuf::RepeatedPtrField<ruled::v1::ManaSpendSelection> restrictedMana;
    ruled::v1::PaymentPreview view;

private:
    struct OptimisticManaCounter
    {
        int counterId = -1;
        QChar symbol;
        quint32 groupId = 0;
    };

    void reconcileOptimisticManaCounters();
    quint64 transactionId = 0;
    quint64 revision = 0;
    bool guardSanitizedPayment = false;
    bool submissionArmed = false;
    QVector<OptimisticManaCounter> optimisticManaCounters;
    QVector<int> retiredOptimisticManaCounterIds;
};

#endif
