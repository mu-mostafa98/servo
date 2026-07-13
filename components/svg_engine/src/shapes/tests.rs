/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Tests for SVG shape types — constructed directly as pure data structs.

#[cfg(test)]
mod tests {
    use crate::shapes::{Circle, Ellipse, Line, Path, Polygon, Polyline, Rectangle, Shape};
    use kurbo::BezPath;

    #[test]
    fn rectangle_basic() {
        let r = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
            rx: None,
            ry: None,
        };
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 0.0);
        assert_eq!(r.width, 100.0);
        assert_eq!(r.height, 50.0);
        assert!(r.rx.is_none());
        assert!(r.ry.is_none());
    }

    #[test]
    fn rectangle_with_position_and_radii() {
        let r = Rectangle {
            x: 10.0,
            y: 20.0,
            width: 200.0,
            height: 100.0,
            rx: Some(5.0),
            ry: Some(3.0),
        };
        assert_eq!(r.x, 10.0);
        assert_eq!(r.width, 200.0);
        assert_eq!(r.rx, Some(5.0));
        assert_eq!(r.ry, Some(3.0));
    }

    #[test]
    fn circle_basic() {
        let c = Circle {
            cx: 0.0,
            cy: 0.0,
            r: 25.0,
        };
        assert_eq!(c.r, 25.0);
    }

    #[test]
    fn circle_with_position() {
        let c = Circle {
            cx: 50.0,
            cy: 50.0,
            r: 30.0,
        };
        assert_eq!(c.cx, 50.0);
        assert_eq!(c.cy, 50.0);
        assert_eq!(c.r, 30.0);
    }

    #[test]
    fn ellipse_basic() {
        let e = Ellipse {
            cx: 0.0,
            cy: 0.0,
            rx: 40.0,
            ry: 20.0,
        };
        assert_eq!(e.rx, 40.0);
        assert_eq!(e.ry, 20.0);
    }

    #[test]
    fn line_basic() {
        let l = Line {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
        };
        assert_eq!(l.x1, 0.0);
        assert_eq!(l.y2, 100.0);
    }

    #[test]
    fn line_defaults() {
        let l = Line {
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: 0.0,
        };
        assert_eq!(l.x1, 0.0);
        assert_eq!(l.y2, 0.0);
    }

    #[test]
    fn polyline_basic() {
        let p = Polyline {
            points: vec![
                kurbo::Point::new(0.0, 0.0),
                kurbo::Point::new(100.0, 0.0),
                kurbo::Point::new(100.0, 100.0),
            ],
        };
        assert_eq!(p.points.len(), 3);
    }

    #[test]
    fn polygon_basic() {
        let p = Polygon {
            points: vec![
                kurbo::Point::new(0.0, 0.0),
                kurbo::Point::new(100.0, 0.0),
                kurbo::Point::new(100.0, 100.0),
                kurbo::Point::new(0.0, 100.0),
            ],
        };
        assert_eq!(p.points.len(), 4);
    }

    #[test]
    fn path_basic() {
        let p = Path {
            path: BezPath::from_svg("M 10 10 L 100 100").unwrap(),
        };
        assert!(!p.path.is_empty());
    }

    #[test]
    fn all_shapes_can_be_wrapped_in_shape_enum() {
        let rect = Shape::Rect(Rectangle {
            width: 10.0,
            height: 20.0,
            x: 0.0,
            y: 0.0,
            rx: None,
            ry: None,
        });
        assert!(matches!(rect, Shape::Rect(_)));

        let circle = Shape::Circle(Circle {
            cx: 0.0,
            cy: 0.0,
            r: 5.0,
        });
        assert!(matches!(circle, Shape::Circle(_)));

        let ellipse = Shape::Ellipse(Ellipse {
            cx: 0.0,
            cy: 0.0,
            rx: 5.0,
            ry: 3.0,
        });
        assert!(matches!(ellipse, Shape::Ellipse(_)));

        let line = Shape::Line(Line {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        });
        assert!(matches!(line, Shape::Line(_)));

        let polyline = Shape::Polyline(Polyline {
            points: vec![kurbo::Point::new(0.0, 0.0), kurbo::Point::new(10.0, 0.0)],
        });
        assert!(matches!(polyline, Shape::Polyline(_)));

        let polygon = Shape::Polygon(Polygon {
            points: vec![
                kurbo::Point::new(0.0, 0.0),
                kurbo::Point::new(10.0, 0.0),
                kurbo::Point::new(5.0, 10.0),
            ],
        });
        assert!(matches!(polygon, Shape::Polygon(_)));

        let path = Shape::Path(Path {
            path: BezPath::from_svg("M 0 0 L 10 10").unwrap(),
        });
        assert!(matches!(path, Shape::Path(_)));
    }
}
