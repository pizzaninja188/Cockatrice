#include "ruled_game_mode_ui.h"

#include <QAction>
#include <QCheckBox>
#include <QCoreApplication>
#include <QEvent>
#include <QObject>
#include <QPointer>
#include <QPushButton>

namespace
{
// The menu shares the prompt's availability and command path, including blocking choices.
class PriorityMenuBinding : public QObject
{
public:
    PriorityMenuBinding(QAction *action, QPushButton *button) : QObject(action), action(action), button(button)
    {
        connect(action, &QAction::changed, this, [this] { sync(); });
        if (button) {
            button->installEventFilter(this);
            connect(button, &QObject::destroyed, this, [this] {
                this->button = nullptr;
                sync();
            });
            connect(action, &QAction::triggered, button, [button] {
                if (!button->isHidden()) {
                    button->click();
                }
            });
        }
        sync();
    }

protected:
    bool eventFilter(QObject *watched, QEvent *event) override
    {
        switch (event->type()) {
            case QEvent::EnabledChange:
            case QEvent::Show:
            case QEvent::Hide:
            case QEvent::ShowToParent:
            case QEvent::HideToParent:
                sync();
                break;
            default:
                break;
        }
        return QObject::eventFilter(watched, event);
    }

private:
    QAction *action;
    QPointer<QPushButton> button;

    void sync()
    {
        const auto label = QCoreApplication::translate("TabGame", "Pass priority");
        if (action->text() != label) {
            action->setText(label);
        }
        const bool enabled = button && !button->isHidden() && button->isEnabled();
        if (action->isEnabled() != enabled) {
            action->setEnabled(enabled);
        }
    }
};
} // namespace

namespace RuledGameModeUi
{
bool allowsLocalFreeformGames()
{
    return false;
}

void configureCreationControls(QCheckBox *ruled, QCheckBox *shuffle)
{
    ruled->setChecked(true);
    ruled->setEnabled(false);
    ruled->hide();
    shuffle->setChecked(false);
    shuffle->setEnabled(false);
    shuffle->hide();
}

void disableLocalGameAction(QAction *action)
{
    const auto disable = [action] {
        if (action->isEnabled()) {
            action->setEnabled(false);
        }
        if (action->isVisible()) {
            action->setVisible(false);
        }
    };
    // Upstream reconnect and game-end callbacks may try to enable the action again.
    QObject::connect(action, &QAction::changed, action, disable);
    disable();
}

void bindPriorityMenuAction(QAction *action, QPushButton *button)
{
    new PriorityMenuBinding(action, button);
}
} // namespace RuledGameModeUi
