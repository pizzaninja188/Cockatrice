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
        actions.append(menu.addAction(QObject::tr("Cast %1").arg(face.faceName)));
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
