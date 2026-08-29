#ifndef RULED_PAYMENT_UI_H
#define RULED_PAYMENT_UI_H

#include "ruled_pending_cast.h"

#include <QPair>
#include <QVector>
#include <optional>

class PlayerActions;
class CardItem;
class QPainter;

/// Fork-owned UI bridge. The headless RuledPayment holds staging; Rust authors legality.
class RuledPaymentUi
{
public:
    explicit RuledPaymentUi(PlayerActions *actions);
    static std::optional<ruled::v1::RuledCommand> buildCommand(PlayerActions *actions);
    static std::optional<ruled::v1::RuledCommand> buildActivationCommand(PlayerActions *actions);
    bool startOrRefresh();
    bool payMana(const QString &name, quint32 groupId = 0);
    bool click(CardItem *card, bool leftClick);
    [[nodiscard]] QVector<QPair<int, QString>> contributionOptions(CardItem *card) const;
    bool contribute(CardItem *card, int kind);
    [[nodiscard]] int optimisticManaCounterSpendCount(int counterId) const;
    bool applicable() const;
    QString prompt() const;
    void clear();
    void suspendForManaAbility(quint32 oid, int abilityIndex);
    void resumeAfterManaAbility();
    static void paint(CardItem *card, QPainter *painter);

private:
    struct QueuedMana
    {
        QChar symbol;
        quint32 groupId = 0;
        int counterId = -1;
    };

    enum class Context
    {
        None,
        Spell,
        Ability,
        Resolution
    };
    Context context() const;
    Context activeContext = Context::None;
    std::optional<ruled::v1::RuledCommand> buildPaymentCommand() const;
    void schedule();
    void query();
    void received();
    void changed();
    void restoreOptimisticManaCounters(const QVector<int> &counterIds);
    PlayerActions *actions;
    bool queued = false;
    bool choosingLifePayment = false;
    QVector<QueuedMana> queuedMana;
    std::optional<PendingRuledSpellCast> suspended;
    std::optional<PendingActivatedAbility> suspendedAbility;
};

#endif
