#ifndef COCKATRICE_RULED_AUTOMATION_CONTROLS_H
#define COCKATRICE_RULED_AUTOMATION_CONTROLS_H

#include <QHash>
#include <QKeySequence>
#include <QWidget>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

class QLabel;
class QMenu;
class QPushButton;
class RuledControlBindings;

/// Presents acknowledged engine automation and submits only engine-offered choices.
class RuledAutomationControls : public QWidget
{
    Q_OBJECT
public:
    explicit RuledAutomationControls(QWidget *parent = nullptr);
    void setScope(QWidget *scope);
    void setShortcut(int key, const QList<QKeySequence> &sequences);
    void setControlState(const ruled::v1::AutomationState &state,
                         quint64 promptId,
                         bool live,
                         bool gameplayPending,
                         bool primaryEnabled);
    void setUpdating(bool updating, bool rejected = false);
    QMenu *stackMenu(quint32 stackObjectId);
    QMenu *phaseMenu(ruled::v1::PhaseId phase);
    QMenu *managerMenu();
signals:
    void primaryRequested();
    void commandRequested(const ruled::v1::RuledCommand &command);

private:
    void refresh();
    void activate(int key);
    void sendUpdate(const ruled::v1::UpdateAutomation &update);
    const ruled::v1::StackAutomationOffer *offer(quint32 oid) const;
    const ruled::v1::AutomationRuleInfo *rule(quint64 id) const;
    bool current(const ruled::v1::AutomationStackRef &reference) const;
    void
    addTriggerScope(QMenu *menu, const ruled::v1::StackAutomationOffer &offer, ruled::v1::AutomationTriggerScope scope);
    RuledControlBindings *bindings;
    QHash<int, QPushButton *> buttons;
    QLabel *status;
    QPushButton *fullControl;
    QPushButton *manage;
    ruled::v1::AutomationState state;
    quint64 promptId = 0;
    bool live = false;
    bool gameplayPending = false;
    bool primaryEnabled = false;
    bool updating = false;
    bool rejected = false;
};
#endif
