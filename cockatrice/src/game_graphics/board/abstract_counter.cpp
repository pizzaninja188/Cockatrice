#include "abstract_counter.h"

#include "../../game/ruled/ruled_actions.h"
#include "../../client/settings/cache_settings.h"
#include "../../interface/widgets/tabs/tab_game.h"
#include <libcockatrice/game/abstract_game.h>
#include <libcockatrice/game/player/player_logic.h>
#include "../../game/ruled/ruled_player_actions.h"
#include "translate_counter_name.h"
#include "../../client/settings/shortcuts_settings.h"
#include "../../game_graphics/board/translate_counter_name.h"
#include "../../interface/widgets/menus/tearoff_menu.h"

#include <QAction>
#include <QApplication>
#include <QEvent>
#include <QFlags>
#include <QGraphicsScene>
#include <QGraphicsSceneMouseEvent>
#include <QGraphicsView>
#include <QKeyEvent>
#include <QKeySequence>
#include <QList>
#include <QSharedPointer>
#include <QString>
#include <QVariant>
#include <QtGlobal>
#include <libcockatrice/game/board/counter_state.h>
#include <libcockatrice/game/player/player_actions.h>
#include <libcockatrice/game/player/player_info.h>
#include <libcockatrice/game/player/player_logic.h>
#include <libcockatrice/utility/expression.h>
#include <qnamespace.h>

namespace {
bool isManaPoolSymbolName(const QString &n)
{
    const QString t = n.trimmed().toLower();
    if (t.length() != 1) {
        return false;
    }
    return QStringLiteral("wubrgxc").contains(t.at(0), Qt::CaseInsensitive);
}

// Server cannot see whether Command_IncCounter came from a land tap (TableZone) or a counter click, so
// in ruled mode we only allow these pools to change through tap automation / tricerules as applicable.
bool manualManaEditsDisallowedInRuledGame(PlayerLogic *p, const QString &counterName)
{
    if (!p || !p->getGame()) {
        return false;
    }
    if (p->getPlayerInfo()->getJudge()) {
        return false;
    }
    if (!RuledActions::isRuledGame(p->getGame())) {
        return false;
    }
    return isManaPoolSymbolName(counterName);
}
} // namespace

class QWidget;

AbstractCounter::AbstractCounter(CounterState *state,
                                 PlayerLogic *_player,
                                 bool _shownInCounterArea,
                                 bool _useNameForShortcut,
                                 QGraphicsItem *parent)
    : QGraphicsItem(parent), player(_player), id(state->getId()), name(state->getName()), value(state->getValue()),
      color(state->getColor()), radius(state->getRadius()), useNameForShortcut(_useNameForShortcut),
      shownInCounterArea(_shownInCounterArea)
{
    setAcceptHoverEvents(true);

    connect(state, &CounterState::valueChanged, this, [this](int oldValue, int newValue, bool skipDamageAnimation) {
        value = newValue;
        onValueChanged(oldValue, newValue, skipDamageAnimation);
        update();
    });

    if (player->getPlayerInfo()->getLocalOrJudge()) {
        menu = new TearOffMenu(TranslateCounterName::getDisplayName(state->getName()));
        aSet = new QAction(this);
        connect(aSet, &QAction::triggered, this, &AbstractCounter::setCounter);
        menu->addAction(aSet);
        menu->addSeparator();
        for (int i = 10; i >= -10; --i) {
            if (i == 0) {
                menu->addSeparator();
                continue;
            }
            auto *a = new QAction(QString(i < 0 ? "%1" : "+%1").arg(i), this);
            if (i == -1) {
                aDec = a;
            }
            if (i == 1) {
                aInc = a;
            }
            a->setData(i);
            connect(a, &QAction::triggered, this, &AbstractCounter::incrementCounter);
            menu->addAction(a);
        }
    } else {
        menu = nullptr;
    }

    connect(&SettingsCache::instance().shortcuts(), &ShortcutsSettings::shortCutChanged, this,
            &AbstractCounter::refreshShortcuts);
    refreshShortcuts();
    retranslateUi();
}

AbstractCounter::~AbstractCounter()
{
    delete menu;
}

void AbstractCounter::delCounter()
{
    if (dialogSemaphore) {
        deleteAfterDialog = true;
    } else {
        deleteLater();
    }
}

void AbstractCounter::retranslateUi()
{
    if (aSet) {
        aSet->setText(tr("&Set counter..."));
    }
}

void AbstractCounter::setShortcutsActive()
{
    if (!menu || !player->getPlayerInfo()->getLocal()) {
        return;
    }
    ShortcutsSettings &sc = SettingsCache::instance().shortcuts();
    shortcutActive = true;
    if (name == "life") {
        aSet->setShortcuts(sc.getShortcut("Player/aSet"));
        aDec->setShortcuts(sc.getShortcut("Player/aDec"));
        aInc->setShortcuts(sc.getShortcut("Player/aInc"));
    } else if (useNameForShortcut) {
        aSet->setShortcuts(sc.getShortcut("Player/aSetCounter_" + name));
        aDec->setShortcuts(sc.getShortcut("Player/aDecCounter_" + name));
        aInc->setShortcuts(sc.getShortcut("Player/aIncCounter_" + name));
    }
}

void AbstractCounter::setShortcutsInactive()
{
    if (!menu) {
        return;
    }

    shortcutActive = false;
    if (name == "life" || useNameForShortcut) {
        aSet->setShortcut(QKeySequence());
        aDec->setShortcut(QKeySequence());
        aInc->setShortcut(QKeySequence());
    }
}

void AbstractCounter::refreshShortcuts()
{
    if (shortcutActive) {
        setShortcutsActive();
    }
}

void AbstractCounter::mousePressEvent(QGraphicsSceneMouseEvent *event)
{
    auto *ruledActions = RuledActions::actions(player);
    if (isUnderMouse() && player->getPlayerInfo()->getLocalOrJudge() && ruledActions &&
        event->button() == Qt::LeftButton && value > 0 &&
        (ruledActions->tryPayRuledSpellWithCounter(name) || ruledActions->tryPayRuledAbilityWithCounter(name) ||
         ruledActions->tryPayRuledResolutionWithCounter(name))) { event->accept(); return; }
    if (manualManaEditsDisallowedInRuledGame(player, name)) { event->accept(); return; }
    if (!isUnderMouse() || !player->getPlayerInfo()->getLocalOrJudge()) {
        event->ignore();
        return;
    }

    if (event->button() == Qt::MiddleButton || QApplication::keyboardModifiers() & Qt::ShiftModifier) {
        if (menu) {
            menu->exec(event->screenPos());
        }
    } else {
        player->getPlayerActions()->actIncrementCounter(id, event->button() == Qt::LeftButton ? 1 : -1);
    }
    event->accept();
}

void AbstractCounter::hoverEnterEvent(QGraphicsSceneHoverEvent *)
{
    hovered = true;
    update();
}
void AbstractCounter::hoverLeaveEvent(QGraphicsSceneHoverEvent *)
{
    hovered = false;
    update();
}

void AbstractCounter::incrementCounter()
{
    if (manualManaEditsDisallowedInRuledGame(player, name)) {
        return;
    }
    player->getPlayerActions()->actIncrementCounter(id, static_cast<QAction *>(sender())->data().toInt());
}

void AbstractCounter::setCounter()
{
    if (manualManaEditsDisallowedInRuledGame(player, name)) {
        return;
    }
    QWidget *parent = nullptr;
    if (auto *view = scene() ? scene()->views().value(0) : nullptr) {
        parent = view->window();
    }

    dialogSemaphore = true;
    AbstractCounterDialog dlg(name, QString::number(value), parent);
    const int ok = dlg.exec();
    dialogSemaphore = false;

    if (deleteAfterDialog) {
        deleteLater();
        return;
    }
    if (!ok) {
        return;
    }

    Expression exp(value);
    player->getPlayerActions()->actSetCounter(id, static_cast<int>(exp.parse(dlg.textValue())));
}

AbstractCounterDialog::AbstractCounterDialog(const QString &name, const QString &value, QWidget *parent)
    : QInputDialog(parent)
{
    setWindowTitle(tr("Set counter"));
    setLabelText(tr("New value for counter '%1':").arg(name));
    setTextValue(value);
    qApp->installEventFilter(this);
}

bool AbstractCounterDialog::eventFilter(QObject *obj, QEvent *event)
{
    Q_UNUSED(obj);
    if (event->type() == QEvent::KeyPress) {
        QKeyEvent *keyEvent = static_cast<QKeyEvent *>(event);
        switch (keyEvent->key()) {
            case Qt::Key_Up:
                changeValue(+1);
                return true;
            case Qt::Key_Down:
                changeValue(-1);
                return true;
        }
    }
    return false;
}

void AbstractCounterDialog::changeValue(int diff)
{
    bool ok;
    int curValue = textValue().toInt(&ok);
    if (!ok) {
        return;
    }
    curValue += diff;
    setTextValue(QString::number(curValue));
}

void AbstractCounter::onValueChanged(int /*oldValue*/, int /*newValue*/, bool /*skipDamageAnimation*/)
{
    // Default: no feedback. Subclasses such as PlayerCounter override this to
    // flash the counter on meaningful changes (life gain/loss).
}
