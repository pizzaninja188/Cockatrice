#include "game/ruled/ruled_auxiliary_area.h"

#include <QGraphicsRectItem>
#include <QPen>
#include <gtest/gtest.h>

TEST(RuledAuxiliaryArea, FitsIndependentSectionsSideBySideInsideTheLandRow)
{
    RuledAuxiliaryArea area;
    auto *commanders = new QGraphicsRectItem(QRectF(0, 0, 72, 108));
    auto *emblems = new QGraphicsRectItem(QRectF(0, 0, 60, 50));
    area.addSection(commanders);
    area.addSection(emblems);

    const QRectF landRow(0, 250, 720, 140);
    area.anchorTo(landRow);
    EXPECT_TRUE(landRow.contains(area.sceneBoundingRect()));
    EXPECT_LE(commanders->sceneBoundingRect().right(), emblems->sceneBoundingRect().left());
}

TEST(RuledAuxiliaryArea, ReanchorsOnMirroringAndResizingAndReleasesEmptySpace)
{
    RuledAuxiliaryArea area;
    auto *commanders = new QGraphicsRectItem(QRectF(0, 0, 72, 108));
    commanders->setPen(Qt::NoPen);
    area.addSection(commanders);
    const QRectF lowerLandRow(0, 270, 900, 135);
    area.anchorTo(lowerLandRow);
    const auto singleWidth = area.reservedWidth();
    EXPECT_EQ(area.sceneBoundingRect().right(), lowerLandRow.right() - 8);

    commanders->setRect(QRectF(0, 0, 148, 108));
    area.relayout();
    EXPECT_EQ(area.reservedWidth(), singleWidth + 76);
    EXPECT_EQ(area.sceneBoundingRect().right(), lowerLandRow.right() - 8);

    const QRectF upperLandRow(0, 0, 410, 135);
    area.anchorTo(upperLandRow);
    EXPECT_TRUE(upperLandRow.contains(area.sceneBoundingRect()));
    EXPECT_EQ(area.sceneBoundingRect().right(), upperLandRow.right() - 8);

    commanders->setRect(QRectF());
    area.relayout();
    EXPECT_EQ(area.reservedWidth(), 0);
}
