#include "ruled_automation_controls.h"

#include "ruled_control_bindings.h"

#include <QAction>
#include <QCoreApplication>
#include <QGridLayout>
#include <QLabel>
#include <QMenu>
#include <QPushButton>
#include <QSignalBlocker>
#include <QVBoxLayout>

namespace rv1 = ruled::v1;
namespace
{
QString label(int key)
{
    const char *text = "";
    switch (key) {
        case 1:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "OK / Pass");
            break;
        case 2:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Response");
            break;
        case 3:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Yes");
            break;
        case 4:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "No");
            break;
        case 5:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Clear all");
            break;
        case 6:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Pass turn");
            break;
        case 8:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Auto-pass");
            break;
        case 0:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "End step");
            break;
    }
    return QCoreApplication::translate("RuledAutomationControls", text);
}
QString yieldLabel(rv1::AutomationYieldMode mode)
{
    const char *text = "";
    switch (mode) {
        case rv1::AUTOMATION_YIELD_MODE_UNTIL_RESPONSE:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Yielding until a response");
            break;
        case rv1::AUTOMATION_YIELD_MODE_TURN:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Yielding through this turn");
            break;
        case rv1::AUTOMATION_YIELD_MODE_END_STEP:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Yielding until the end step");
            break;
        case rv1::AUTOMATION_YIELD_MODE_STACK_ITEM:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Yielding to one stack item");
            break;
        case rv1::AUTOMATION_YIELD_MODE_CURRENT_STACK:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Yielding to the current stack; new items stop it");
            break;
        case rv1::AUTOMATION_YIELD_MODE_ENTIRE_STACK:
            text =
                QT_TRANSLATE_NOOP("RuledAutomationControls", "Yielding until the stack is empty, including new items");
            break;
        case rv1::AUTOMATION_YIELD_MODE_PHASE:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Yielding to the selected phase");
            break;
        default:
            break;
    }
    return QCoreApplication::translate("RuledAutomationControls", text);
}
QString autoPassStopLabel(rv1::AutomationStopReason reason)
{
    const char *text = "";
    switch (reason) {
        case rv1::AUTOMATION_STOP_REASON_NONE:
            return {};
        case rv1::AUTOMATION_STOP_REASON_PHASE_STOP:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Stopped: phase stop");
            break;
        case rv1::AUTOMATION_STOP_REASON_LAND:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Land play available");
            break;
        case rv1::AUTOMATION_STOP_REASON_SPELL:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Spell available");
            break;
        case rv1::AUTOMATION_STOP_REASON_ABILITY:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Ability available");
            break;
        case rv1::AUTOMATION_STOP_REASON_SPECIAL_ACTION:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Special action available");
            break;
        case rv1::AUTOMATION_STOP_REASON_WAITING_FOR_PRIORITY:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Waiting for priority");
            break;
        case rv1::AUTOMATION_STOP_REASON_REQUIRED_CHOICE:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Waiting for a required choice or payment");
            break;
        case rv1::AUTOMATION_STOP_REASON_MISSING_POLICY:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Waiting for phase-stop settings");
            break;
        default:
            text = QT_TRANSLATE_NOOP("RuledAutomationControls", "Action availability uncertain — holding priority");
            break;
    }
    return QCoreApplication::translate("RuledAutomationControls", text);
}
} // namespace

RuledAutomationControls::RuledAutomationControls(QWidget *parent)
    : QWidget(parent), bindings(new RuledControlBindings(this))
{
    setObjectName("ruledAutomationControls");
    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(6, 2, 6, 4);
    layout->setSpacing(4);
    auto *grid = new QGridLayout;
    const QList<int> keys{1, 2, 3, 4, 5, 6, 8, 0};
    for (int i = 0; i < keys.size(); ++i) {
        const int key = keys[i];
        auto *button = new QPushButton(this);
        button->setObjectName(QStringLiteral("ruledControl%1").arg(key));
        button->setAutoDefault(false);
        button->setSizePolicy(QSizePolicy::Minimum, QSizePolicy::Fixed);
        buttons.insert(key, button);
        grid->addWidget(button, i / 4, i % 4);
        connect(button, &QPushButton::clicked, this, [this, key] { activate(key); });
        bindings->bind(QString::number(key), {}, [button] { button->click(); });
        setShortcut(key, {QKeySequence(QString::number(key)), QKeySequence(QStringLiteral("Num+%1").arg(key))});
    }
    buttons[2]->setToolTip(
        tr("Pass through this turn until a response, an unconfigured trigger, or a required choice."));
    buttons[5]->setToolTip(tr("Cancel yields, remove this game's trigger and answer rules, and disable automatic "
                              "passing. Keep phase stops."));
    buttons[6]->setToolTip(
        tr("Yield through the rest of this turn, including new stack items. Required choices still stop you."));
    buttons[8]->setToolTip(tr("Disable bluffing: pass when no usable action is available. Ordinary mana production "
                              "alone does not hold priority. Uncertainty, phase stops, and Full Control still stop you."));
    buttons[0]->setToolTip(
        tr("Yield to your first priority at this turn's end step; responses and required choices interrupt."));
    layout->addLayout(grid);
    auto *row = new QHBoxLayout;
    fullControl = new QPushButton(tr("Full Control / Hold Priority"), this);
    fullControl->setObjectName("ruledFullControl");
    fullControl->setCheckable(true);
    fullControl->setToolTip(tr("Temporarily stop automatic passing and answers without deleting saved rules."));
    connect(fullControl, &QPushButton::clicked, this, [this] {
        const bool enabled = !state.full_control();
        refresh(); // Checked state is acknowledged state, never the speculative click.
        rv1::UpdateAutomation update;
        update.set_full_control(enabled);
        sendUpdate(update);
    });
    manage = new QPushButton(tr("Manage…"), this);
    manage->setObjectName("ruledManageAutomation");
    connect(manage, &QPushButton::clicked, this, [this] {
        auto *menu = managerMenu();
        menu->setAttribute(Qt::WA_DeleteOnClose);
        menu->popup(manage->mapToGlobal(QPoint(0, manage->height())));
    });
    row->addWidget(fullControl, 1);
    row->addWidget(manage);
    layout->addLayout(row);
    status = new QLabel(this);
    status->setObjectName("ruledAutomationStatus");
    status->setWordWrap(true);
    status->setTextFormat(Qt::PlainText);
    layout->addWidget(status);
    refresh();
}

void RuledAutomationControls::setScope(QWidget *scope)
{
    bindings->setScope(scope);
}
void RuledAutomationControls::setShortcut(int key, const QList<QKeySequence> &sequences)
{
    if (!buttons.contains(key)) {
        return;
    }
    bindings->setKeys(QString::number(key), sequences);
    const auto keyText = sequences.isEmpty() ? QString() : sequences.first().toString(QKeySequence::NativeText);
    buttons[key]->setText(keyText.isEmpty() ? label(key) : keyText + QStringLiteral("\n") + label(key));
    buttons[key]->setAccessibleName(label(key));
}
void RuledAutomationControls::setControlState(const rv1::AutomationState &value,
                                              quint64 id,
                                              bool active,
                                              bool pending,
                                              bool primary)
{
    state = value;
    promptId = id;
    live = active;
    gameplayPending = pending;
    primaryEnabled = primary;
    refresh();
}
void RuledAutomationControls::setUpdating(bool value, bool failed)
{
    updating = value;
    rejected = failed;
    refresh();
}
void RuledAutomationControls::refresh()
{
    const bool ready = live && !gameplayPending && !updating;
    for (int key : {2, 6, 0}) {
        buttons[key]->setEnabled(ready && state.can_start_yield());
    }
    buttons[1]->setEnabled(ready && primaryEnabled);
    buttons[3]->setEnabled(ready && promptId && state.has_yes_branch_index());
    buttons[4]->setEnabled(ready && promptId && state.can_answer_no());
    buttons[5]->setEnabled(live);
    buttons[8]->setEnabled(ready);
    fullControl->setEnabled(live);
    {
        QSignalBlocker block(fullControl);
        fullControl->setChecked(state.full_control());
    }
    manage->setEnabled(live);
    QStringList parts;
    if (!live) {
        parts << tr("Controls are available to seated players in a live game.");
    } else {
        if (state.full_control()) {
            parts << tr("Full Control: automation is stopped");
        } else if (state.paused()) {
            parts << tr("Paused — pass once or resume from Manage");
        } else if (state.yield_mode() != rv1::AUTOMATION_YIELD_MODE_NONE) {
            parts << yieldLabel(state.yield_mode());
        }
        if (state.auto_pass()) {
            parts << tr("No-action auto-pass enabled");
            if (!state.full_control() && !state.paused() && state.yield_mode() == rv1::AUTOMATION_YIELD_MODE_NONE) {
                const auto reason = autoPassStopLabel(state.stop_reason());
                if (!reason.isEmpty()) {
                    parts << reason;
                }
            }
        }
        if (state.rules_size()) {
            parts << tr("%1 trigger rules").arg(state.rules_size());
        }
        if (state.has_stop_once()) {
            parts << tr("One-time phase stop set");
        }
        if (parts.isEmpty()) {
            parts << tr("Following phase stops");
        }
        if (updating) {
            parts << tr("Updating automation…");
        } else if (rejected) {
            parts << tr("Request was not accepted; current state is shown");
        }
    }
    status->setText(parts.join(QStringLiteral(" · ")));
}
void RuledAutomationControls::sendUpdate(const rv1::UpdateAutomation &update)
{
    if (!live) {
        return;
    }
    rv1::RuledCommand command;
    command.mutable_update_automation()->CopyFrom(update);
    emit commandRequested(command);
}
void RuledAutomationControls::activate(int key)
{
    if (!buttons.value(key) || !buttons[key]->isEnabled()) {
        return;
    }
    if (key == 1) {
        emit primaryRequested();
        return;
    }
    rv1::RuledCommand command;
    if (key == 3 || key == 4) {
        auto *answer = command.mutable_submit_resolution_choice();
        answer->set_resolution_prompt_id(promptId);
        answer->set_decision(key == 3 ? rv1::RESOLUTION_CHOICE_DECISION_SELECT_BRANCH
                                      : rv1::RESOLUTION_CHOICE_DECISION_DECLINE);
        if (key == 3) {
            answer->set_selected_branch_index(state.yes_branch_index());
        }
        emit commandRequested(command);
        return;
    }
    auto *update = command.mutable_update_automation();
    if (key == 5) {
        update->set_clear_all(true);
    } else if (key == 8) {
        update->set_auto_pass(!state.auto_pass());
    } else {
        update->mutable_yield_to()->set_mode(key == 2   ? rv1::AUTOMATION_YIELD_MODE_UNTIL_RESPONSE
                                             : key == 6 ? rv1::AUTOMATION_YIELD_MODE_TURN
                                                        : rv1::AUTOMATION_YIELD_MODE_END_STEP);
    }
    emit commandRequested(command);
}
const rv1::StackAutomationOffer *RuledAutomationControls::offer(quint32 oid) const
{
    for (const auto &entry : state.stack_offers()) {
        if (entry.has_stack() && entry.stack().object_id() == oid) {
            return &entry;
        }
    }
    return nullptr;
}
const rv1::AutomationRuleInfo *RuledAutomationControls::rule(quint64 id) const
{
    for (const auto &entry : state.rules()) {
        if (entry.rule_id() == id) {
            return &entry;
        }
    }
    return nullptr;
}
bool RuledAutomationControls::current(const rv1::AutomationStackRef &reference) const
{
    const auto *entry = offer(reference.object_id());
    return live && entry && entry->stack().SerializeAsString() == reference.SerializeAsString();
}
QMenu *RuledAutomationControls::stackMenu(quint32 oid)
{
    const auto *entry = offer(oid);
    if (!live || !entry) {
        return nullptr;
    }
    auto *menu = new QMenu(this);
    menu->setObjectName("ruledStackAutomationMenu");
    const auto addYield = [&](const QString &label, rv1::AutomationYieldMode mode, const char *name) {
        auto *action = menu->addAction(label);
        action->setObjectName(name);
        action->setEnabled(state.can_start_yield() && !gameplayPending && !updating);
        connect(action, &QAction::triggered, this, [this, reference = entry->stack(), mode] {
            if (!current(reference) || !state.can_start_yield() || gameplayPending || updating) {
                return;
            }
            rv1::UpdateAutomation update;
            update.mutable_yield_to()->set_mode(mode);
            if (mode == rv1::AUTOMATION_YIELD_MODE_STACK_ITEM) {
                update.mutable_yield_to()->mutable_stack()->CopyFrom(reference);
            }
            sendUpdate(update);
        });
    };
    addYield(tr("Yield to this item once"), rv1::AUTOMATION_YIELD_MODE_STACK_ITEM, "yieldStackItem");
    addYield(tr("Yield to current stack — stop on new items"), rv1::AUTOMATION_YIELD_MODE_CURRENT_STACK,
             "yieldCurrentStack");
    addYield(tr("Yield until stack is empty — including new items"), rv1::AUTOMATION_YIELD_MODE_ENTIRE_STACK,
             "yieldEntireStack");
    if (entry->can_yield_trigger()) {
        menu->addSeparator();
        auto *source = menu->addMenu(tr("This triggered ability → This source"));
        source->setEnabled(entry->can_source_rule());
        addTriggerScope(source, *entry, rv1::AUTOMATION_TRIGGER_SCOPE_SOURCE);
        addTriggerScope(menu->addMenu(tr("This triggered ability → All matching sources")), *entry,
                        rv1::AUTOMATION_TRIGGER_SCOPE_MATCHING_ABILITY);
    }
    menu->addSeparator();
    auto *manager = managerMenu();
    manager->setParent(menu, manager->windowFlags());
    manager->setTitle(tr("Manage automation"));
    menu->addMenu(manager);
    return menu;
}
void RuledAutomationControls::addTriggerScope(QMenu *menu,
                                              const rv1::StackAutomationOffer &entry,
                                              rv1::AutomationTriggerScope scope)
{
    const auto id = scope == rv1::AUTOMATION_TRIGGER_SCOPE_SOURCE ? entry.source_rule_id() : entry.matching_rule_id();
    const auto *existing = rule(id);
    const bool yielding = existing && existing->yield_priority();
    const auto answer = existing ? existing->answer() : rv1::AUTOMATION_ANSWER_ASK;
    const auto send = [this, reference = entry.stack(), scope,
                       revision = state.revision()](bool yielding, rv1::AutomationAnswer answer) {
        if (!current(reference) || state.revision() != revision) {
            return;
        }
        const auto *latest = offer(reference.object_id());
        if (scope == rv1::AUTOMATION_TRIGGER_SCOPE_SOURCE && !latest->can_source_rule()) {
            return;
        }
        const auto id =
            scope == rv1::AUTOMATION_TRIGGER_SCOPE_SOURCE ? latest->source_rule_id() : latest->matching_rule_id();
        rv1::UpdateAutomation update;
        if (!yielding && answer == rv1::AUTOMATION_ANSWER_ASK) {
            if (!id) {
                return;
            }
            update.set_remove_rule(id);
        } else {
            auto *rule = update.mutable_trigger_rule();
            rule->mutable_stack()->CopyFrom(reference);
            rule->set_scope(scope);
            rule->set_yield_priority(yielding);
            rule->set_answer(answer);
        }
        sendUpdate(update);
    };
    auto *yield = menu->addAction(tr("Always yield priority"));
    yield->setObjectName(scope == rv1::AUTOMATION_TRIGGER_SCOPE_SOURCE ? "yieldThisSource" : "yieldMatchingSources");
    yield->setCheckable(true);
    yield->setChecked(yielding);
    connect(yield, &QAction::triggered, this, [send, yielding, answer] { send(!yielding, answer); });
    if (entry.can_answer_optional()) {
        for (auto option : {rv1::AUTOMATION_ANSWER_ASK, rv1::AUTOMATION_ANSWER_YES, rv1::AUTOMATION_ANSWER_NO}) {
            auto *action =
                menu->addAction(option == rv1::AUTOMATION_ANSWER_ASK   ? tr("Ask each time")
                                : option == rv1::AUTOMATION_ANSWER_YES ? tr("Always Yes to the optional trigger")
                                                                       : tr("Always No to the optional trigger"));
            action->setObjectName(QStringLiteral("triggerAnswer%1").arg(option));
            action->setCheckable(true);
            action->setChecked(answer == option);
            connect(action, &QAction::triggered, this, [send, yielding, option] { send(yielding, option); });
        }
    }
    if (id) {
        auto *remove = menu->addAction(tr("Remove this rule"));
        connect(remove, &QAction::triggered, this, [this, id] {
            if (rule(id)) {
                rv1::UpdateAutomation u;
                u.set_remove_rule(id);
                sendUpdate(u);
            }
        });
    }
}
QMenu *RuledAutomationControls::managerMenu()
{
    auto *menu = new QMenu(this);
    menu->setObjectName("ruledAutomationManager");
    auto *resume = menu->addAction(tr("Resume saved automation"));
    resume->setEnabled(live && (state.paused() || state.full_control()));
    connect(resume, &QAction::triggered, this, [this] {
        rv1::UpdateAutomation u;
        u.set_full_control(false);
        sendUpdate(u);
    });
    auto *pause = menu->addAction(tr("Pause automation"));
    pause->setEnabled(live);
    connect(pause, &QAction::triggered, this, [this] {
        rv1::UpdateAutomation u;
        u.set_pause(true);
        sendUpdate(u);
    });
    auto *clear = menu->addAction(tr("Clear all automation"));
    clear->setEnabled(live);
    connect(clear, &QAction::triggered, buttons[5], &QPushButton::click);
    menu->addSeparator();
    if (state.rules().empty()) {
        menu->addAction(tr("No trigger rules"))->setEnabled(false);
    }
    for (const auto &entry : state.rules()) {
        QString description = QString::fromStdString(entry.description()).simplified();
        if (description.isEmpty()) {
            description = tr("Triggered ability");
        }
        QStringList flags;
        flags << (entry.scope() == rv1::AUTOMATION_TRIGGER_SCOPE_SOURCE ? tr("this source") : tr("matching sources"));
        flags << tr("controller %1").arg(entry.controller_player_id());
        if (entry.yield_priority()) {
            flags << tr("yield");
        }
        if (entry.answer() != rv1::AUTOMATION_ANSWER_ASK) {
            flags << (entry.answer() == rv1::AUTOMATION_ANSWER_YES ? tr("Yes") : tr("No"));
        }
        auto *action =
            menu->addAction(tr("Remove: %1 (%2)").arg(description.left(90), flags.join(QStringLiteral(", "))));
        action->setToolTip(description);
        connect(action, &QAction::triggered, this, [this, id = entry.rule_id()] {
            if (rule(id)) {
                rv1::UpdateAutomation u;
                u.set_remove_rule(id);
                sendUpdate(u);
            }
        });
    }
    return menu;
}
QMenu *RuledAutomationControls::phaseMenu(rv1::PhaseId phase)
{
    auto *menu = new QMenu(this);
    menu->setObjectName("ruledPhaseAutomationMenu");
    for (bool own : {true, false}) {
        auto *group = menu->addMenu(own ? tr("On your turn") : tr("On an opponent's turn"));
        auto *stop = group->addAction(tr("Stop here once"));
        stop->setObjectName(own ? "stopOnceOwnTurn" : "stopOnceOpponentTurn");
        stop->setEnabled(live);
        connect(stop, &QAction::triggered, this, [this, phase, own] {
            rv1::UpdateAutomation u;
            u.mutable_stop_once()->set_phase(phase);
            u.mutable_stop_once()->set_on_own_turn(own);
            sendUpdate(u);
        });
        auto *yield = group->addAction(tr("Yield until here"));
        yield->setObjectName(own ? "yieldPhaseOwnTurn" : "yieldPhaseOpponentTurn");
        yield->setEnabled(live && state.can_start_yield() && !gameplayPending && !updating);
        connect(yield, &QAction::triggered, this, [this, phase, own] {
            if (!state.can_start_yield() || gameplayPending || updating) {
                return;
            }
            rv1::UpdateAutomation u;
            u.mutable_yield_to()->set_mode(rv1::AUTOMATION_YIELD_MODE_PHASE);
            u.mutable_yield_to()->mutable_phase()->set_phase(phase);
            u.mutable_yield_to()->mutable_phase()->set_on_own_turn(own);
            sendUpdate(u);
        });
    }
    return menu;
}
