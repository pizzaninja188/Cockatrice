#pragma once

#include <QGraphicsObject>
#include <QList>

// Shared presentation container for command-zone cards and future emblem/effect sections.
// Sections retain their own model, identity and interaction behavior.
class RuledAuxiliaryArea : public QGraphicsObject
{
    Q_OBJECT
public:
    explicit RuledAuxiliaryArea(QGraphicsItem *parent = nullptr);
    QRectF boundingRect() const override
    {
        return bounds;
    }
    void paint(QPainter *, const QStyleOptionGraphicsItem *, QWidget *) override;
    void addSection(QGraphicsItem *section);
    void anchorTo(const QRectF &battlefield, bool localPlayer);
    qreal reservedWidth() const
    {
        return bounds.isEmpty() ? 0 : bounds.width() + 16;
    }
public slots:
    void relayout();
signals:
    void extentChanged();

private:
    QList<QGraphicsItem *> sections;
    QRectF bounds;
    QRectF battlefield;
    bool localPlayer = false;
};
