#ifndef COCKATRICE_RULED_CONTROL_BINDINGS_H
#define COCKATRICE_RULED_CONTROL_BINDINGS_H

#include <QHash>
#include <QKeySequence>
#include <QObject>
#include <QPointer>
#include <functional>

class QShortcut;
class QWidget;

/// Game-tab shortcuts that leave text entry and modal choices to their owning widgets.
class RuledControlBindings : public QObject
{
public:
    explicit RuledControlBindings(QWidget *owner);
    ~RuledControlBindings() override;
    void setScope(QWidget *scope);
    void bind(const QString &name, const QList<QKeySequence> &keys, std::function<void()> invoke);
    void setKeys(const QString &name, const QList<QKeySequence> &keys);

protected:
    bool eventFilter(QObject *object, QEvent *event) override;

private:
    void refresh();
    bool acceptsInput() const;
    QWidget *owner;
    QPointer<QWidget> scope;
    QHash<QString, QPointer<QShortcut>> shortcuts;
};

#endif
