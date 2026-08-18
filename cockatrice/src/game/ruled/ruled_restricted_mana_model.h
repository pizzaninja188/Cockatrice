#ifndef COCKATRICE_RULED_RESTRICTED_MANA_MODEL_H
#define COCKATRICE_RULED_RESTRICTED_MANA_MODEL_H

#include "ruled_client_state.h"

#include <QHash>
#include <QMap>
#include <QVector>

using RuledRestrictedManaSelections = QHash<quint32, QMap<QChar, int>>;

struct RuledRestrictedManaProduction
{
    quint32 groupId = 0;
    QChar symbol;
    int amount = 0;
};

/// Tracks absolute engine-owned restricted contribution counts across pool snapshots.
class RuledRestrictedManaTracker
{
public:
    [[nodiscard]] QVector<RuledRestrictedManaProduction> observe(const QVector<RuledRestrictedManaGroup> &groups)
    {
        RuledRestrictedManaSelections current;
        QVector<RuledRestrictedManaProduction> produced;
        for (const auto &group : groups) {
            for (const QChar symbol : QStringLiteral("WUBRGC")) {
                const int count = group.countForSymbol(symbol);
                current[group.groupId][symbol] = count;
                const int previous = authoritativeCounts.value(group.groupId).value(symbol);
                if (count > previous) {
                    produced.append({group.groupId, symbol, count - previous});
                }
            }
        }
        authoritativeCounts = current;
        return produced;
    }

    void reset()
    {
        authoritativeCounts.clear();
    }

private:
    RuledRestrictedManaSelections authoritativeCounts;
};

/// Number of adjacent UI columns that still contain at least one unstaged contribution.
[[nodiscard]] inline int
ruledVisibleRestrictedManaColumnCount(const QVector<RuledRestrictedManaGroup> &groups,
                                      const RuledRestrictedManaSelections &optimisticallyStaged)
{
    int visible = 0;
    for (const auto &group : groups) {
        bool groupVisible = false;
        for (const QChar symbol : QStringLiteral("WUBRGC")) {
            if (group.countForSymbol(symbol) > optimisticallyStaged.value(group.groupId).value(symbol)) {
                groupVisible = true;
                break;
            }
        }
        visible += groupVisible ? 1 : 0;
    }
    return visible;
}

#endif // COCKATRICE_RULED_RESTRICTED_MANA_MODEL_H
