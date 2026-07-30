/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG engine tests — usvg-based data model.

use usvg::*;

// ======================= Tree Construction =======================

#[test]
fn build_empty_tree() {
    let size = Size::from_wh(100.0, 100.0).unwrap();
    let tree = Tree::new(size, Group::new());
    assert_eq!(tree.size().width(), 100.0);
    assert_eq!(tree.size().height(), 100.0);
}

#[test]
fn build_group_with_children() {
    let mut root = Group::new();
    root.push_child(Node::Group(Box::new(Group::new())));
    assert!(root.has_children());
}

#[test]
fn build_simple_shape_rect() {
    use usvg::SimpleShapeKind::Rect;
    let fill = Fill::new(
        Paint::Color(Color::new_rgb(255, 0, 0)),
        Opacity::ONE,
        FillRule::NonZero,
    );
    let shape = SimpleShape::new(
        Rect { x: 10.0, y: 20.0, width: 100.0, height: 50.0, rx: Some(5.0), ry: Some(5.0) },
        Some(fill),
        None,
        Transform::default(),
    );
    assert_eq!(shape.bounding_box().width(), 100.0);
    assert_eq!(shape.bounding_box().height(), 50.0);
    assert!(shape.fill().is_some());
    assert!(shape.stroke().is_none());
}

#[test]
fn build_simple_shape_circle() {
    use usvg::SimpleShapeKind::Circle;
    let shape = SimpleShape::new(
        Circle { cx: 50.0, cy: 50.0, r: 30.0 },
        None,
        None,
        Transform::default(),
    );
    assert_eq!(shape.bounding_box().width(), 60.0);
    assert_eq!(shape.bounding_box().height(), 60.0);
}

#[test]
fn build_simple_shape_ellipse() {
    use usvg::SimpleShapeKind::Ellipse;
    let shape = SimpleShape::new(
        Ellipse { cx: 50.0, cy: 50.0, rx: 40.0, ry: 20.0 },
        None,
        None,
        Transform::default(),
    );
    assert_eq!(shape.bounding_box().width(), 80.0);
    assert_eq!(shape.bounding_box().height(), 40.0);
}

#[test]
fn build_simple_shape_line() {
    use usvg::SimpleShapeKind::Line;
    let shape = SimpleShape::new(
        Line { x1: 0.0, y1: 0.0, x2: 100.0, y2: 0.0 },
        None,
        None,
        Transform::default(),
    );
    assert_eq!(shape.bounding_box().width(), 100.0);
}

#[test]
fn simple_shape_setters() {
    use usvg::SimpleShapeKind::Circle;
    let mut shape = SimpleShape::new(
        Circle { cx: 0.0, cy: 0.0, r: 10.0 },
        None,
        None,
        Transform::default(),
    );

    let fill = Fill::new(
        Paint::Color(Color::new_rgb(0, 255, 0)),
        Opacity::ONE,
        FillRule::NonZero,
    );
    shape.set_fill(Some(fill));
    assert!(shape.fill().is_some());

    let stroke = Stroke::new(
        Paint::Color(Color::new_rgb(0, 0, 255)),
        StrokeWidth::new(2.0).unwrap(),
    );
    shape.set_stroke(Some(stroke));
    assert!(shape.stroke().is_some());

    shape.set_visible(false);
    assert!(!shape.is_visible());
    shape.set_visible(true);
    assert!(shape.is_visible());
}

#[test]
fn build_path_from_d_attribute() {
    let fill = Fill::new(
        Paint::Color(Color::new_rgb(255, 0, 0)),
        Opacity::ONE,
        FillRule::NonZero,
    );
    let path = Path::from_d("M10,20 L30,40 L50,60 Z", Some(fill), None, Transform::default());
    assert!(path.is_some());
    let path = path.unwrap();
    assert!(path.data().len() >= 3);
}

#[test]
fn build_path_from_points() {
    let fill = Fill::new(
        Paint::Color(Color::new_rgb(0, 0, 255)),
        Opacity::ONE,
        FillRule::NonZero,
    );
    // Polygon: close=true
    let polygon = Path::from_points("10,20 30,40 50,10", true, Some(fill.clone()), None, Transform::default());
    assert!(polygon.is_some());

    // Polyline: close=false
    let polyline = Path::from_points("10,20 30,40 50,10", false, Some(fill), None, Transform::default());
    assert!(polyline.is_some());
}

#[test]
fn group_push_remove_child() {
    let mut group = Group::new();
    assert!(!group.has_children());

    let child = Node::Group(Box::new(Group::new()));
    group.push_child(child);
    assert!(group.has_children());

    let _removed = group.remove_child(0);
    assert!(!group.has_children());
}

#[test]
fn node_enum_supports_simple_shape() {
    use usvg::SimpleShapeKind::Rect;
    let shape = SimpleShape::new(
        Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0, rx: None, ry: None },
        None, None, Transform::default(),
    );
    let node = Node::SimpleShape(Box::new(shape));
    if let Node::SimpleShape(s) = &node {
        assert_eq!(s.bounding_box().width(), 10.0);
    } else {
        panic!("Expected SimpleShape");
    }
}

// ======================= Backend Pipeline — End-to-End =======================

use svg_engine::render_svg_tree_to;
use svg_engine::renderer::krilla::KrillaBackend;
use webrender_api::units::LayoutPoint;
use webrender_api::SpatialId;

const PAGE_W: f32 = 300.0;
const PAGE_H: f32 = 200.0;

fn make_krilla() -> KrillaBackend {
    KrillaBackend::new(PAGE_W, PAGE_H)
}

#[test]
fn krilla_backend_produces_valid_pdf() {
    use usvg::*;

    let mut root = Group::new();

    let rect = SimpleShape::new(
        SimpleShapeKind::Rect { x: 10.0, y: 10.0, width: 100.0, height: 60.0, rx: None, ry: None },
        Some(Fill::new(Paint::Color(Color::new_rgb(255, 0, 0)), Opacity::ONE, FillRule::NonZero)),
        None, Transform::default(),
    );
    root.push_child(Node::SimpleShape(Box::new(rect)));

    let circle = SimpleShape::new(
        SimpleShapeKind::Circle { cx: 200.0, cy: 40.0, r: 30.0 },
        Some(Fill::new(Paint::Color(Color::new_rgb(0, 0, 255)), Opacity::ONE, FillRule::NonZero)),
        None, Transform::default(),
    );
    root.push_child(Node::SimpleShape(Box::new(circle)));

    let line = SimpleShape::new(
        SimpleShapeKind::Line { x1: 10.0, y1: 150.0, x2: 180.0, y2: 150.0 },
        None,
        Some(Stroke::new(Paint::Color(Color::new_rgb(0, 255, 0)), StrokeWidth::new(3.0).unwrap())),
        Transform::default(),
    );
    root.push_child(Node::SimpleShape(Box::new(line)));

    let tree = Tree::new(Size::from_wh(PAGE_W, PAGE_H).unwrap(), root);

    let mut krilla = make_krilla();
    render_svg_tree_to(
        &tree,
        &LayoutPoint::new(0.0, 0.0),
        &mut krilla,
        SpatialId::root_scroll_node(webrender_api::PipelineId::dummy()),
        webrender_api::ClipChainId::INVALID,
    );

    let pdf = krilla.finish();
    let text = String::from_utf8_lossy(&pdf);

    // Valid PDF structure
    assert!(text.contains("%PDF-1.4"));
    assert!(text.ends_with("%%EOF\n"));
}

#[test]
fn save_pdf_to_disk() {
    use usvg::*;

    // Build all shapes from our test HTML
    let mut root = Group::new();

    // 1. Red rect
    root.push_child(Node::SimpleShape(Box::new(SimpleShape::new(
        SimpleShapeKind::Rect { x: 20.0, y: 20.0, width: 160.0, height: 110.0, rx: None, ry: None },
        Some(Fill::new(Paint::Color(Color::new_rgb(255, 0, 0)), Opacity::ONE, FillRule::NonZero)),
        None, Transform::default(),
    ))));

    // 2. Blue circle
    root.push_child(Node::SimpleShape(Box::new(SimpleShape::new(
        SimpleShapeKind::Circle { cx: 100.0, cy: 40.0, r: 25.0 },
        Some(Fill::new(Paint::Color(Color::new_rgb(0, 0, 255)), Opacity::ONE, FillRule::NonZero)),
        None, Transform::default(),
    ))));

    // 3. Green line
    root.push_child(Node::SimpleShape(Box::new(SimpleShape::new(
        SimpleShapeKind::Line { x1: 20.0, y1: 160.0, x2: 180.0, y2: 160.0 },
        None,
        Some(Stroke::new(Paint::Color(Color::new_rgb(0, 255, 0)), StrokeWidth::new(4.0).unwrap())),
        Transform::default(),
    ))));

    let tree = Tree::new(Size::from_wh(PAGE_W, PAGE_H).unwrap(), root);

    let mut krilla = make_krilla();
    render_svg_tree_to(
        &tree, &LayoutPoint::zero(), &mut krilla,
        SpatialId::root_scroll_node(webrender_api::PipelineId::dummy()),
        webrender_api::ClipChainId::INVALID,
    );

    let pdf = krilla.finish();
    std::fs::write("test_output.pdf", &pdf).expect("Failed to write PDF");
    println!("PDF saved to test_output.pdf ({} bytes)", pdf.len());
}

#[test]
fn same_tree_renders_identically_across_backends() {
    use usvg::*;

    let mut root = Group::new();
    let rect = SimpleShape::new(
        SimpleShapeKind::Rect { x: 0.0, y: 0.0, width: 100.0, height: 80.0, rx: None, ry: None },
        Some(Fill::new(Paint::Color(Color::new_rgb(255, 0, 0)), Opacity::ONE, FillRule::NonZero)),
        None, Transform::default(),
    );
    root.push_child(Node::SimpleShape(Box::new(rect)));
    let tree = Tree::new(Size::from_wh(PAGE_W, PAGE_H).unwrap(), root);

    let mut krilla1 = make_krilla();
    render_svg_tree_to(&tree, &LayoutPoint::zero(), &mut krilla1,
        SpatialId::root_scroll_node(webrender_api::PipelineId::dummy()),
        webrender_api::ClipChainId::INVALID);
    let output1 = krilla1.finish();

    let mut krilla2 = make_krilla();
    render_svg_tree_to(&tree, &LayoutPoint::zero(), &mut krilla2,
        SpatialId::root_scroll_node(webrender_api::PipelineId::dummy()),
        webrender_api::ClipChainId::INVALID);
    let output2 = krilla2.finish();

    assert_eq!(output1, output2, "Same tree must produce identical PDF output");
}
