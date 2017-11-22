use cgmath::Point2;
use num::zero;
use spade::SpatialObject;
use spade::BoundingRect;

/// A spatial point, to be stored in an R tree from the spade crate.
#[derive(Debug)]
pub struct SpatialPoint {
    /// The point's coordinates.
    pub center: Point2<f64>,
    /// The associated OSM id.
    pub id: i64,
}

impl SpatialPoint {
    /// Create a new point.
    pub fn new(center: Point2<f64>, id: i64) -> SpatialPoint {
        SpatialPoint {
            center: center,
            id: id,
        }
    }
}

impl SpatialObject for SpatialPoint {
    type Point = Point2<f64>;

    fn mbr(&self) -> BoundingRect<Point2<f64>> {
        BoundingRect::from_corners(&(self.center.clone()), &(self.center.clone()))
    }

    fn distance2(&self, point: &Point2<f64>) -> f64 {
        let dx = self.center[0] - point[0];
        let dy = self.center[1] - point[1];
        let dist = (dx * dx + dy * dy).sqrt().max(zero());
        dist * dist
    }

    // Nothing is contained within a point.
    fn contains(&self, point: &Point2<f64>) -> bool {
        false
    }
}
