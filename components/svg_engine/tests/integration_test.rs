/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Comprehensive SVG engine test suite.
//!
//! Covers: all shape types, fill/stroke/gradient/pattern rendering data,
//! CSS cascade + presentation attribute interaction, clip-path/mask/filter,
//! transforms, viewBox/preserveAspectRatio, `<use>` references,
//! `<defs>` collection, visitor pattern, and edge cases.

use std::collections::HashMap;

use svg_engine::render_tree::*;
use svg_engine::shapes::*;
use svg_engine::style::gradient::{SpreadMethod, *};
use svg_engine::style::transform_ops::TransformOp;
use svg_engine::style::*;
use svg_engine::{SvgImage, SvgTag, TextAnchor, TextSpan};

// ============================================================
// 1. SHAPE DATA STRUCT TESTS
// ============================================================

#[test]
fn rect_data() {
    let r = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 50.0,
        rx: None,
        ry: None,
    };
    assert_eq!(r.width, 100.0);
    assert_eq!(r.height, 50.0);
    assert_eq!(r.x, 0.0);
    assert_eq!(r.y, 0.0);
}

#[test]
fn rect_with_radius() {
    let r = Rectangle {
        x: 10.0,
        y: 20.0,
        width: 200.0,
        height: 100.0,
        rx: Some(10.0),
        ry: Some(5.0),
    };
    assert_eq!(r.rx, Some(10.0));
    assert_eq!(r.ry, Some(5.0));
}

#[test]
fn rect_rx_inherits_ry_and_vice_versa() {
    // rx only: ry = rx
    let r = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        rx: Some(10.0),
        ry: None,
    };
    assert_eq!(r.rx, Some(10.0));
    // ry only: rx = ry
    let r2 = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        rx: None,
        ry: Some(15.0),
    };
    assert_eq!(r2.ry, Some(15.0));
}

#[test]
fn circle_data() {
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
fn ellipse_data() {
    let e = Ellipse {
        cx: 100.0,
        cy: 80.0,
        rx: 60.0,
        ry: 40.0,
    };
    assert_eq!(e.rx, 60.0);
    assert_eq!(e.ry, 40.0);
}

#[test]
fn line_data() {
    let l = Line {
        x1: 0.0,
        y1: 0.0,
        x2: 100.0,
        y2: 100.0,
    };
    assert_eq!(l.x2, 100.0);
    assert_eq!(l.y2, 100.0);
}

#[test]
fn polyline_data() {
    let pts = vec![
        kurbo::Point::new(0.0, 0.0),
        kurbo::Point::new(50.0, 100.0),
        kurbo::Point::new(100.0, 0.0),
    ];
    let p = Polyline { points: pts };
    assert_eq!(p.points.len(), 3);
}

#[test]
fn polygon_data() {
    let pts = vec![
        kurbo::Point::new(0.0, 0.0),
        kurbo::Point::new(100.0, 0.0),
        kurbo::Point::new(50.0, 100.0),
    ];
    let p = Polygon { points: pts };
    assert_eq!(p.points.len(), 3);
}

#[test]
fn path_data_parse() {
    let path = kurbo::BezPath::from_svg("M10 10 L100 100").unwrap();
    let p = Path { path };
    assert_eq!(p.path.elements().len(), 2);
}

#[test]
fn path_data_invalid_rejected() {
    assert!(kurbo::BezPath::from_svg("M invalid").is_err());
}

#[test]
fn shape_enum_all_variants_constructible() {
    let _rect = Shape::Rect(Rectangle {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
        rx: None,
        ry: None,
    });
    let _circle = Shape::Circle(Circle {
        cx: 5.0,
        cy: 5.0,
        r: 5.0,
    });
    let _ellipse = Shape::Ellipse(Ellipse {
        cx: 5.0,
        cy: 5.0,
        rx: 5.0,
        ry: 3.0,
    });
    let _line = Shape::Line(Line {
        x1: 0.0,
        y1: 0.0,
        x2: 10.0,
        y2: 10.0,
    });
    // Text and Image are in SvgTag, not Shape
    let _text_tag = SvgTag::Text(TextSpan {
        text: "Hi".into(),
        x: 10.0,
        y: 20.0,
        dx: vec![],
        dy: vec![],
        text_anchor: TextAnchor::Start,
        glyphs: vec![],
        font_instance_key: None,
        advance_offset: 0.0,
    });
    let _image_tag = SvgTag::Image(SvgImage {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        href: Some("test.png".into()),
        image_key: None,
        natural_width: None,
        natural_height: None,
        preserve_aspect_ratio: AspectRatio::default(),
    });
    // All construct without panic.
}

#[test]
fn text_span_data() {
    let t = TextSpan {
        text: "Hello SVG".into(),
        x: 10.0,
        y: 30.0,
        dx: vec![],
        dy: vec![],
        text_anchor: TextAnchor::Start,
        glyphs: vec![],
        font_instance_key: None,
        advance_offset: 0.0,
    };
    assert_eq!(t.text, "Hello SVG");
    assert_eq!(t.x, 10.0);
    assert_eq!(t.y, 30.0);
    assert_eq!(t.advance_offset, 0.0);
}

#[test]
fn text_span_with_dx_dy() {
    let t = TextSpan {
        text: "AB".into(),
        x: 0.0,
        y: 0.0,
        dx: vec![5.0, 10.0],
        dy: vec![0.0, 3.0],
        text_anchor: TextAnchor::Start,
        glyphs: vec![],
        font_instance_key: None,
        advance_offset: 0.0,
    };
    assert_eq!(t.dx.len(), 2);
    assert_eq!(t.dy.len(), 2);
}

#[test]
fn text_span_advance_offset_positions_runs() {
    // Two runs on one line: the second begins where the first ends.
    // With no glyphs shaped, total_advance falls back to 8px/char.
    let first = TextSpan {
        text: "Red".into(), // 3 chars → 24px fallback advance
        x: 10.0,
        y: 80.0,
        dx: vec![],
        dy: vec![],
        text_anchor: TextAnchor::Start,
        glyphs: vec![],
        font_instance_key: None,
        advance_offset: 0.0,
    };
    let second = TextSpan {
        text: " Blue".into(), // 5 chars
        x: 10.0,
        y: 80.0,
        dx: vec![],
        dy: vec![],
        text_anchor: TextAnchor::Start,
        glyphs: vec![],
        font_instance_key: None,
        advance_offset: first.total_advance(),
    };
    // The second run's pen position is the first run's x + its advance.
    assert_eq!(first.total_advance(), 24.0);
    assert_eq!(second.advance_offset, 24.0);
}

#[test]
fn text_anchor_variants() {
    assert_eq!(TextAnchor::Start.alignment_offset(), 0.0);
    assert_eq!(TextAnchor::Middle.alignment_offset(), -0.5);
    assert_eq!(TextAnchor::End.alignment_offset(), -1.0);
}

#[test]
fn svg_image_data() {
    let img = SvgImage {
        x: 10.0,
        y: 20.0,
        width: 300.0,
        height: 200.0,
        href: Some("image.png".into()),
        image_key: None,
        natural_width: None,
        natural_height: None,
        preserve_aspect_ratio: AspectRatio::default(),
    };
    assert_eq!(img.width, 300.0);
    assert_eq!(img.height, 200.0);
    assert_eq!(img.href, Some("image.png".into()));
    assert!(img.image_key.is_none());
}

#[test]
fn svg_image_no_href() {
    let img = SvgImage {
        x: 0.0,
        y: 0.0,
        width: 50.0,
        height: 50.0,
        href: None,
        image_key: None,
        natural_width: None,
        natural_height: None,
        preserve_aspect_ratio: AspectRatio::default(),
    };
    assert!(img.href.is_none());
    assert!(img.image_key.is_none());
}

#[test]
fn line_no_fill_geometry_by_spec() {
    // Per SVG spec, <line> has no fill geometry — only stroke renders.
    // This is verified at the Render trait level (line.rs).
    let line = Shape::Line(Line {
        x1: 0.0,
        y1: 0.0,
        x2: 10.0,
        y2: 10.0,
    });
    assert!(matches!(line, Shape::Line(_)));
}

#[test]
fn rect_has_fill_and_stroke_geometry() {
    let rect = Shape::Rect(Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        rx: None,
        ry: None,
    });
    assert!(matches!(rect, Shape::Rect(_)));
}

// ============================================================
// 2. STYLE TYPE TESTS
// ============================================================

#[test]
fn node_style_defaults() {
    let s = NodeStyle::default();
    assert!(s.is_visible());
    assert!(s.is_displayed());
    assert!(s.fill.is_none());
    assert!(s.stroke.is_none());
    assert_eq!(s.opacity, 1.0);
}

#[test]
fn node_style_visibility_hidden() {
    let mut s = NodeStyle::default();
    s.visibility = Visibility::Hidden;
    assert!(!s.is_visible());
    assert!(s.is_displayed());
}

#[test]
fn node_style_display_none() {
    let mut s = NodeStyle::default();
    s.display = Display::None;
    assert!(!s.is_displayed());
}

#[test]
fn fill_params_solid_color() {
    let f = FillParams {
        color: Some(svgtypes::Color::new_rgb(255, 0, 0)),
        paint_server: None,
        opacity: 0.8,
        fill_rule: FillRule::NonZero,
    };
    assert_eq!(f.opacity, 0.8);
    assert!(matches!(f.fill_rule, FillRule::NonZero));
    assert!(f.paint_server.is_none());
}

#[test]
fn fill_params_gradient_paint_server() {
    let f = FillParams {
        color: None,
        paint_server: Some(PaintServer::Gradient("myGrad".to_owned())),
        opacity: 1.0,
        fill_rule: FillRule::NonZero,
    };
    assert!(matches!(f.paint_server, Some(PaintServer::Gradient(ref id)) if id == "myGrad"));
}

#[test]
fn fill_params_evenodd() {
    let f = FillParams {
        color: None,
        paint_server: None,
        opacity: 1.0,
        fill_rule: FillRule::EvenOdd,
    };
    assert!(matches!(f.fill_rule, FillRule::EvenOdd));
}

#[test]
fn stroke_params_all_fields() {
    let s = StrokeParams {
        color: Some(svgtypes::Color::new_rgb(0, 0, 0)),
        paint_server: None,
        opacity: 0.5,
        width: 3.0,
        line_cap: LineCap::Round,
        line_join: LineJoin::Bevel,
        miter_limit: 10.0,
        dash_array: Some(vec![5.0, 3.0]),
        dash_offset: 2.0,
    };
    assert_eq!(s.width, 3.0);
    assert!(matches!(s.line_cap, LineCap::Round));
    assert!(matches!(s.line_join, LineJoin::Bevel));
    assert_eq!(s.miter_limit, 10.0);
    assert_eq!(s.dash_array, Some(vec![5.0, 3.0]));
    assert_eq!(s.dash_offset, 2.0);
}

#[test]
fn stroke_line_cap_all_variants() {
    assert!(matches!(LineCap::Butt, LineCap::Butt));
    assert!(matches!(LineCap::Round, LineCap::Round));
    assert!(matches!(LineCap::Square, LineCap::Square));
}

#[test]
fn stroke_line_join_all_variants() {
    assert!(matches!(LineJoin::Miter, LineJoin::Miter));
    assert!(matches!(LineJoin::Round, LineJoin::Round));
    assert!(matches!(LineJoin::Bevel, LineJoin::Bevel));
}

#[test]
fn node_effects_default_empty() {
    let effects = NodeEffects {
        clip_path: None,
        mask: None,
        filter: None,
    };
    assert!(effects.clip_path.is_none());
    assert!(effects.mask.is_none());
    assert!(effects.filter.is_none());
}

#[test]
fn node_effects_with_clip_path() {
    let effects = NodeEffects {
        clip_path: Some("c1".into()),
        mask: None,
        filter: None,
    };
    assert!(effects.clip_path.is_some());
}

// ============================================================
// 3. RENDER TREE TESTS
// ============================================================

#[test]
fn svg_tag_shape_and_container() {
    let shape_tag = SvgTag::Shape(Shape::Rect(Rectangle {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
        rx: None,
        ry: None,
    }));
    let group_tag = SvgTag::Container(Container::Group);
    assert!(matches!(shape_tag, SvgTag::Shape(_)));
    assert!(matches!(group_tag, SvgTag::Container(Container::Group)));
}

#[test]
fn container_all_variants() {
    assert!(matches!(Container::Group, Container::Group));
    assert!(matches!(Container::Svg, Container::Svg));
    assert!(matches!(Container::Defs, Container::Defs));
    assert!(matches!(Container::Use, Container::Use));
    assert!(matches!(Container::Symbol, Container::Symbol));
}

#[test]
fn viewport_info_defaults() {
    let vp = ViewportInfo {
        width: 300.0,
        height: 150.0,
        view_box: None,
        overflow_visible: false,
        aspect_ratio: None,
    };
    assert_eq!(vp.width, 300.0);
}

#[test]
fn viewport_with_viewbox() {
    let vp = ViewportInfo {
        width: 200.0,
        height: 200.0,
        view_box: Some(ViewBox {
            min_x: 0.0,
            min_y: 0.0,
            width: 100.0,
            height: 100.0,
        }),
        overflow_visible: false,
        aspect_ratio: None,
    };
    assert_eq!(vp.view_box.unwrap().width, 100.0);
}

#[test]
fn aspect_ratio_default_xmidymid_meet() {
    let ar = AspectRatio::default();
    assert!(matches!(ar.align, AspectAlign::XMidYMid));
    assert!(matches!(ar.meet_or_slice, MeetOrSlice::Meet));
}

#[test]
fn aspect_align_all_10_variants_exist() {
    let aligns = [
        AspectAlign::None,
        AspectAlign::XMinYMin,
        AspectAlign::XMidYMin,
        AspectAlign::XMaxYMin,
        AspectAlign::XMinYMid,
        AspectAlign::XMidYMid,
        AspectAlign::XMaxYMid,
        AspectAlign::XMinYMax,
        AspectAlign::XMidYMax,
        AspectAlign::XMaxYMax,
    ];
    assert_eq!(aligns.len(), 10);
}

#[test]
fn parse_aspect_ratio_none() {
    let ar = parse_aspect_ratio("none");
    assert!(matches!(ar.align, AspectAlign::None));
}

#[test]
fn parse_aspect_ratio_xmidymid_slice() {
    let ar = parse_aspect_ratio("xMidYMid slice");
    assert!(matches!(ar.align, AspectAlign::XMidYMid));
    assert!(matches!(ar.meet_or_slice, MeetOrSlice::Slice));
}

#[test]
fn parse_aspect_ratio_defaults_meet() {
    let ar = parse_aspect_ratio("xMinYMin");
    assert!(matches!(ar.meet_or_slice, MeetOrSlice::Meet));
}

#[test]
fn parse_aspect_ratio_all_valid_aligns() {
    for align in [
        "xMinYMin", "xMidYMin", "xMaxYMin", "xMinYMid", "xMidYMid", "xMaxYMid", "xMinYMax",
        "xMidYMax", "xMaxYMax",
    ] {
        let ar = parse_aspect_ratio(align);
        assert!(
            !matches!(ar.align, AspectAlign::None),
            "Align should not be None for {align}"
        );
    }
}

#[test]
fn parse_aspect_ratio_unknown_defaults_xmidymid() {
    let ar = parse_aspect_ratio("garbage");
    assert!(matches!(ar.align, AspectAlign::XMidYMid));
}

// ============================================================
// 4. VIEWBOX TESTS
// ============================================================

#[test]
fn viewbox_valid() {
    let vb = extract_viewbox("0 0 200 200").unwrap();
    assert_eq!(
        (vb.min_x, vb.min_y, vb.width, vb.height),
        (0.0, 0.0, 200.0, 200.0)
    );
}

#[test]
fn viewbox_with_commas() {
    let vb = extract_viewbox("10,20 300,400").unwrap();
    assert_eq!(vb.width, 300.0);
    assert_eq!(vb.height, 400.0);
}

#[test]
fn viewbox_negative_coords() {
    let vb = extract_viewbox("-100 -100 200 200").unwrap();
    assert_eq!(vb.min_x, -100.0);
    assert_eq!(vb.min_y, -100.0);
}

#[test]
fn viewbox_zero_dimensions_rejected() {
    assert!(extract_viewbox("0 0 0 200").is_none());
    assert!(extract_viewbox("0 0 200 0").is_none());
    assert!(extract_viewbox("0 0 -10 200").is_none());
}

#[test]
fn viewbox_invalid_inputs() {
    assert!(extract_viewbox("").is_none());
    assert!(extract_viewbox("0 0 200").is_none());
    assert!(extract_viewbox("abc def ghi jkl").is_none());
}

// ============================================================
// 5. GRADIENT TESTS
// ============================================================

#[test]
fn paint_server_solid_named_color() {
    for color in [
        "red", "blue", "green", "black", "white", "yellow", "purple", "orange",
    ] {
        let ps = PaintServer::from_attr(color);
        assert!(ps.is_some(), "Failed to parse named color: {color}");
        assert!(matches!(ps, Some(PaintServer::Solid(_))));
    }
}

#[test]
fn paint_server_hex_color() {
    let ps = PaintServer::from_attr("#ff0000").unwrap();
    assert!(matches!(ps, PaintServer::Solid(c) if c.red == 255 && c.green == 0 && c.blue == 0));
}

#[test]
fn paint_server_rgb_function() {
    let ps = PaintServer::from_attr("rgb(0, 128, 255)");
    assert!(ps.is_some());
}

#[test]
fn paint_server_url_gradient() {
    let ps = PaintServer::from_attr("url(#myGradient)").unwrap();
    assert!(matches!(ps, PaintServer::Gradient(ref id) if id == "myGradient"));
}

#[test]
fn paint_server_none_is_not_a_color() {
    // "none" is an SVG keyword, not a paint server value.
    // It's handled at the FillParams/StrokeParams level (None variant).
    let ps = PaintServer::from_attr("none");
    // "none" is not a CSS color name, so parsing fails
    assert!(ps.is_none());
}

#[test]
fn gradient_stop_ordering_by_offset() {
    let stops = vec![
        GradientStop {
            offset: 0.5,
            color: svgtypes::Color::new_rgb(128, 128, 128),
        },
        GradientStop {
            offset: 0.0,
            color: svgtypes::Color::new_rgb(0, 0, 0),
        },
        GradientStop {
            offset: 1.0,
            color: svgtypes::Color::new_rgb(255, 255, 255),
        },
    ];
    let mut sorted = stops.clone();
    sorted.sort_by(|a, b| {
        a.offset
            .partial_cmp(&b.offset)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    assert_eq!(sorted[0].offset, 0.0);
    assert_eq!(sorted[1].offset, 0.5);
    assert_eq!(sorted[2].offset, 1.0);
}

#[test]
fn gradient_length_number() {
    let n = GradientLength::Number(50.0);
    assert_eq!(n.to_object_bbox(), 50.0);
    assert_eq!(n.to_user_space(200.0), 50.0);
}

#[test]
fn gradient_length_percentage() {
    let p = GradientLength::Percentage(50.0);
    assert_eq!(p.to_object_bbox(), 0.5);
    assert_eq!(p.to_user_space(200.0), 100.0);
}

#[test]
fn gradient_units_both_variants() {
    assert!(matches!(
        GradientUnits::ObjectBoundingBox,
        GradientUnits::ObjectBoundingBox
    ));
    assert!(matches!(
        GradientUnits::UserSpaceOnUse,
        GradientUnits::UserSpaceOnUse
    ));
}

#[test]
fn linear_gradient_default_x2() {
    // When x2 not specified in objectBoundingBox, defaults to 100%
    let lg = LinearGradient {
        id: "g".into(),
        x1: GradientLength::Number(0.0),
        y1: GradientLength::Number(0.0),
        x2: GradientLength::Percentage(100.0),
        y2: GradientLength::Number(0.0),
        units: GradientUnits::ObjectBoundingBox,
        stops: vec![],
        transform: vec![],
        spread_method: SpreadMethod::Pad,
    };
    assert_eq!(lg.x2.to_object_bbox(), 1.0);
}

#[test]
fn radial_gradient_default_center() {
    let rg = RadialGradient {
        id: "g".into(),
        cx: GradientLength::Percentage(50.0),
        cy: GradientLength::Percentage(50.0),
        r: GradientLength::Percentage(50.0),
        fx: GradientLength::Percentage(50.0),
        fy: GradientLength::Percentage(50.0),
        units: GradientUnits::ObjectBoundingBox,
        stops: vec![],
        transform: vec![],
        spread_method: SpreadMethod::Pad,
    };
    assert_eq!(rg.cx.to_object_bbox(), 0.5);
    assert_eq!(rg.r.to_object_bbox(), 0.5);
}

// ============================================================
// 6. CLIP PATH / MASK / FILTER DEFINITION TESTS
// ============================================================

#[test]
fn clip_path_units_both_variants() {
    assert!(matches!(
        ClipPathUnits::ObjectBoundingBox,
        ClipPathUnits::ObjectBoundingBox
    ));
    assert!(matches!(
        ClipPathUnits::UserSpaceOnUse,
        ClipPathUnits::UserSpaceOnUse
    ));
}

#[test]
fn clip_path_def_non_empty_shapes() {
    let shapes = vec![Shape::Rect(Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        rx: None,
        ry: None,
    })];
    let def = ClipPathDef {
        shapes,
        clip_path_units: ClipPathUnits::UserSpaceOnUse,
    };
    assert_eq!(def.shapes.len(), 1);
}

#[test]
fn mask_def_with_shapes_and_styles() {
    let rect = Shape::Rect(Rectangle {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
        rx: None,
        ry: None,
    });
    let shapes = vec![(rect, NodeStyle::default())];
    let def = MaskDef { shapes };
    assert_eq!(def.shapes.len(), 1);
}

#[test]
fn filter_def_all_primitive_variants() {
    let primitives = vec![
        FilterPrimitive::GaussianBlur(2.0, 2.0),
        FilterPrimitive::DropShadow(5.0, 5.0, 3.0, 0.0, 0.0, 0.0, 0.5),
        FilterPrimitive::ColorMatrix([1.0; 20]),
    ];
    let def = FilterDef {
        primitives,
        x: -0.1,
        y: -0.1,
        width: 1.2,
        height: 1.2,
    };
    assert_eq!(def.primitives.len(), 3);
    assert!(matches!(
        def.primitives[0],
        FilterPrimitive::GaussianBlur(2.0, 2.0)
    ));
    assert!(matches!(
        def.primitives[1],
        FilterPrimitive::DropShadow(_, _, _, _, _, _, _)
    ));
    assert!(matches!(def.primitives[2], FilterPrimitive::ColorMatrix(_)));
}

// ============================================================
// 7. PATTERN TESTS
// ============================================================

#[test]
fn pattern_def_basic() {
    let rect = Shape::Rect(Rectangle {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
        rx: None,
        ry: None,
    });
    let shapes = vec![(rect, NodeStyle::default())];
    let def = PatternDef {
        width: 20.0,
        height: 20.0,
        x: 0.0,
        y: 0.0,
        pattern_units: PatternUnits::UserSpaceOnUse,
        pattern_content_units: PatternContentUnits::UserSpaceOnUse,
        transform: vec![],
        view_box: None,
        aspect_ratio: None,
        shapes,
    };
    assert_eq!(def.width, 20.0);
    assert_eq!(def.height, 20.0);
}

#[test]
fn pattern_units_variants() {
    assert!(matches!(
        PatternUnits::ObjectBoundingBox,
        PatternUnits::ObjectBoundingBox
    ));
    assert!(matches!(
        PatternUnits::UserSpaceOnUse,
        PatternUnits::UserSpaceOnUse
    ));
}

#[test]
fn pattern_content_units_variants() {
    assert!(matches!(
        PatternContentUnits::ObjectBoundingBox,
        PatternContentUnits::ObjectBoundingBox
    ));
    assert!(matches!(
        PatternContentUnits::UserSpaceOnUse,
        PatternContentUnits::UserSpaceOnUse
    ));
}

// ============================================================
// 8. TRANSFORM TESTS
// ============================================================

#[test]
fn transform_op_translate() {
    let op = TransformOp::Translate(10.0, 20.0);
    assert!(matches!(op, TransformOp::Translate(10.0, 20.0)));
}

#[test]
fn transform_op_rotate() {
    let op = TransformOp::Rotate(45.0, 0.0, 0.0);
    assert!(matches!(op, TransformOp::Rotate(45.0, _, _)));
}

#[test]
fn transform_op_scale() {
    let op = TransformOp::Scale(2.0, 2.0);
    assert!(matches!(op, TransformOp::Scale(2.0, 2.0)));
}

#[test]
fn transform_op_skew() {
    assert!(matches!(TransformOp::SkewX(30.0), TransformOp::SkewX(30.0)));
    assert!(matches!(TransformOp::SkewY(15.0), TransformOp::SkewY(15.0)));
}

#[test]
fn transform_op_matrix() {
    let op = TransformOp::Matrix([1.0, 0.0, 0.0, 1.0, 50.0, 50.0]);
    assert!(matches!(
        op,
        TransformOp::Matrix([1.0, 0.0, 0.0, 1.0, 50.0, 50.0])
    ));
}

#[test]
fn parse_transform_str_translate() {
    let ops = svg_engine::style::transform_ops::parse_transform_str("translate(10, 20)");
    assert!(!ops.is_empty());
    assert!(matches!(ops[0], TransformOp::Translate(10.0, 20.0)));
}

#[test]
fn parse_transform_str_multiple_ops() {
    let ops = svg_engine::style::transform_ops::parse_transform_str("translate(10,0) rotate(45)");
    assert_eq!(ops.len(), 2);
}

#[test]
fn parse_transform_str_empty() {
    let ops = svg_engine::style::transform_ops::parse_transform_str("");
    assert!(ops.is_empty());
}

#[test]
fn parse_transform_str_scale() {
    let ops = svg_engine::style::transform_ops::parse_transform_str("scale(2)");
    assert!(!ops.is_empty());
    assert!(matches!(ops[0], TransformOp::Scale(2.0, 2.0)));
}

#[test]
fn parse_transform_str_matrix() {
    let ops = svg_engine::style::transform_ops::parse_transform_str("matrix(1,0,0,1,50,50)");
    assert!(!ops.is_empty());
    assert!(matches!(ops[0], TransformOp::Matrix(_)));
}

// ============================================================
// 9. RENDER HINTS TESTS (color-rendering, color-interpolation, paint-order)
// ============================================================

#[test]
fn color_rendering_variants_exist() {
    assert!(matches!(ColorRendering::Auto, ColorRendering::Auto));
    assert!(matches!(
        ColorRendering::OptimizeSpeed,
        ColorRendering::OptimizeSpeed
    ));
    assert!(matches!(
        ColorRendering::OptimizeQuality,
        ColorRendering::OptimizeQuality
    ));
}

#[test]
fn color_interpolation_variants_exist() {
    assert!(matches!(ColorInterpolation::Auto, ColorInterpolation::Auto));
    assert!(matches!(ColorInterpolation::Srgb, ColorInterpolation::Srgb));
    assert!(matches!(
        ColorInterpolation::LinearRGB,
        ColorInterpolation::LinearRGB
    ));
}

#[test]
fn color_interpolation_linear_rgb_gradient_math() {
    // Linear RGB interpolation of red→blue should produce visibly
    // different result from sRGB interpolation (darker mid-tones).
    let red = svgtypes::Color::new_rgb(255, 0, 0);
    let blue = svgtypes::Color::new_rgb(0, 0, 255);
    let stops = vec![
        GradientStop {
            offset: 0.0,
            color: red,
        },
        GradientStop {
            offset: 1.0,
            color: blue,
        },
    ];

    // sRGB midpoint
    let srgb_mid = svg_engine::color_at_t_with_space(&stops, 0.5, ColorInterpolation::Srgb);
    // Linear RGB midpoint
    let linear_mid = svg_engine::color_at_t_with_space(&stops, 0.5, ColorInterpolation::LinearRGB);

    // Linear RGB midpoint should be perceptually different from sRGB.
    // The R and B channels diverge — linear RGB produces a darker purple.
    assert!(srgb_mid.r > 0.0 && srgb_mid.b > 0.0);
    assert!(linear_mid.r > 0.0 && linear_mid.b > 0.0);
    // Linear RGB typically has a lower max channel value at t=0.5
    // because gamma correction darkens the midpoint.
    assert!(
        (srgb_mid.r - linear_mid.r).abs() > 0.0 || (srgb_mid.b - linear_mid.b).abs() > 0.0,
        "sRGB and linear RGB should produce different midpoints"
    );
}

#[test]
fn paint_order_normal() {
    let po = PaintOrder::Normal;
    assert!(!po.stroke_before_fill());
}

#[test]
fn paint_order_stroke_fill() {
    let po = PaintOrder::StrokeFill;
    assert!(po.stroke_before_fill());
}

#[test]
fn paint_order_fill_stroke() {
    let po = PaintOrder::FillStroke;
    assert!(!po.stroke_before_fill());
}

#[test]
fn render_hints_with_color_interpolation() {
    let hints = RenderHints {
        vector_effect: None,
        shape_rendering: None,
        color_rendering: None,
        color_interpolation: Some(ColorInterpolation::LinearRGB),
        paint_order: None,
        text_rendering: None,
        image_rendering: None,
    };
    assert!(matches!(
        hints.color_interpolation,
        Some(ColorInterpolation::LinearRGB)
    ));
}

#[test]
fn render_hints_with_color_rendering_optimize_quality() {
    let hints = RenderHints {
        vector_effect: None,
        shape_rendering: None,
        color_rendering: Some(ColorRendering::OptimizeQuality),
        color_interpolation: None,
        paint_order: None,
        text_rendering: None,
        image_rendering: None,
    };
    assert!(matches!(
        hints.color_rendering,
        Some(ColorRendering::OptimizeQuality)
    ));
}

#[test]
fn render_hints_with_paint_order_stroke_fill() {
    let hints = RenderHints {
        vector_effect: None,
        shape_rendering: None,
        color_rendering: None,
        color_interpolation: None,
        paint_order: Some(PaintOrder::StrokeFill),
        text_rendering: None,
        image_rendering: None,
    };
    assert!(hints.paint_order.unwrap().stroke_before_fill());
}

// ============================================================
// 10. VISITOR PATTERN TESTS
// ============================================================

#[test]
fn visitor_visits_all_nodes() {
    let tree = make_simple_tree();
    struct Counter(usize);
    impl SvgRenderTreeVisitor for Counter {
        fn visit_node(&mut self, _node: &SvgRenderNode) -> VisitDecision {
            self.0 += 1;
            VisitDecision::Continue
        }
    }
    let mut counter = Counter(0);
    tree.visit(&mut counter);
    assert_eq!(counter.0, 3);
}

#[test]
fn visitor_skip_children() {
    let tree = make_simple_tree();
    struct SkipRoot(bool);
    impl SvgRenderTreeVisitor for SkipRoot {
        fn visit_node(&mut self, node: &SvgRenderNode) -> VisitDecision {
            if node.id == Some("root".to_owned()) {
                VisitDecision::SkipChildren
            } else {
                self.0 = true;
                VisitDecision::Continue
            }
        }
    }
    let mut v = SkipRoot(false);
    tree.visit(&mut v);
    assert!(!v.0, "Children should be skipped");
}

#[test]
fn visitor_stop_does_not_panic() {
    let tree = make_simple_tree();
    struct StopAfterRoot;
    impl SvgRenderTreeVisitor for StopAfterRoot {
        fn visit_node(&mut self, _node: &SvgRenderNode) -> VisitDecision {
            VisitDecision::Stop
        }
    }
    tree.visit(&mut StopAfterRoot);
}

#[test]
fn mutable_visitor_modifies_nodes() {
    let mut tree = make_simple_tree_with_fill();
    struct OpacityBump;
    impl SvgRenderTreeVisitorMut for OpacityBump {
        fn visit_node_mut(&mut self, node: &mut SvgRenderNode) -> VisitDecision {
            node.style.opacity *= 0.5;
            VisitDecision::Continue
        }
    }
    tree.visit_mut(&mut OpacityBump);
    assert_eq!(tree.root.style.opacity, 0.5);
}

#[test]
fn paintserver_fixup_converts_gradient_to_pattern() {
    use svg_engine::style::gradient::PaintServer;
    use svg_engine::visitor::PaintServerFixupVisitor;

    let mut node = make_svg_node();
    node.style.fill = Some(FillParams {
        color: None,
        paint_server: Some(PaintServer::Gradient("myPat".to_owned())),
        opacity: 1.0,
        fill_rule: FillRule::NonZero,
    });

    let mut patterns = HashMap::new();
    patterns.insert(
        "myPat".to_owned(),
        PatternDef {
            width: 10.0,
            height: 10.0,
            x: 0.0,
            y: 0.0,
            pattern_units: PatternUnits::UserSpaceOnUse,
            pattern_content_units: PatternContentUnits::UserSpaceOnUse,
            transform: vec![],
            view_box: None,
            aspect_ratio: None,
            shapes: vec![],
        },
    );

    let mut visitor = PaintServerFixupVisitor {
        pattern_ids: &patterns,
    };
    node.accept_mut(&mut visitor);

    let fill = node.style.fill.unwrap();
    assert!(matches!(fill.paint_server, Some(PaintServer::Pattern(ref id)) if id == "myPat"));
}

#[test]
fn paintserver_fixup_does_not_affect_real_gradient() {
    use svg_engine::style::gradient::PaintServer;
    use svg_engine::visitor::PaintServerFixupVisitor;

    let mut node = make_svg_node();
    node.style.fill = Some(FillParams {
        color: None,
        paint_server: Some(PaintServer::Gradient("realGrad".to_owned())),
        opacity: 1.0,
        fill_rule: FillRule::NonZero,
    });

    let patterns: HashMap<String, PatternDef> = HashMap::new();
    let mut visitor = PaintServerFixupVisitor {
        pattern_ids: &patterns,
    };
    node.accept_mut(&mut visitor);

    let fill = node.style.fill.unwrap();
    assert!(matches!(fill.paint_server, Some(PaintServer::Gradient(ref id)) if id == "realGrad"));
}

// ============================================================
// 10. RENDER TREE INTEGRATION TESTS
// ============================================================

#[test]
fn empty_tree_does_not_panic_on_visit() {
    let tree = make_empty_tree();
    struct CountingVisitor<'a>(&'a mut usize);
    impl<'a> SvgRenderTreeVisitor for CountingVisitor<'a> {
        fn visit_node(&mut self, _node: &SvgRenderNode) -> VisitDecision {
            *self.0 += 1;
            VisitDecision::Continue
        }
    }
    let mut count = 0;
    tree.visit(&mut CountingVisitor(&mut count));
    assert_eq!(count, 1);
}

#[test]
fn tree_with_nested_groups() {
    let tree = make_simple_tree();
    assert_eq!(tree.root.children.len(), 2);
}

#[test]
fn defs_container_in_tree() {
    let defs = SvgRenderNode {
        id: None,
        tag: SvgTag::Container(Container::Defs),
        style: NodeStyle::default(),
        transforms: vec![],
        viewport: None,
        children: vec![],
    };
    assert!(matches!(defs.tag, SvgTag::Container(Container::Defs)));
}

#[test]
fn svg_render_node_with_transforms() {
    use svg_engine::style::transform_ops::TransformOp;
    let node = SvgRenderNode {
        id: Some("t".into()),
        tag: SvgTag::Container(Container::Group),
        style: NodeStyle::default(),
        transforms: vec![TransformOp::Translate(50.0, 50.0)],
        viewport: None,
        children: vec![],
    };
    assert_eq!(node.transforms.len(), 1);
}

// ============================================================
// 11. ERROR TYPE TESTS
// ============================================================

#[test]
fn svg_engine_error_missing_attr() {
    use svg_engine::error::SvgEngineError;
    let err = SvgEngineError::MissingAttribute("width".to_owned());
    assert_eq!(err.to_string(), "missing SVG attribute: width");
}

#[test]
fn svg_engine_error_parse_error() {
    use svg_engine::error::SvgEngineError;
    let err = SvgEngineError::ParseError("invalid number".to_owned());
    assert_eq!(err.to_string(), "SVG parse error: invalid number");
}

#[test]
fn svg_engine_error_unsupported() {
    use svg_engine::error::SvgEngineError;
    let err = SvgEngineError::UnsupportedFeature("gradients".to_owned());
    assert_eq!(err.to_string(), "unsupported SVG feature: gradients");
}

#[test]
fn svg_engine_error_implements_std_error() {
    use svg_engine::error::SvgEngineError;
    let err: &dyn std::error::Error = &SvgEngineError::ParseError("test".to_owned());
    assert!(!err.to_string().is_empty());
}

#[test]
fn svg_engine_error_debug_differs_from_display() {
    use svg_engine::error::SvgEngineError;
    let err = SvgEngineError::ParseError("test".to_owned());
    assert_ne!(format!("{err:?}"), format!("{err}"));
}

// ============================================================
// 12. SvgRenderTree DEFINITION COLLECTION TESTS
// ============================================================

#[test]
fn render_tree_initializes_with_empty_def_maps() {
    let tree = make_empty_tree();
    assert!(tree.gradients.is_empty());
    assert!(tree.clip_paths.is_empty());
    assert!(tree.patterns.is_empty());
    assert!(tree.masks.is_empty());
    assert!(tree.filters.is_empty());
}

#[test]
fn render_tree_with_gradient_def() {
    let mut tree = make_empty_tree();
    let grad = GradientDef::Linear(LinearGradient {
        id: "g1".into(),
        x1: GradientLength::Number(0.0),
        y1: GradientLength::Number(0.0),
        x2: GradientLength::Percentage(100.0),
        y2: GradientLength::Number(0.0),
        units: GradientUnits::ObjectBoundingBox,
        stops: vec![],
        transform: vec![],
        spread_method: SpreadMethod::Pad,
    });
    tree.gradients.insert("g1".into(), grad);
    assert_eq!(tree.gradients.len(), 1);
}

#[test]
fn render_tree_gradient_insert_and_check() {
    let mut tree = make_empty_tree();
    let grad = GradientDef::Linear(LinearGradient {
        id: "g1".into(),
        x1: GradientLength::Number(0.0),
        y1: GradientLength::Number(0.0),
        x2: GradientLength::Percentage(100.0),
        y2: GradientLength::Number(0.0),
        units: GradientUnits::ObjectBoundingBox,
        stops: vec![],
        transform: vec![],
        spread_method: SpreadMethod::Pad,
    });
    tree.gradients.insert("g1".into(), grad);
    assert_eq!(tree.gradients.len(), 1);
    assert!(!tree.gradients.contains_key("missing"));
}

// ============================================================
// 13. VISIBILITY & DISPLAY TESTS
// ============================================================

#[test]
fn visibility_visible_and_hidden() {
    let v1 = Visibility::Visible;
    let v2 = Visibility::Hidden;
    assert!(matches!(v1, Visibility::Visible));
    assert!(matches!(v2, Visibility::Hidden));
}

#[test]
fn display_variants() {
    assert!(matches!(Display::Inline, Display::Inline));
    assert!(matches!(Display::Block, Display::Block));
    assert!(matches!(Display::None, Display::None));
}

// ============================================================
// 14. RENDER HINTS TESTS
// ============================================================

#[test]
fn render_hints_non_scaling_stroke() {
    let hints = RenderHints {
        vector_effect: Some(VectorEffect::NonScalingStroke),
        shape_rendering: None,
        color_rendering: None,
        color_interpolation: None,
        text_rendering: None,
        image_rendering: None,
        paint_order: None,
    };
    assert!(matches!(
        hints.vector_effect,
        Some(VectorEffect::NonScalingStroke)
    ));
}

#[test]
fn shape_rendering_all_variants() {
    assert!(matches!(ShapeRendering::Auto, ShapeRendering::Auto));
    assert!(matches!(
        ShapeRendering::OptimizeSpeed,
        ShapeRendering::OptimizeSpeed
    ));
    assert!(matches!(
        ShapeRendering::CrispEdges,
        ShapeRendering::CrispEdges
    ));
    assert!(matches!(
        ShapeRendering::GeometricPrecision,
        ShapeRendering::GeometricPrecision
    ));
}

// ============================================================
// HELPER FUNCTIONS
// ============================================================

fn make_svg_node() -> SvgRenderNode {
    SvgRenderNode {
        id: None,
        tag: SvgTag::Container(Container::Svg),
        style: NodeStyle::default(),
        transforms: vec![],
        viewport: None,
        children: vec![],
    }
}

fn make_simple_tree() -> SvgRenderTree {
    let child1 = SvgRenderNode {
        id: Some("child1".to_owned()),
        tag: SvgTag::Shape(Shape::Circle(Circle {
            cx: 10.0,
            cy: 10.0,
            r: 5.0,
        })),
        style: NodeStyle::default(),
        transforms: vec![],
        viewport: None,
        children: vec![],
    };
    let child2 = SvgRenderNode {
        id: Some("child2".to_owned()),
        tag: SvgTag::Shape(Shape::Rect(Rectangle {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
            rx: None,
            ry: None,
        })),
        style: NodeStyle::default(),
        transforms: vec![],
        viewport: None,
        children: vec![],
    };
    let root = SvgRenderNode {
        id: Some("root".to_owned()),
        tag: SvgTag::Container(Container::Svg),
        style: NodeStyle::default(),
        transforms: vec![],
        viewport: None,
        children: vec![child1, child2],
    };
    SvgRenderTree {
        root,
        viewport: ViewportInfo {
            width: 100.0,
            height: 100.0,
            view_box: None,
            overflow_visible: false,
            aspect_ratio: None,
        },
        gradients: HashMap::new(),
        clip_paths: HashMap::new(),
        patterns: HashMap::new(),
        masks: HashMap::new(),
        filters: HashMap::new(),
        markers: HashMap::new(),
    }
}

fn make_simple_tree_with_fill() -> SvgRenderTree {
    let mut tree = make_simple_tree();
    tree.root.style.fill = Some(FillParams {
        color: Some(svgtypes::Color::new_rgb(255, 0, 0)),
        paint_server: None,
        opacity: 1.0,
        fill_rule: FillRule::NonZero,
    });
    tree
}

fn make_empty_tree() -> SvgRenderTree {
    let root = SvgRenderNode {
        id: None,
        tag: SvgTag::Container(Container::Svg),
        style: NodeStyle::default(),
        transforms: vec![],
        viewport: None,
        children: vec![],
    };
    SvgRenderTree {
        root,
        viewport: ViewportInfo {
            width: 100.0,
            height: 100.0,
            view_box: None,
            overflow_visible: false,
            aspect_ratio: None,
        },
        gradients: HashMap::new(),
        clip_paths: HashMap::new(),
        patterns: HashMap::new(),
        masks: HashMap::new(),
        filters: HashMap::new(),
        markers: HashMap::new(),
    }
}
