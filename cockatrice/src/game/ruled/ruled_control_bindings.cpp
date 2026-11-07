#include "ruled_control_bindings.h"

#include <QAbstractSpinBox>
#include <QApplication>
#include <QComboBox>
#include <QEvent>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QShortcut>
#include <QTextEdit>
#include <QWidget>

RuledControlBindings::RuledControlBindings(QWidget *widget) : QObject(widget), owner(widget), scope(widget)
{
    qApp->installEventFilter(this);
    connect(qApp, &QApplication::focusChanged, this, [this] { refresh(); });
}

void RuledControlBindings::setScope(QWidget *widget)
{
    scope = widget;
    for (auto shortcut : shortcuts)
        if (shortcut)
            shortcut->setParent(widget);
    refresh();
}

RuledControlBindings::~RuledControlBindings()
{
    for (auto shortcut : shortcuts)
        delete shortcut;
}

void RuledControlBindings::bind(const QString &name, const QList<QKeySequence> &keys, std::function<void()> invoke)
{
    auto *shortcut = new QShortcut(scope);
    shortcut->setKeys(keys);
    shortcut->setContext(Qt::WidgetWithChildrenShortcut);
    shortcut->setAutoRepeat(false);
    connect(shortcut, &QShortcut::activated, this, [this, invoke = std::move(invoke)] {
        if (acceptsInput())
            invoke();
    });
    delete shortcuts.take(name);
    shortcuts.insert(name, shortcut);
    refresh();
}

void RuledControlBindings::setKeys(const QString &name, const QList<QKeySequence> &keys)
{
    if (auto shortcut = shortcuts.value(name))
        shortcut->setKeys(keys);
}

bool RuledControlBindings::acceptsInput() const
{
    if (!scope || !scope->isVisible() || !owner->isVisible() || !owner->isEnabled() ||
        QApplication::activePopupWidget() || QApplication::activeModalWidget())
        return false;
    auto *focus = QApplication::focusWidget();
    if (!focus || (focus != scope && !scope->isAncestorOf(focus)))
        return false;
    for (auto *widget = focus; widget; widget = widget->parentWidget()) {
        if (qobject_cast<QLineEdit *>(widget) || qobject_cast<QTextEdit *>(widget) ||
            qobject_cast<QPlainTextEdit *>(widget) || qobject_cast<QAbstractSpinBox *>(widget) ||
            (qobject_cast<QComboBox *>(widget) && qobject_cast<QComboBox *>(widget)->isEditable()))
            return false;
        if (widget == scope)
            break;
    }
    return true;
}

void RuledControlBindings::refresh()
{
    const bool enabled = acceptsInput();
    for (auto shortcut : shortcuts)
        if (shortcut)
            shortcut->setEnabled(enabled);
}

bool RuledControlBindings::eventFilter(QObject *object, QEvent *event)
{
    // Qt sends ShortcutOverride before dispatching its shortcut map. Refresh here as well as on
    // focus changes: showing/hiding tabs and opening a menu can change the permitted input scope.
    if (event->type() == QEvent::ShortcutOverride)
        refresh();
    return QObject::eventFilter(object, event);
}
