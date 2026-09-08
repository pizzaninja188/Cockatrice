#include "ruled_pending_cast.h"

#include <QCoreApplication>

// Headless cost reconciliation and presentation. Separate from the menu implementation so
// menu-only consumers need not link the authoritative client-state query implementation.

QMap<QChar, int> RuledPendingCast::parseSimpleManaCost(const QString &manaCost)
{
    QMap<QChar, int> parsed;
    auto addSymbol = [&parsed](QChar c) {
        const QChar sym = c.toUpper();
        if (QStringLiteral("WUBRGCX").contains(sym)) {
            parsed[sym] += 1;
        }
    };

    bool inBraces = false;
    QString token;
    for (QChar c : manaCost) {
        if (c == '{') {
            inBraces = true;
            token.clear();
            continue;
        }
        if (c == '}') {
            inBraces = false;
            // Numeric tokens are generic mana of any length ({1}, {4}, {10}); only fall back
            // to single-symbol parsing for non-numeric tokens ({G}, {C}, {X}). The previous
            // size()==1 check routed single digits to addSymbol, which silently dropped them.
            bool ok = false;
            const int generic = token.toInt(&ok);
            if (ok) {
                if (generic > 0) {
                    parsed['X'] += generic;
                }
            } else if (token.size() == 1) {
                addSymbol(token.at(0));
            }
            token.clear();
            continue;
        }
        if (inBraces) {
            token.append(c);
            continue;
        }
        if (c.isDigit()) {
            parsed['X'] += c.digitValue();
            continue;
        }
        addSymbol(c);
    }
    return parsed;
}

QString RuledPendingCast::formatSimpleManaCost(const QMap<QChar, int> &cost)
{
    // Render in canonical Scryfall brace form ({4}{G}{G}). The brackets double as a
    // placeholder for real mana symbols later, so they are kept rather than stripped.
    QString out;
    const int generic = cost.value('X', 0);
    if (generic > 0) {
        out += QStringLiteral("{%1}").arg(generic);
    }
    for (QChar c : QStringLiteral("WUBRGC")) {
        const int count = cost.value(c, 0);
        for (int i = 0; i < count; ++i) {
            out += QStringLiteral("{%1}").arg(c);
        }
    }
    return out;
}

QVector<RuledFlexPip> RuledPendingCast::parseFlexPips(const QString &manaCost)
{
    // Walk the Scryfall brace groups in order so each pip's index matches the engine's
    // ManaCost pip order. Flexible pips (CR 107.4d–f) contain a slash: {G/U} hybrid,
    // {2/W} mono-hybrid, {C/P} Phyrexian. Everything else just advances the index.
    QVector<RuledFlexPip> out;
    const QString validColors = QStringLiteral("WUBRG");
    quint32 index = 0;
    bool inBraces = false;
    QString token;
    for (QChar c : manaCost) {
        if (c == '{') {
            inBraces = true;
            token.clear();
            continue;
        }
        if (c == '}') {
            inBraces = false;
            const int slash = token.indexOf('/');
            if (slash > 0) {
                const QString left = token.left(slash).toUpper();
                const QString right = token.mid(slash + 1).toUpper();
                RuledFlexPip pip;
                pip.pipIndex = index;
                bool numeric = false;
                const int leftNum = left.toInt(&numeric);
                if (right == QLatin1String("P") && left.size() == 1 && validColors.contains(left)) {
                    pip.phyrexian = true;
                    pip.colorA = left.at(0);
                    out.append(pip);
                } else if (numeric && right.size() == 1 && validColors.contains(right)) {
                    pip.generic = leftNum;
                    pip.colorA = right.at(0);
                    out.append(pip);
                } else if (left.size() == 1 && right.size() == 1 && validColors.contains(left) &&
                           validColors.contains(right)) {
                    pip.colorA = left.at(0);
                    pip.colorB = right.at(0);
                    out.append(pip);
                }
                // Unrecognized slash forms (e.g. {G/U/P}, {S}) are left for the engine to reject.
            }
            ++index;
            token.clear();
            continue;
        }
        if (inBraces) {
            token.append(c);
        }
    }
    return out;
}

bool RuledPendingCast::flexPipMatchesColor(const RuledFlexPip &pip, QChar color)
{
    const QChar c = color.toUpper();
    if (pip.colorA == c) {
        return true;
    }
    // Only true two-color hybrid pips ({G/U}) accept their second color; mono-hybrid and
    // Phyrexian pips have a single color (their other alternative is generic mana / life).
    return !pip.phyrexian && pip.generic == 0 && !pip.colorB.isNull() && pip.colorB == c;
}

void RuledPendingCast::applyFlexChoicesToCost(QMap<QChar, int> &fixed,
                                              QVector<quint32> &lifePipIndices,
                                              QVector<RuledFlexPip> &flex,
                                              const QVector<bool> &choiceIsAlternative)
{
    for (int i = 0; i < flex.size(); ++i) {
        const RuledFlexPip &pip = flex[i];
        const bool alternative = (i < choiceIsAlternative.size()) && choiceIsAlternative[i];
        if (pip.phyrexian) {
            if (alternative) {
                lifePipIndices.append(pip.pipIndex); // CR 107.4f: pay 2 life
            } else {
                fixed[pip.colorA.toUpper()] += 1;
            }
        } else if (pip.generic > 0) {
            if (alternative) {
                fixed[QChar('X')] += pip.generic; // CR 107.4e: N generic
            } else {
                fixed[pip.colorA.toUpper()] += 1;
            }
        } else {
            fixed[(alternative ? pip.colorB : pip.colorA).toUpper()] += 1; // CR 107.4d
        }
    }
    flex.clear();
}

bool RuledPendingCast::applyManaPipToFlexibleCost(QMap<QChar, int> &fixed,
                                                  QVector<RuledFlexPip> &flex,
                                                  bool colorlessMana,
                                                  QChar coloredMana)
{
    if (!colorlessMana) {
        const QChar sym = coloredMana.toUpper();
        // 1. A fixed colored demand of this exact color (CR 202.1).
        if (fixed.value(sym, 0) > 0) {
            fixed[sym] -= 1;
            return true;
        }
        // 2. CR 107.4d–f: pay an as-yet-untouched flexible pip's colored alternative. Preferring
        //    untouched pips means a correct-color tap claims a fresh pip rather than topping up a
        //    half-paid mono-hybrid generic — e.g. {2/R}{2/R} with one generic already down, a red
        //    completes the *other* pip and leaves the partial one alone.
        for (int i = 0; i < flex.size(); ++i) {
            if (flex[i].genericPaid == 0 && flexPipMatchesColor(flex[i], sym)) {
                flex.remove(i);
                return true;
            }
        }
    }
    // 3. Fixed generic {N}/{X}: payable by any mana.
    if (fixed.value('X', 0) > 0) {
        fixed['X'] -= 1;
        return true;
    }
    // 4. Fixed colorless {C}: only colorless mana qualifies (CR 107.4c).
    if (colorlessMana && fixed.value('C', 0) > 0) {
        fixed['C'] -= 1;
        return true;
    }
    // 5. CR 107.4e: a mono-hybrid generic alternative ({2/W}), payable by any mana. Top up a
    //    partially-paid pip first (so the mana already spent on it isn't stranded), otherwise
    //    open a fresh one.
    int partialIdx = -1;
    int freshIdx = -1;
    for (int i = 0; i < flex.size(); ++i) {
        if (flex[i].generic <= 0) {
            continue; // hybrid / Phyrexian have no generic alternative
        }
        if (flex[i].genericPaid > 0) {
            if (partialIdx < 0 || flex[i].genericPaid > flex[partialIdx].genericPaid) {
                partialIdx = i;
            }
        } else if (freshIdx < 0) {
            freshIdx = i;
        }
    }
    const int idx = (partialIdx >= 0) ? partialIdx : freshIdx;
    if (idx >= 0) {
        flex[idx].genericPaid += 1;
        if (flex[idx].genericPaid >= flex[idx].generic) {
            flex.remove(idx);
        }
        return true;
    }
    return false;
}

QString RuledPendingCast::formatRemainingCost(const QMap<QChar, int> &fixed, const QVector<RuledFlexPip> &flex)
{
    QString out = formatSimpleManaCost(fixed);
    for (const RuledFlexPip &pip : flex) {
        if (pip.phyrexian) {
            out += QStringLiteral("{%1/P}").arg(pip.colorA);
        } else if (pip.generic > 0) {
            out += QStringLiteral("{%1/%2}").arg(pip.generic - pip.genericPaid).arg(pip.colorA);
        } else {
            out += QStringLiteral("{%1/%2}").arg(pip.colorA).arg(pip.colorB);
        }
    }
    return out;
}

int RuledPendingCast::totalRemainingForCost(const QMap<QChar, int> &fixed, const QVector<RuledFlexPip> &flex)
{
    int total = 0;
    for (auto it = fixed.constBegin(); it != fixed.constEnd(); ++it) {
        total += it.value();
    }
    // Every unresolved flexible pip still owes at least one more mana (or, for Phyrexian, 2 life).
    total += flex.size();
    return total;
}

namespace
{
bool selectionStillLegal(const RuledClientState &state,
                         int localPlayerId,
                         const RuledPendingCostSelection &selection,
                         const QVector<RuledCostChoice> &choices)
{
    const auto choice = std::find_if(choices.cbegin(), choices.cend(), [&selection](const RuledCostChoice &entry) {
        return entry.costIndex == selection.costIndex && entry.zone == selection.zone;
    });
    if (choice == choices.cend()) {
        return false;
    }
    if (choice->kind == RuledCostChoiceKind::RemoveCounters) {
        return ruledCounterSelectionStillLegal(selection, *choice);
    }
    if (selection.selectedIds.isEmpty() || selection.selectedIds.size() > choice->max) {
        return false;
    }
    return std::all_of(selection.selectedIds.cbegin(), selection.selectedIds.cend(), [&](quint32 selectedId) {
        if (selection.zone == RuledCostChoiceZone::Hand) {
            const int slot = state.engineHandSlotForServerCard(localPlayerId, static_cast<int>(selectedId));
            return slot >= 0 && choice->candidateIds.contains(static_cast<quint32>(slot));
        }
        if (!choice->candidateIds.contains(selectedId)) {
            return false;
        }
        if (ruledCostUsesObjectRefs(*choice)) {
            const int selectedIndex = selection.selectedIds.indexOf(selectedId);
            return selectedIndex >= 0 &&
                   selection.selectedGenerations.value(selectedIndex) == choice->candidateGenerations.value(selectedId);
        }
        return true;
    });
}
} // namespace

bool RuledPendingCast::reconcileSpellCosts(const RuledClientState &state, int localPlayerId)
{
    if (spell.valid) {
        const bool sourceStillLegal =
            spell.source == RuledCastSource::Hand
                ? state.isHandCastActionLegal(spell.handIndex, spell.faceIndex, spell.castMethod)
                : state.isZoneCastActionLegal(static_cast<quint32>(spell.handIndex), spell.faceIndex, spell.source,
                                              spell.castMethod, spell.castingPermissionId,
                                              spell.sourceZoneChangeGeneration);
        const auto latest = state.spellCostData(spell.handIndex, spell.faceIndex, spell.source, spell.castMethod,
                                                spell.castingPermissionId);
        const bool castCostSelectionsStillLegal =
            std::all_of(spell.castCostSelections.cbegin(), spell.castCostSelections.cend(), [&](const auto &selection) {
                const auto group =
                    std::find_if(latest.castCostGroups.cbegin(), latest.castCostGroups.cend(),
                                 [&selection](const auto &entry) { return entry.groupIndex == selection.groupIndex; });
                if (group == latest.castCostGroups.cend()) {
                    return false;
                }
                const auto option =
                    std::find_if(group->options.cbegin(), group->options.cend(), [&selection](const auto &entry) {
                        return entry.optionIndex == selection.optionIndex;
                    });
                if (option == group->options.cend() || !option->selectable) {
                    return false;
                }
                if (selection.objectKind == RuledPendingCastCostSelection::ObjectKind::Hand) {
                    const int slot = state.engineHandSlotForServerCard(localPlayerId, selection.selectedId);
                    return slot >= 0 && option->validHandIndices.contains(static_cast<quint32>(slot));
                }
                if (selection.objectKind == RuledPendingCastCostSelection::ObjectKind::Permanent) {
                    if (ruledCastCostUsesPermanentCohort(option->kind)) {
                        qint64 total = 0;
                        for (const quint32 oid : selection.selectedObjectIds) {
                            if (!option->validPermanentIds.contains(oid) ||
                                option->validPermanentGenerations.value(oid) !=
                                    selection.selectedObjectGenerations.value(oid) ||
                                option->candidateContributions.value(oid) !=
                                    selection.selectedObjectContributions.value(oid))
                                return false;
                            total += selection.selectedObjectContributions.value(oid);
                        }
                        const bool activeIncompleteSelection =
                            spell.waitingForCastCostObject && spell.nextCastCostGroup < latest.castCostGroups.size() &&
                            latest.castCostGroups.at(spell.nextCastCostGroup).groupIndex == selection.groupIndex &&
                            spell.activeCastCostOption == selection.optionIndex;
                        if (activeIncompleteSelection)
                            return selection.selectedObjectIds.size() <= option->objectMax;
                        return selection.selectedObjectIds.size() >= option->objectMin &&
                               selection.selectedObjectIds.size() <= option->objectMax &&
                               (option->aggregateMinimum <= 0 || total >= option->aggregateMinimum);
                    }
                    return option->validPermanentIds.contains(selection.selectedId) &&
                           option->validPermanentGenerations.value(selection.selectedId) ==
                               selection.expectedZoneChangeGeneration &&
                           option->validPermanentGenericReductions.value(selection.selectedId) ==
                               selection.genericCostReduction;
                }
                return option->kind == RuledCastCostOptionKind::Mana ||
                       option->kind == RuledCastCostOptionKind::PayLife;
            });
        const bool pendingCastCostObjectStillLegal =
            !spell.waitingForCastCostObject || (spell.nextCastCostGroup < latest.castCostGroups.size() && [&]() {
                const auto &group = latest.castCostGroups.at(spell.nextCastCostGroup);
                const auto option =
                    std::find_if(group.options.cbegin(), group.options.cend(),
                                 [this](const auto &entry) { return entry.optionIndex == spell.activeCastCostOption; });
                return option != group.options.cend() && option->selectable &&
                       ruledCastCostUsesObjectChoice(option->kind);
            }());
        if (!sourceStillLegal || !castCostSelectionsStillLegal || !pendingCastCostObjectStillLegal ||
            std::any_of(spell.costSelections.cbegin(), spell.costSelections.cend(),
                        [&state, localPlayerId, &latest](const auto &selection) {
                            return !selectionStillLegal(state, localPlayerId, selection, latest.choices);
                        })) {
            return false;
        } else {
            spell.costChoices = latest.choices;
            spell.castCostGroups = latest.castCostGroups;
        }
    }
    return true;
}

bool RuledPendingCast::reconcileAbilityCosts(const RuledClientState &state, int localPlayerId)
{
    if (ability.valid) {
        const bool sourceStillCurrent = ruledPendingAbilitySourceStillCurrent(state, ability);
        const auto latest = ability.permanentAction
                                ? QVector<RuledCostChoice>{}
                                : state.abilityCostChoices(ability.permanentOid, ability.abilityIndex);
        if (!sourceStillCurrent || std::any_of(ability.costSelections.cbegin(), ability.costSelections.cend(),
                                               [&state, localPlayerId, &latest](const auto &selection) {
                                                   return !selectionStillLegal(state, localPlayerId, selection, latest);
                                               })) {
            return false;
        } else {
            ability.costChoices = latest;
        }
    }
    return true;
}

bool RuledPendingCast::isAwaitingRuledCastCostOption() const
{
    return spell.valid && !spell.waitingForCastCostObject && spell.nextCastCostGroup < spell.castCostGroups.size();
}

bool RuledPendingCast::pendingRuledCastCostGroupIsOptional() const
{
    if (!spell.valid || spell.nextCastCostGroup >= spell.castCostGroups.size())
        return false;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    return group.min == 0 &&
           std::none_of(spell.selectedModeLinkedCastCosts.cbegin(), spell.selectedModeLinkedCastCosts.cend(),
                        [&group](const auto &coordinate) { return coordinate.first == group.groupIndex; });
}

QString RuledPendingCast::pendingRuledCastCostSkipLabel() const
{
    return pendingRuledCastCostGroupIsOptional() ? spell.castCostGroups.at(spell.nextCastCostGroup).skipLabel
                                                 : QString{};
}

QVector<RuledCastCostOption> RuledPendingCast::pendingRuledCastCostOptions() const
{
    if (!isAwaitingRuledCastCostOption()) {
        return {};
    }
    auto options = spell.castCostGroups.at(spell.nextCastCostGroup).options;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    const bool full = ruledCastCostGroupSelectionCount(spell, group.groupIndex) >= group.max;
    for (auto &option : options) {
        const QPair<int, int> coordinate{group.groupIndex, option.optionIndex};
        const bool selected = ruledCastCostOptionAlreadySelected(spell, group.groupIndex, option.optionIndex);
        option.selectable = option.selectable && (!full || selected) && !spell.modeLinkedCastCosts.contains(coordinate);
        if (selected)
            option.label = QCoreApplication::translate("PlayerActions", "Selected: %1").arg(option.label);
    }
    return options;
}

int RuledPendingCast::pendingRuledCastCostSelectedCount() const
{
    if (isAwaitingRuledCastCostObject()) {
        const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
        const auto selection =
            std::find_if(spell.castCostSelections.cbegin(), spell.castCostSelections.cend(), [&](const auto &entry) {
                return entry.groupIndex == group.groupIndex && entry.optionIndex == spell.activeCastCostOption;
            });
        return selection == spell.castCostSelections.cend() ? 0 : selection->selectedObjectIds.size();
    }
    if (!isAwaitingRuledCastCostOption())
        return 0;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    return ruledCastCostGroupSelectionCount(spell, group.groupIndex);
}

int RuledPendingCast::pendingRuledCastCostMinimum() const
{
    if (isAwaitingRuledCastCostObject()) {
        const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
        const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [&](const auto &entry) {
            return entry.optionIndex == spell.activeCastCostOption;
        });
        return option == group.options.cend() ? 0 : option->objectMin;
    }
    return isAwaitingRuledCastCostOption() ? spell.castCostGroups.at(spell.nextCastCostGroup).min : 0;
}

int RuledPendingCast::pendingRuledCastCostMaximum() const
{
    if (isAwaitingRuledCastCostObject()) {
        const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
        const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [&](const auto &entry) {
            return entry.optionIndex == spell.activeCastCostOption;
        });
        return option == group.options.cend() ? 0 : option->objectMax;
    }
    return isAwaitingRuledCastCostOption() ? spell.castCostGroups.at(spell.nextCastCostGroup).max : 0;
}

bool RuledPendingCast::pendingRuledCastCostObjectCanConfirm() const
{
    if (!isAwaitingRuledCastCostObject())
        return false;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [&](const auto &entry) {
        return entry.optionIndex == spell.activeCastCostOption;
    });
    const auto selection =
        std::find_if(spell.castCostSelections.cbegin(), spell.castCostSelections.cend(), [&](const auto &entry) {
            return entry.groupIndex == group.groupIndex && entry.optionIndex == spell.activeCastCostOption;
        });
    if (option == group.options.cend() || selection == spell.castCostSelections.cend())
        return false;
    const qint64 total = std::accumulate(
        selection->selectedObjectIds.cbegin(), selection->selectedObjectIds.cend(), qint64{0},
        [&](qint64 value, quint32 oid) { return value + selection->selectedObjectContributions.value(oid); });
    return selection->selectedObjectIds.size() >= option->objectMin &&
           selection->selectedObjectIds.size() <= option->objectMax &&
           (option->aggregateMinimum <= 0 || total >= option->aggregateMinimum);
}

bool RuledPendingCast::pendingRuledCastCostObjectUsesExplicitConfirmation() const
{
    if (!isAwaitingRuledCastCostObject())
        return false;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [&](const auto &entry) {
        return entry.optionIndex == spell.activeCastCostOption;
    });
    return option != group.options.cend() && ruledCastCostUsesPermanentCohort(option->kind);
}

bool RuledPendingCast::isAwaitingRuledSpellCostSelection() const
{
    return spell.valid && spell.waitingForCost && spell.nextCostChoice < spell.costChoices.size();
}

bool RuledPendingCast::isAwaitingRuledCastCostObject() const
{
    return spell.valid && spell.waitingForCastCostObject && spell.nextCastCostGroup < spell.castCostGroups.size();
}

bool RuledPendingCast::isAwaitingRuledAbilityCostSelection() const
{
    return ability.valid && ability.waitingForCost && ability.nextCostChoice < ability.costChoices.size();
}

QString RuledPendingCast::pendingRuledAbilityCostPromptText() const
{
    if (!isAwaitingRuledAbilityCostSelection()) {
        return {};
    }
    const auto &choice = ability.costChoices.at(ability.nextCostChoice);
    const QString prompt = ruledCostSelectionPrompt(choice, ability.cardName);
    const auto progress = ruledPendingGraveyardCostSelectionProgress(ability);
    return choice.aggregateMinimum > 0 && progress ? QCoreApplication::translate("PlayerActions", "%1\nTotal: %2 / %3")
                                                         .arg(prompt)
                                                         .arg(progress->selected)
                                                         .arg(progress->required)
                                                   : prompt;
}

bool RuledPendingCast::isAwaitingRuledGraveyardCostSelection() const
{
    const bool abilityMulti = isAwaitingRuledAbilityCostSelection() &&
                              (ability.costChoices.at(ability.nextCostChoice).zone == RuledCostChoiceZone::Graveyard ||
                               ruledCostUsesObjectRefs(ability.costChoices.at(ability.nextCostChoice)));
    const bool spellTap =
        isAwaitingRuledSpellCostSelection() && ruledCostUsesObjectRefs(spell.costChoices.at(spell.nextCostChoice));
    return abilityMulti || spellTap;
}

bool RuledPendingCast::isRuledGraveyardCostObjectSelected(quint32 objectId) const
{
    return ruledPendingGraveyardCostSelectionContains(ability, objectId) ||
           ruledPendingGraveyardCostSelectionContains(spell, objectId);
}

bool RuledPendingCast::getRuledGraveyardCostSelectionProgress(int &required, int &selected) const
{
    auto progress = ruledPendingGraveyardCostSelectionProgress(ability);
    if (!progress.has_value()) {
        progress = ruledPendingGraveyardCostSelectionProgress(spell);
    }
    if (!progress.has_value()) {
        // Fail closed if an inconsistent local snapshot reaches the prompt renderer.
        required = 1;
        selected = 0;
        return false;
    }
    required = static_cast<int>(progress->required);
    selected = static_cast<int>(progress->selected);
    return true;
}

QString RuledPendingCast::pendingRuledSpellPromptText() const
{
    if (!spell.valid || spell.inDamageAllocationMode) {
        return {};
    }
    if (spell.waitingForCastCostObject && spell.nextCastCostGroup < spell.castCostGroups.size()) {
        const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
        const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [this](const auto &entry) {
            return entry.optionIndex == spell.activeCastCostOption;
        });
        if (option == group.options.cend()) {
            return group.prompt;
        }
        const QString instruction = ruledCastCostSelectionPrompt(*option);
        QString progress;
        if (ruledCastCostUsesPermanentCohort(option->kind)) {
            const auto selection = std::find_if(
                spell.castCostSelections.cbegin(), spell.castCostSelections.cend(), [&](const auto &entry) {
                    return entry.groupIndex == group.groupIndex && entry.optionIndex == option->optionIndex;
                });
            if (selection != spell.castCostSelections.cend()) {
                const qint64 total =
                    std::accumulate(selection->selectedObjectIds.cbegin(), selection->selectedObjectIds.cend(),
                                    qint64{0}, [&](qint64 value, quint32 oid) {
                                        return value + selection->selectedObjectContributions.value(oid);
                                    });
                progress = option->aggregateMinimum > 0
                               ? QCoreApplication::translate("PlayerActions", "\nSelected: %1, total power: %2 / %3")
                                     .arg(selection->selectedObjectIds.size())
                                     .arg(total)
                                     .arg(option->aggregateMinimum)
                               : QCoreApplication::translate("PlayerActions", "\nSelected: %1 / %2")
                                     .arg(selection->selectedObjectIds.size())
                                     .arg(option->objectMax);
            }
        }
        const QString text = instruction + progress;
        return spell.castCostObjectError.isEmpty()
                   ? text
                   : QCoreApplication::translate("PlayerActions", "%1\n%2").arg(spell.castCostObjectError, text);
    }
    if (spell.nextCastCostGroup < spell.castCostGroups.size()) {
        const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
        return QCoreApplication::translate("PlayerActions", "%1\nSelected: %2 (%3-%4)")
            .arg(group.prompt)
            .arg(ruledCastCostGroupSelectionCount(spell, group.groupIndex))
            .arg(group.min)
            .arg(group.max);
    }
    if (spell.waitingForTarget) {
        return {};
    }
    if (isAwaitingRuledSpellCostSelection()) {
        const auto &choice = spell.costChoices.at(spell.nextCostChoice);
        const QString prompt = ruledCostSelectionPrompt(choice, spell.cardName);
        const auto progress = ruledPendingGraveyardCostSelectionProgress(spell);
        return choice.aggregateMinimum > 0 && progress
                   ? QCoreApplication::translate("PlayerActions", "%1\nTotal: %2 / %3")
                         .arg(prompt)
                         .arg(progress->selected)
                         .arg(progress->required)
                   : prompt;
    }
    if (totalRemainingForCost(spell.remainingCost, spell.flexPips) == 0) {
        return {};
    }
    return QCoreApplication::translate("PlayerActions", "Pay mana for %1: %2 remaining (click mana counters).")
        .arg(spell.cardName, formatRemainingCost(spell.remainingCost, spell.flexPips));
}

QString RuledPendingCast::pendingRuledAbilityPromptText() const
{
    if (!ability.valid || !ability.waitingForMana) {
        return {};
    }
    if (totalRemainingForCost(ability.remainingCost, ability.flexPips) == 0) {
        return {};
    }
    return QCoreApplication::translate("PlayerActions", "Pay mana for %1: %2 remaining (click mana counters).")
        .arg(ability.cardName, formatRemainingCost(ability.remainingCost, ability.flexPips));
}

bool RuledPendingCast::declineCastCostGroup()
{
    if (!pendingRuledCastCostGroupIsOptional())
        return false;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    for (auto it = spell.castCostSelections.begin(); it != spell.castCostSelections.end();) {
        if (it->groupIndex != group.groupIndex) {
            ++it;
            continue;
        }
        const auto option = std::find_if(group.options.cbegin(), group.options.cend(),
                                         [&it](const auto &entry) { return entry.optionIndex == it->optionIndex; });
        if (option != group.options.cend() && option->kind == RuledCastCostOptionKind::Mana) {
            const auto extra = parseSimpleManaCost(option->additionalManaCost);
            for (auto mana = extra.constBegin(); mana != extra.constEnd(); ++mana)
                spell.remainingCost[mana.key()] -= mana.value();
        }
        spell.castCostGenericReduction -= it->genericCostReduction;
        it = spell.castCostSelections.erase(it);
    }
    spell.waitingForCastCostObject = false;
    spell.activeCastCostOption = -1;
    spell.castCostObjectError.clear();
    ++spell.nextCastCostGroup;
    return true;
}
