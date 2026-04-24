#include "ruled_utils.h"

bool isRuledModeManaPoolCounterName(const QString &name)
{
    const QString n = name.trimmed().toLower();
    if (n.length() != 1) {
        return false;
    }
    return QStringLiteral("wubrgxc").contains(n.at(0), Qt::CaseInsensitive);
}

int ruledPhaseLabelToCockatricePhase(const std::string &phase)
{
    if (phase == "untap") {
        return 0;
    }
    if (phase == "upkeep") {
        return 1;
    }
    if (phase == "draw") {
        return 2;
    }
    if (phase == "main1") {
        return 3;
    }
    if (phase == "begin_combat") {
        return 4;
    }
    if (phase == "declare_attackers") {
        return 5;
    }
    if (phase == "declare_blockers") {
        return 6;
    }
    if (phase == "combat_damage") {
        return 7;
    }
    if (phase == "end_combat") {
        return 8;
    }
    if (phase == "main2") {
        return 9;
    }
    if (phase == "end_step" || phase == "cleanup") {
        return 10;
    }
    return -1;
}
