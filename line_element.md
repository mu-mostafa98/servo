# SVG `<line>` Element Rendering

## Overview

The `<line>` element is an SVG basic shape that draws a straight line segment between two points. It's defined in the [SVG 2 Shapes specification](https://www.w3.org/TR/SVG2/shapes.html#LineElement).

```xml
<line x1="10" y1="10" x2="100" y2="50" stroke="black" stroke-width="2"/>
```

### Attributes

| Attribute | Type    | Description                  | Default |
|-----------|---------|------------------------------|---------|
| `x1`      | `<length>` | X-coordinate of start point | 0       |
| `y1`      | `<length>` | Y-coordinate of start point | 0       |
| `x2`      | `<length>` | X-coordinate of end point   | 0       |
| `y2`      | `<length>` | Y-coordinate of end point   | 0       |

Unlike `rect`, `circle`, and `ellipse`, a `<line>` has no fill area — it's rendered purely through its stroke properties.

## Implementation

### Data Model (`shapes.rs`)

```rust
pub struct Line {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}
```

### Extraction (`extract.rs`)

The `extract_line()` function parses x1, y1, x2, y2 using the existing `parse_length()` helper, which handles pixel values and unitless numbers.

### Rendering (`renderers/line.rs`)

#### Challenge: No polygon/quad primitive in WebRender 0.68

WebRender 0.68's `DisplayListBuilder` provides `push_rect` (filled rectangle), `push_border` (rectangular border), and `push_line` (axis-aligned line only). It has no `push_quad`, `push_triangle`, or `push_polygon` method. SVG `<line>` can be at any angle, so axis-aligned primitives don't suffice.

#### Solution: Reference frame with rotation transform

A line from (x1,y1) to (x2,y2) with stroke width `w` is geometrically a rotated rectangle. The rendering approach:

1. Compute midpoint `mx = (x1+x2)/2`, `my = (y1+y2)/2` and angle `θ = atan2(y2-y1, x2-x1)`
2. Push a WebRender reference frame at the midpoint with a Z-axis rotation of `θ`
3. Within the rotated frame, push a rect at `(-L/2, -w/2)` of size `(L, w)` where `L` is the line length
4. The rect in the rotated frame appears as the correctly positioned line in parent space

```
Parent space:                    Frame space (rotated by θ):
                                   (-L/2, -w/2) ──────── (L/2, -w/2)
   (x1,y1) ← ― ― ― → (x2,y2)           │                    │
        └──── at midpoint ────┘          │    line rect      │
              rotate by θ               │                    │
                                   (-L/2, w/2)  ──────── (L/2, w/2)
```

#### Why not use `push_line`?

WebRender's `push_line` only supports `LineOrientation::Horizontal` and `LineOrientation::Vertical`, making it unsuitable for arbitrary SVG line angles.

### Line Caps

Current implementation uses `stroke-linecap: butt` (the SVG default). The line segment is drawn precisely between (x1,y1) and (x2,y2) without extending past the endpoints.

| Linecap  | Visual                     | Status |
|----------|----------------------------|--------|
| `butt`   | Ends flush with endpoints  | ✅ v1  |
| `round`  | Semicircular caps          | ❌     |
| `square` | Extends half stroke-width  | ❌     |

Round and square linecaps require additional geometry at endpoints (circles or rectangles beyond the endpoints).

## Usage

Test with the SVG engine enabled:

```sh
./mach run --svg-engine svg_line_test.html
```

Styled via CSS or presentation attributes:
```xml
<line x1="10" y1="50" x2="190" y2="50" stroke="blue" stroke-width="4"/>
<line x1="10" y1="10" x2="190" y2="90" stroke="red" stroke-width="2" stroke-dasharray="5,5"/>
```
