#include "phases_toolbar.h"

#include "../interface/pixel_map_generator.h"

#include <QAction>
#include <QDebug>
#include <QPainter>
#include <QPen>
#include <QGraphicsSceneMouseEvent>
#include <QTimer>
#include <libcockatrice/protocol/pb/command_draw_cards.pb.h>
#include <libcockatrice/protocol/pb/command_next_turn.pb.h>
#include <libcockatrice/protocol/pb/command_set_active_phase.pb.h>
#include <libcockatrice/protocol/pb/command_set_card_attr.pb.h>
#include <libcockatrice/utility/zone_names.h>

PhaseButton::PhaseButton(const QString &_name, QGraphicsItem *parent, QAction *_doubleClickAction, bool _highlightable)
    : QObject(), QGraphicsItem(parent), name(_name), active(false), highlightable(_highlightable),
      hasStops(true), opponentTurnStopEnabled(false), myTurnStopEnabled(false), activeAnimationCounter(0),
      doubleClickAction(_doubleClickAction), width(50), height(50), stopIndicatorWidth(0), stopIndicatorGap(0)
{
    if (highlightable) {
        activeAnimationTimer = new QTimer(this);
        connect(activeAnimationTimer, &QTimer::timeout, this, &PhaseButton::updateAnimation);
        activeAnimationTimer->setSingleShot(false);
    } else
        activeAnimationCounter = 9;

    setCacheMode(DeviceCoordinateCache);
}

QRectF PhaseButton::boundingRect() const
{
    return {0, 0, width, height};
}

void PhaseButton::paint(QPainter *painter, const QStyleOptionGraphicsItem * /*option*/, QWidget * /*widget*/)
{
    const QRectF contentRect = boundingRect().adjusted(3, 3, -3, -3);
    const qreal iconLeft = hasStops ? stopIndicatorWidth + stopIndicatorGap + 3.0 : 3.0;
    QRectF iconRect(iconLeft, 3, width - iconLeft - 3, height - 6);
    QRectF translatedIconRect = painter->combinedTransform().mapRect(iconRect);
    qreal scaleFactor = translatedIconRect.width() / iconRect.width();
    QPixmap iconPixmap = PhasePixmapGenerator::generatePixmap(qRound(translatedIconRect.height()), name);

    painter->setBrush(QColor(static_cast<int>(220 * (activeAnimationCounter / 10.0)),
                             static_cast<int>(220 * (activeAnimationCounter / 10.0)),
                             static_cast<int>(220 * (activeAnimationCounter / 10.0))));
    painter->setPen(Qt::gray);
    painter->drawRect(0, 0, static_cast<int>(width - 1), static_cast<int>(height - 1));
    painter->save();
    resetPainterTransform(painter);
    painter->drawPixmap(iconPixmap.rect().translated(qRound(iconRect.x() * scaleFactor), qRound(3 * scaleFactor)),
                        iconPixmap, iconPixmap.rect());
    painter->restore();

    painter->setBrush(QColor(0, 0, 0, static_cast<int>(255 * ((10 - activeAnimationCounter) / 15.0))));
    painter->setPen(Qt::gray);
    painter->drawRect(0, 0, static_cast<int>(width - 1), static_cast<int>(height - 1));

    if (hasStops) {
        const qreal boxSize = stopIndicatorWidth;
        const qreal left = 3;
        const qreal topY = contentRect.top();
        const qreal bottomY = contentRect.bottom() - boxSize;
        const QRectF oppRect(left, topY, boxSize, boxSize);
        const QRectF myRect(left, bottomY, boxSize, boxSize);

        painter->setPen(Qt::lightGray);
        painter->setBrush(opponentTurnStopEnabled ? QColor(220, 130, 50) : QColor(45, 45, 45));
        painter->drawRect(oppRect);
        painter->setBrush(myTurnStopEnabled ? QColor(50, 170, 220) : QColor(45, 45, 45));
        painter->drawRect(myRect);
    }
}

void PhaseButton::setWidth(double _width)
{
    prepareGeometryChange();
    width = _width;
}

void PhaseButton::setHeight(double _height)
{
    prepareGeometryChange();
    height = _height;
}

void PhaseButton::setStopIndicatorsLayout(double indicatorWidth, double indicatorGap)
{
    stopIndicatorWidth = indicatorWidth;
    stopIndicatorGap = indicatorGap;
}

void PhaseButton::setHasStops(bool enabled)
{
    hasStops = enabled;
    update();
}

void PhaseButton::setOpponentTurnStopEnabled(bool enabled)
{
    opponentTurnStopEnabled = enabled;
    update();
}

void PhaseButton::setMyTurnStopEnabled(bool enabled)
{
    myTurnStopEnabled = enabled;
    update();
}

bool PhaseButton::hasStopOnOpponentTurn() const
{
    return opponentTurnStopEnabled;
}

bool PhaseButton::hasStopOnMyTurn() const
{
    return myTurnStopEnabled;
}

void PhaseButton::setActive(bool _active)
{
    if ((active == _active) || !highlightable)
        return;

    active = _active;
    activeAnimationTimer->start(25);
}

void PhaseButton::updateAnimation()
{
    if (!highlightable)
        return;

    // the counter ticks up to 10 when active and down to 0 when inactive
    if (active && activeAnimationCounter < 10) {
        ++activeAnimationCounter;
    } else if (!active && activeAnimationCounter > 0) {
        --activeAnimationCounter;
    } else {
        activeAnimationTimer->stop();
    }

    update();
}

void PhaseButton::mousePressEvent(QGraphicsSceneMouseEvent *event)
{
    if (event && toggleStopAtPosition(event->pos())) {
        return;
    }
    emit clicked();
}

bool PhaseButton::toggleStopAtPosition(const QPointF &pos)
{
    if (!hasStops) {
        return false;
    }

    const qreal left = 3;
    const qreal boxSize = stopIndicatorWidth;
    const qreal topY = 3;
    const qreal bottomY = height - 3 - boxSize;
    const QRectF opponentRect(left, topY, boxSize, boxSize);
    const QRectF myRect(left, bottomY, boxSize, boxSize);

    if (opponentRect.contains(pos)) {
        opponentTurnStopEnabled = !opponentTurnStopEnabled;
        emit stopToggled(true, opponentTurnStopEnabled);
        update();
        return true;
    }
    if (myRect.contains(pos)) {
        myTurnStopEnabled = !myTurnStopEnabled;
        emit stopToggled(false, myTurnStopEnabled);
        update();
        return true;
    }

    return false;
}

void PhaseButton::mouseDoubleClickEvent(QGraphicsSceneMouseEvent * /*event*/)
{
    triggerDoubleClickAction();
}

void PhaseButton::triggerDoubleClickAction()
{
    if (doubleClickAction)
        doubleClickAction->trigger();
}

PhasesToolbar::PhasesToolbar(QGraphicsItem *parent)
    : QGraphicsItem(parent), width(100), height(100), ySpacing(1), symbolSize(8),
      stopOnOpponentTurn({false, false, false, false, true, true, true, false, false, false, true}),
      stopOnMyTurn({false, false, false, true, false, true, true, false, false, true, false})
{
    auto *aUntapAll = new QAction(this);
    connect(aUntapAll, &QAction::triggered, this, &PhasesToolbar::actUntapAll);
    auto *aDrawCard = new QAction(this);
    connect(aDrawCard, &QAction::triggered, this, &PhasesToolbar::actDrawCard);

    PhaseButton *untapButton = new PhaseButton("untap", this, aUntapAll);
    PhaseButton *upkeepButton = new PhaseButton("upkeep", this);
    PhaseButton *drawButton = new PhaseButton("draw", this, aDrawCard);
    PhaseButton *main1Button = new PhaseButton("main1", this);
    PhaseButton *combatStartButton = new PhaseButton("combat_start", this);
    PhaseButton *combatAttackersButton = new PhaseButton("combat_attackers", this);
    PhaseButton *combatBlockersButton = new PhaseButton("combat_blockers", this);
    PhaseButton *combatDamageButton = new PhaseButton("combat_damage", this);
    PhaseButton *combatEndButton = new PhaseButton("combat_end", this);
    PhaseButton *main2Button = new PhaseButton("main2", this);
    PhaseButton *cleanupButton = new PhaseButton("cleanup", this);

    buttonList << untapButton << upkeepButton << drawButton << main1Button << combatStartButton << combatAttackersButton
               << combatBlockersButton << combatDamageButton << combatEndButton << main2Button << cleanupButton;

    for (auto &i : buttonList)
        connect(i, &PhaseButton::clicked, this, &PhasesToolbar::phaseButtonClicked);
    for (int i = 0; i < buttonList.size(); ++i) {
        connect(buttonList[i], &PhaseButton::stopToggled, this, [this, i](bool opponentTurn, bool enabled) {
            if (opponentTurn) {
                stopOnOpponentTurn[i] = enabled;
            } else {
                stopOnMyTurn[i] = enabled;
            }
        });
    }

    nextTurnButton = new PhaseButton("nextturn", this, nullptr, false);
    connect(nextTurnButton, &PhaseButton::clicked, this, &PhasesToolbar::actNextTurn);

    rearrangeButtons();
    syncButtonStopsFromState();

    retranslateUi();
}

QRectF PhasesToolbar::boundingRect() const
{
    return {0, 0, width, height};
}

void PhasesToolbar::retranslateUi()
{
    for (int i = 0; i < buttonList.size(); ++i) {
        if (i == 0) {
            buttonList[i]->setToolTip(getLongPhaseName(i));
        } else {
            buttonList[i]->setToolTip(getLongPhaseName(i) + QLatin1String("\n")
                                      + tr("Top box: stop on opponent's turn\nBottom box: stop on your turn"));
        }
    }
}

QString PhasesToolbar::getLongPhaseName(int phase) const
{
    switch (phase) {
        case 0:
            return tr("Untap step");
        case 1:
            return tr("Upkeep step");
        case 2:
            return tr("Draw step");
        case 3:
            return tr("First main phase");
        case 4:
            return tr("Beginning of combat step");
        case 5:
            return tr("Declare attackers step");
        case 6:
            return tr("Declare blockers step");
        case 7:
            return tr("Combat damage step");
        case 8:
            return tr("End of combat step");
        case 9:
            return tr("Second main phase");
        case 10:
            return tr("End of turn step");
        default:
            return QString();
    }
}

void PhasesToolbar::paint(QPainter *painter, const QStyleOptionGraphicsItem * /*option*/, QWidget * /*widget*/)
{
    painter->fillRect(boundingRect(), QColor(50, 50, 50));
}

const double PhasesToolbar::marginSize = 3;

void PhasesToolbar::rearrangeButtons()
{
    const double stopIndicatorWidth = symbolSize * 0.28;
    const double stopIndicatorGap = symbolSize * 0.18;
    const double phaseButtonWidth = symbolSize + stopIndicatorWidth + stopIndicatorGap;

    for (int i = 0; i < buttonList.size(); ++i) {
        buttonList[i]->setHasStops(i != 0);
        buttonList[i]->setStopIndicatorsLayout(stopIndicatorWidth, stopIndicatorGap);
        buttonList[i]->setHeight(symbolSize);
        buttonList[i]->setWidth(phaseButtonWidth);
    }
    nextTurnButton->setHasStops(false);
    nextTurnButton->setHeight(symbolSize);
    nextTurnButton->setWidth(symbolSize);

    double y = marginSize;
    buttonList[0]->setPos(marginSize, y);
    buttonList[1]->setPos(marginSize, y += symbolSize);
    buttonList[2]->setPos(marginSize, y += symbolSize);
    y += ySpacing;
    buttonList[3]->setPos(marginSize, y += symbolSize);
    y += ySpacing;
    buttonList[4]->setPos(marginSize, y += symbolSize);
    buttonList[5]->setPos(marginSize, y += symbolSize);
    buttonList[6]->setPos(marginSize, y += symbolSize);
    buttonList[7]->setPos(marginSize, y += symbolSize);
    buttonList[8]->setPos(marginSize, y += symbolSize);
    y += ySpacing;
    buttonList[9]->setPos(marginSize, y += symbolSize);
    y += ySpacing;
    buttonList[10]->setPos(marginSize, y += symbolSize);
    y += ySpacing;
    y += ySpacing;
    nextTurnButton->setPos(marginSize, y + symbolSize);
}

void PhasesToolbar::setHeight(double _height)
{
    prepareGeometryChange();

    height = _height;
    ySpacing = (height - 2 * marginSize) / (buttonCount * 5 + spaceCount);
    symbolSize = ySpacing * 5;
    width = symbolSize + (symbolSize * 0.28) + (symbolSize * 0.18) + 2 * marginSize;

    rearrangeButtons();
}

void PhasesToolbar::setActivePhase(int phase)
{
    if (phase >= buttonList.size())
        return;

    for (int i = 0; i < buttonList.size(); ++i)
        buttonList[i]->setActive(i == phase);
}

void PhasesToolbar::triggerPhaseAction(int phase)
{
    if (0 <= phase && phase < buttonList.size()) {
        buttonList[phase]->triggerDoubleClickAction();
    }
}

bool PhasesToolbar::shouldStopAtPhase(int phase, bool myTurn) const
{
    if (phase < 0 || phase >= static_cast<int>(stopOnMyTurn.size())) {
        return false;
    }
    if (myTurn) {
        return stopOnMyTurn[phase];
    }
    return stopOnOpponentTurn[phase];
}

void PhasesToolbar::syncButtonStopsFromState()
{
    for (int i = 0; i < buttonList.size(); ++i) {
        buttonList[i]->setOpponentTurnStopEnabled(stopOnOpponentTurn[i]);
        buttonList[i]->setMyTurnStopEnabled(stopOnMyTurn[i]);
    }
}

void PhasesToolbar::phaseButtonClicked()
{
    auto *button = qobject_cast<PhaseButton *>(sender());
    if (button->getActive())
        button->triggerDoubleClickAction();

    Command_SetActivePhase cmd;
    cmd.set_phase(static_cast<google::protobuf::uint32>(buttonList.indexOf(button)));

    emit sendGameCommand(cmd, -1);
}

void PhasesToolbar::actNextTurn()
{
    emit sendGameCommand(Command_NextTurn(), -1);
}

void PhasesToolbar::actUntapAll()
{
    Command_SetCardAttr cmd;
    cmd.set_zone(ZoneNames::TABLE);
    cmd.set_attribute(AttrTapped);
    cmd.set_attr_value("0");

    emit sendGameCommand(cmd, -1);
}

void PhasesToolbar::actDrawCard()
{
    Command_DrawCards cmd;
    cmd.set_number(1);

    emit sendGameCommand(cmd, -1);
}
