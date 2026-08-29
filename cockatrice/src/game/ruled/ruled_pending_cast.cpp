#include "ruled_pending_cast.h"

#include <QCheckBox>
#include <QCursor>
#include <QHBoxLayout>
#include <QLabel>
#include <QMenu>
#include <QPushButton>
#include <QVBoxLayout>
#include <QWidgetAction>

RuledPendingCast::RuledPendingCast() = default;

bool RuledPendingCast::chooseCounterCosts(QWidget *parent, PendingActivatedAbility &pending)
{
    while (pending.valid && pending.nextCostChoice < pending.costChoices.size()) {
        const auto choice = pending.costChoices.at(pending.nextCostChoice);
        if (choice.kind != RuledCostChoiceKind::RemoveCounters)
            break;
        if (choice.counterOptions.isEmpty() || choice.counterCount == 0 ||
            choice.counterSourceId != pending.permanentOid ||
            choice.counterSourceGeneration != pending.expectedZoneChangeGeneration)
            return false;
        quint32 optionId = choice.counterOptions.front().optionId;
        const int position = pending.nextCostChoice;
        const int abilityIndex = pending.abilityIndex;
        if (choice.counterOptions.size() > 1) {
            QMenu menu(parent);
            menu.addSection(QObject::tr("Remove %1 counter(s) from %2").arg(choice.counterCount).arg(pending.cardName));
            for (const auto &option : choice.counterOptions) {
                auto *action =
                    menu.addAction(QObject::tr("%1 (%2 available)").arg(option.label).arg(option.availableCount));
                action->setData(option.optionId);
            }
            QAction *chosen = menu.exec(QCursor::pos());
            if (!chosen)
                return false;
            optionId = chosen->data().toUInt();
        }
        // The nested menu loop may receive a new engine batch or cancel the transaction.
        RuledPendingCostSelection selection{
            choice.costIndex, choice.zone, {choice.counterSourceId}, {choice.counterSourceGeneration}, optionId};
        if (!pending.valid || pending.abilityIndex != abilityIndex || pending.nextCostChoice != position ||
            pending.permanentOid != choice.counterSourceId ||
            pending.expectedZoneChangeGeneration != choice.counterSourceGeneration ||
            position >= pending.costChoices.size() ||
            !ruledCounterSelectionStillLegal(selection, pending.costChoices.at(position)))
            return false;
        pending.costSelections.append(selection);
        ++pending.nextCostChoice;
    }
    return pending.valid;
}

PendingRuledSpellCast &RuledPendingCast::beginSpell()
{
    ability = {};
    spell = {};
    spell.valid = true;
    return spell;
}

PendingActivatedAbility &RuledPendingCast::beginAbility()
{
    spell = {};
    ability = {};
    ability.valid = true;
    return ability;
}

void RuledPendingCast::clearSpell()
{
    spell = {};
}

void RuledPendingCast::clearAbility()
{
    ability = {};
}

RuledPendingCast::InteractionKind RuledPendingCast::activeInteraction() const
{
    if (spell.valid) {
        return InteractionKind::Spell;
    }
    if (ability.valid) {
        return InteractionKind::Ability;
    }
    return InteractionKind::None;
}

static QString ruledCastOptionLabel(const RuledFaceOption &face)
{
    const QString label = face.castMethod == ruled::v1::CAST_METHOD_WARP ? QObject::tr("Warp %1").arg(face.faceName)
                                                                         : QObject::tr("Cast %1").arg(face.faceName);
    return face.manaCost.isEmpty() ? label : QStringLiteral("%1 (%2)").arg(label, face.manaCost);
}

QVector<RuledCardActionMenuOption>
RuledPendingCast::cardActionMenuOptions(const QVector<RuledFaceOption> &castFaces,
                                        const QList<int> &abilityIndices,
                                        const QStringList &abilityLabels,
                                        const QHash<int, bool> &abilityEnabled,
                                        const QStringList &manaProduced,
                                        bool manaAbilitiesOnly,
                                        const QVector<QPair<int, QString>> &paymentContributions)
{
    QVector<RuledCardActionMenuOption> options;
    options.reserve(paymentContributions.size() + castFaces.size() + abilityIndices.size());
    for (const auto &[kind, label] : paymentContributions)
        options.append({RuledCardActionMenuOption::Kind::PaymentContribution, kind, label, true});
    for (const auto &face : castFaces) {
        if (manaAbilitiesOnly)
            break;
        options.append({RuledCardActionMenuOption::Kind::CastFace, face.faceIndex, ruledCastOptionLabel(face), true, 0,
                        face.castMethod});
    }
    for (const int abilityIndex : abilityIndices) {
        if (manaAbilitiesOnly && manaProduced.value(abilityIndex).isEmpty())
            continue;
        const QStringList manaOptions = manaProduced.value(abilityIndex).split(QLatin1Char('/'));
        for (int optionIndex = 0; optionIndex < manaOptions.size(); ++optionIndex) {
            const QString label =
                manaOptions.size() > 1
                    ? QObject::tr("%1 — Add {%2}").arg(abilityLabels.value(abilityIndex), manaOptions.at(optionIndex))
                    : abilityLabels.value(abilityIndex);
            options.append({RuledCardActionMenuOption::Kind::ActivateAbility, abilityIndex, label,
                            abilityEnabled.value(abilityIndex, false), optionIndex});
        }
    }
    return options;
}

std::optional<RuledFaceOption> RuledPendingCast::chooseFace(QWidget *parent,
                                                            const QString &cardName,
                                                            const QVector<RuledFaceOption> &faces)
{
    if (faces.isEmpty()) {
        return std::nullopt;
    }

    QMenu menu(parent);
    menu.setTitle(cardName);
    QVector<QAction *> actions;
    actions.reserve(faces.size());
    for (const auto &face : faces) {
        actions.append(menu.addAction(ruledCastOptionLabel(face)));
    }
    QAction *chosen = menu.exec(QCursor::pos());
    const int position = actions.indexOf(chosen);
    if (position < 0) {
        return std::nullopt;
    }
    return faces.at(position);
}

std::optional<QVector<int>> RuledPendingCast::chooseModes(QWidget *parent,
                                                          const QString &cardName,
                                                          const QVector<RuledModalSpellOption> &modes,
                                                          int minModes,
                                                          int maxModes)
{
    QMenu menu(parent);
    menu.setTitle(cardName);

    if (minModes == 1 && maxModes == 1) {
        QVector<QAction *> actions;
        actions.reserve(modes.size());
        for (const auto &mode : modes) {
            auto *action = menu.addAction(
                QObject::tr("Cast %1 — %2").arg(cardName, mode.label));
            action->setEnabled(mode.selectable);
            actions.append(action);
        }
        QAction *chosen = menu.exec(QCursor::pos());
        const int position = actions.indexOf(chosen);
        if (position < 0) {
            return std::nullopt;
        }
        return QVector<int>{modes.at(position).modeIndex};
    }

    auto *panel = new QWidget(&menu);
    auto *layout = new QVBoxLayout(panel);
    layout->setContentsMargins(8, 6, 8, 6);
    auto *status = new QLabel(panel);
    layout->addWidget(status);

    QVector<QCheckBox *> checks;
    checks.reserve(modes.size());
    for (const auto &mode : modes) {
        auto *check = new QCheckBox(mode.label, panel);
        check->setEnabled(mode.selectable);
        layout->addWidget(check);
        checks.append(check);
    }

    auto *buttons = new QHBoxLayout;
    auto *confirm = new QPushButton(QObject::tr("Cast with selected modes"), panel);
    auto *cancel = new QPushButton(QObject::tr("Cancel"), panel);
    buttons->addWidget(confirm);
    buttons->addWidget(cancel);
    layout->addLayout(buttons);

    auto updateState = [=]() {
        int selected = 0;
        for (const auto *check : checks) {
            selected += check->isChecked() ? 1 : 0;
        }
        const QString requirement = minModes == maxModes
            ? QObject::tr("Choose exactly %1 — %2/%1 selected.").arg(minModes).arg(selected)
            : QObject::tr("Choose %1–%2 — %3 selected.").arg(minModes).arg(maxModes).arg(selected);
        status->setText(requirement);
        confirm->setEnabled(selected >= minModes && selected <= maxModes);
        for (auto *check : checks) {
            if (!check->isChecked()) {
                check->setEnabled(check->property("modeSelectable").toBool() && selected < maxModes);
            }
        }
    };
    for (int i = 0; i < checks.size(); ++i) {
        checks[i]->setProperty("modeSelectable", modes[i].selectable);
        QObject::connect(checks[i], &QCheckBox::toggled, panel, updateState);
    }

    bool confirmed = false;
    QObject::connect(confirm, &QPushButton::clicked, &menu, [&]() {
        confirmed = true;
        menu.close();
    });
    QObject::connect(cancel, &QPushButton::clicked, &menu, &QMenu::close);

    auto *widgetAction = new QWidgetAction(&menu);
    widgetAction->setDefaultWidget(panel);
    menu.addAction(widgetAction);
    updateState();
    menu.exec(QCursor::pos());
    if (!confirmed) {
        return std::nullopt;
    }

    QVector<int> selectedModes;
    for (int i = 0; i < checks.size(); ++i) {
        if (checks[i]->isChecked()) {
            selectedModes.append(modes[i].modeIndex);
        }
    }
    return selectedModes;
}
