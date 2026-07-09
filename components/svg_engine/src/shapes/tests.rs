/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Tests for the [`BuildFromElement`] factory trait.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::shapes::{
        AttrAccessor, BuildFromElement, Circle, Ellipse, Line, Path, Polygon, Polyline, Rectangle,
        Shape,
    };

    struct TestElement {
        attrs: HashMap<String, String>,
    }

    impl TestElement {
        fn new(attrs: &[(&str, &str)]) -> Self {
            TestElement {
                attrs: attrs
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            }
        }
    }

    impl AttrAccessor for TestElement {
        fn get_attr(&self, name: &str) -> Option<String> {
            self.attrs.get(name).cloned()
        }
    }

    #[test]
    fn rectangle_from_attrs_basic() {
        let el = TestElement::new(&[("width", "100"), ("height", "50")]);
        let r = Rectangle::from_attrs(16.0, &el).unwrap();
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 0.0);
        assert_eq!(r.width, 100.0);
        assert_eq!(r.height, 50.0);
        assert!(r.rx.is_none());
        assert!(r.ry.is_none());
    }

    #[test]
    fn rectangle_from_attrs_with_position_and_radii() {
        let el = TestElement::new(&[
            ("x", "10"),
            ("y", "20"),
            ("width", "200"),
            ("height", "100"),
            ("rx", "5"),
            ("ry", "3"),
        ]);
        let r = Rectangle::from_attrs(16.0, &el).unwrap();
        assert_eq!(r.x, 10.0);
        assert_eq!(r.y, 20.0);
        assert_eq!(r.width, 200.0);
        assert_eq!(r.height, 100.0);
        assert_eq!(r.rx, Some(5.0));
        assert_eq!(r.ry, Some(3.0));
    }

    #[test]
    fn rectangle_from_attrs_negative_width() {
        let el = TestElement::new(&[("width", "-100"), ("height", "50")]);
        assert!(Rectangle::from_attrs(16.0, &el).is_none());
    }

    #[test]
    fn rectangle_from_attrs_missing_width() {
        let el = TestElement::new(&[("height", "50")]);
        assert!(Rectangle::from_attrs(16.0, &el).is_none());
    }

    #[test]
    fn circle_from_attrs_basic() {
        let el = TestElement::new(&[("r", "25")]);
        let c = Circle::from_attrs(16.0, &el).unwrap();
        assert_eq!(c.cx, 0.0);
        assert_eq!(c.cy, 0.0);
        assert_eq!(c.r, 25.0);
    }

    #[test]
    fn circle_from_attrs_with_position() {
        let el = TestElement::new(&[("cx", "50"), ("cy", "50"), ("r", "30")]);
        let c = Circle::from_attrs(16.0, &el).unwrap();
        assert_eq!(c.cx, 50.0);
        assert_eq!(c.cy, 50.0);
        assert_eq!(c.r, 30.0);
    }

    #[test]
    fn circle_from_attrs_missing_r() {
        let el = TestElement::new(&[("cx", "50")]);
        assert!(Circle::from_attrs(16.0, &el).is_none());
    }

    #[test]
    fn ellipse_from_attrs_basic() {
        let el = TestElement::new(&[("rx", "40"), ("ry", "20")]);
        let e = Ellipse::from_attrs(16.0, &el).unwrap();
        assert_eq!(e.cx, 0.0);
        assert_eq!(e.cy, 0.0);
        assert_eq!(e.rx, 40.0);
        assert_eq!(e.ry, 20.0);
    }

    #[test]
    fn ellipse_from_attrs_missing_rx() {
        let el = TestElement::new(&[("ry", "20")]);
        assert!(Ellipse::from_attrs(16.0, &el).is_none());
    }

    #[test]
    fn line_from_attrs_basic() {
        let el = TestElement::new(&[("x1", "0"), ("y1", "0"), ("x2", "100"), ("y2", "100")]);
        let l = Line::from_attrs(16.0, &el).unwrap();
        assert_eq!(l.x1, 0.0);
        assert_eq!(l.y1, 0.0);
        assert_eq!(l.x2, 100.0);
        assert_eq!(l.y2, 100.0);
    }

    #[test]
    fn line_from_attrs_defaults() {
        let el = TestElement::new(&[]);
        let l = Line::from_attrs(16.0, &el).unwrap();
        assert_eq!(l.x1, 0.0);
        assert_eq!(l.y1, 0.0);
        assert_eq!(l.x2, 0.0);
        assert_eq!(l.y2, 0.0);
    }

    #[test]
    fn polyline_from_attrs_basic() {
        let el = TestElement::new(&[("points", "0,0 100,0 100,100")]);
        let p = Polyline::from_attrs(16.0, &el).unwrap();
        assert_eq!(p.points.len(), 3);
    }

    #[test]
    fn polyline_from_attrs_missing_points() {
        let el = TestElement::new(&[]);
        assert!(Polyline::from_attrs(16.0, &el).is_none());
    }

    #[test]
    fn polygon_from_attrs_basic() {
        let el = TestElement::new(&[("points", "0,0 100,0 100,100 0,100")]);
        let p = Polygon::from_attrs(16.0, &el).unwrap();
        assert_eq!(p.points.len(), 4);
    }

    #[test]
    fn polygon_from_attrs_missing_points() {
        let el = TestElement::new(&[]);
        assert!(Polygon::from_attrs(16.0, &el).is_none());
    }

    #[test]
    fn path_from_attrs_basic() {
        let el = TestElement::new(&[("d", "M 10 10 L 100 100")]);
        let p = Path::from_attrs(16.0, &el).unwrap();
        assert!(!p.path.is_empty());
    }

    #[test]
    fn path_from_attrs_missing_d() {
        let el = TestElement::new(&[]);
        assert!(Path::from_attrs(16.0, &el).is_none());
    }

    #[test]
    fn path_from_attrs_invalid_d() {
        let el = TestElement::new(&[("d", "NOT A PATH")]);
        assert!(Path::from_attrs(16.0, &el).is_none());
    }

    #[test]
    fn closure_implements_attr_accessor() {
        // Verify that closures can be used as AttrAccessor.
        let mut map = HashMap::new();
        map.insert("width".to_string(), "42".to_string());
        map.insert("height".to_string(), "10".to_string());
        let closure = |name: &str| map.get(name).cloned();
        let r = Rectangle::from_attrs(16.0, &closure).unwrap();
        assert_eq!(r.width, 42.0);
        assert_eq!(r.height, 10.0);
    }

    #[test]
    fn all_shapes_can_be_constructed_via_build_from_element() {
        let rect_el = TestElement::new(&[("width", "10"), ("height", "20")]);
        let shape = Shape::Rect(Rectangle::from_attrs(16.0, &rect_el).unwrap());
        assert!(matches!(shape, Shape::Rect(_)));

        let circle_el = TestElement::new(&[("r", "5")]);
        let shape = Shape::Circle(Circle::from_attrs(16.0, &circle_el).unwrap());
        assert!(matches!(shape, Shape::Circle(_)));

        let ellipse_el = TestElement::new(&[("rx", "5"), ("ry", "3")]);
        let shape = Shape::Ellipse(Ellipse::from_attrs(16.0, &ellipse_el).unwrap());
        assert!(matches!(shape, Shape::Ellipse(_)));

        let line_el = TestElement::new(&[("x1", "0"), ("y1", "0"), ("x2", "10"), ("y2", "10")]);
        let shape = Shape::Line(Line::from_attrs(16.0, &line_el).unwrap());
        assert!(matches!(shape, Shape::Line(_)));

        let polyline_el = TestElement::new(&[("points", "0,0 10,0")]);
        let shape = Shape::Polyline(Polyline::from_attrs(16.0, &polyline_el).unwrap());
        assert!(matches!(shape, Shape::Polyline(_)));

        let polygon_el = TestElement::new(&[("points", "0,0 10,0 5,10")]);
        let shape = Shape::Polygon(Polygon::from_attrs(16.0, &polygon_el).unwrap());
        assert!(matches!(shape, Shape::Polygon(_)));

        let path_el = TestElement::new(&[("d", "M 0 0 L 10 10")]);
        let shape = Shape::Path(Path::from_attrs(16.0, &path_el).unwrap());
        assert!(matches!(shape, Shape::Path(_)));
    }

    #[test]
    fn closure_attr_accessor_with_nonexistent_attr() {
        let map: HashMap<String, String> = HashMap::new();
        let closure = |name: &str| map.get(name).cloned();
        assert!(Circle::from_attrs(16.0, &closure).is_none());

        // Width/height for rect should fail
        assert!(Rectangle::from_attrs(16.0, &closure).is_none());
    }
}
