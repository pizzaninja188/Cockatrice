#include "game/ruled/ruled_auxiliary_area.h"

#include <QGraphicsRectItem>
#include <gtest/gtest.h>

TEST(RuledAuxiliaryArea, AnchorsEveryPlayerAndKeepsIndependentSections)
{
    RuledAuxiliaryArea area;
    auto *commanders = new QGraphicsRectItem(QRectF(0, 0, 164, 128));
    auto *emblems = new QGraphicsRectItem(QRectF(0, 0, 90, 40));
    area.addSection(commanders);
    area.addSection(emblems);
    const QRectF battlefield(25, 50, 900, 400);
    area.anchorTo(battlefield, true);
    EXPECT_EQ(area.sceneBoundingRect().right(), battlefield.right() - 8);
    EXPECT_EQ(area.sceneBoundingRect().bottom(), battlefield.bottom() - 8);
    EXPECT_FALSE(commanders->mapRectToParent(commanders->boundingRect())
                     .intersects(emblems->mapRectToParent(emblems->boundingRect())));
    area.anchorTo(battlefield, false);
    EXPECT_EQ(area.sceneBoundingRect().right(), battlefield.right() - 8);
    EXPECT_EQ(area.sceneBoundingRect().top(), battlefield.top() + 8);
    commanders->hide();
    area.relayout();
    EXPECT_LT(area.reservedWidth(), 164);
    emblems->hide();
    area.relayout();
    EXPECT_EQ(area.reservedWidth(), 0);
}
