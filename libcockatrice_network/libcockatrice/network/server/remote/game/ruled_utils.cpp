#include "ruled_utils.h"

bool isRuledModeManaPoolCounterName(const QString &name)
{
    const QString n = name.trimmed().toLower();
    if (n.length() != 1) {
        return false;
    }
    return QStringLiteral("wubrgxc").contains(n.at(0), Qt::CaseInsensitive);
}

bool isPrivateChoiceKind(ruled::v1::ChoiceKind kind)
{
    switch (kind) {
        case ruled::v1::CHOICE_KIND_HAND_CARDS:     // the decider's own hand
        case ruled::v1::CHOICE_KIND_LIBRARY_SEARCH: // the decider's library
        case ruled::v1::CHOICE_KIND_OPPONENT_HAND:  // another player's hand, CR 701.7 "look"
            return true;
        case ruled::v1::CHOICE_KIND_REVEALED:
        case ruled::v1::CHOICE_KIND_TARGET_OBJECTS:
        case ruled::v1::CHOICE_KIND_LEGEND_KEEP:
            return false;
        default:
            // Unknown kind from a newer engine: assume it conceals something.
            return true;
    }
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
    if (phase == "combat_damage" || phase == "first_strike_damage") {
        // CR 510.4: the first-strike substep shares the "Combat Damage Step" slot in the
        // Cockatrice phases toolbar; the prompt widget distinguishes them via the
        // `first_strike_step_pending` flag on the per-player view.
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
