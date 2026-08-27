#ifndef RULED_SPELL_PAYMENT_UI_H
#define RULED_SPELL_PAYMENT_UI_H

#include "ruled_pending_cast.h"

#include <QPair>
#include <QVector>
#include <optional>

class PlayerActions;
class CardItem;
class QPainter;

/// Fork-owned UI bridge. The headless RuledSpellPayment holds staging; Rust authors legality.
class RuledSpellPaymentUi
{
public:
    explicit RuledSpellPaymentUi(PlayerActions *actions);
    static std::optional<ruled::v1::RuledCommand> buildCommand(PlayerActions *actions);
    bool startOrRefresh();
    bool payMana(const QString &name, quint32 groupId = 0);
    bool click(CardItem *card, bool leftClick);
    QString prompt() const;
    void clear();
    void suspendForManaAbility(quint32 oid, int abilityIndex);
    void resumeAfterManaAbility();
    static void paint(CardItem *card, QPainter *painter);

private:
    void schedule();
    void query();
    void received();
    void changed();
    bool applicable() const;
    PlayerActions *actions;
    bool queued = false;
    bool choosingLifePayment = false;
    QVector<QPair<QChar, quint32>> queuedMana;
    std::optional<PendingRuledSpellCast> suspended;
};

#endif
