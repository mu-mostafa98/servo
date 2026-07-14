# SVG Engine — Phase-based Implementation Plan

## Overview

The SVG engine is built incrementally across 22 phases. Each phase adds one self-contained feature and is a reviewable PR. The final state has all features wired.

## Phases

| # | Feature | DOM Elements | New Integration Code |
|---|---|---|---|
| 1 | Rect + Fill | SVGElement, SVGRectElement, SVGGeometryElement | Full skeleton, Rect builds, Fill renders |
| 2 | Stroke | — | Stroke wiring + render |
| 3 | Circle + Ellipse | SVGCircleElement, SVGEllipseElement | collects arms for circle/ellipse |
| 4 | Transforms | — | Transform operations wired |
| 5 | Groups + `<style>` | SVGGElement | Container dispatch + CSS class rules |
| 6 | Line | SVGLineElement | collects arm for line |
| 7 | Polyline fill | SVGPolylineElement | collects arm for polyline |
| 8 | Polyline stroke | — | Polyline stroke renders |
| 9 | Polygon | SVGPolygonElement | collects arm for polygon |
| 10 | Path fill | SVGPathElement | collects arm for path |
| 11 | Path stroke | — | Path stroke renders |
| 12 | viewBox | — | Viewport extraction + scaling |
| 13 | `<defs>` infrastructure | SVGDefsElement | DefinitionCollector + Container::Defs |
| 14 | ClipPath | SVGClipPathElement | Clip-path parsing + rendering |
| 15 | Mask | SVGMaskElement | Mask parsing + rendering |
| 16 | Linear gradients | SVGGradientElement, SVGStopElement | Linear gradient parser |
| 17 | Radial gradients | — | Radial gradient parser |
| 18 | Patterns | SVGPatternElement | Pattern parsing + rendering |
| 19 | Blur filter | SVGFilterElement | Filter parsing, blur primitive |
| 20 | Drop shadow | — | Dropshadow filter primitive |
| 21 | `<use>` element | SVGUseElement | Element resolution + cloning |
| 22 | `<symbol>` | SVGSymbolElement | Container dispatch |

## Per-phase checklist

1. Add DOM element class + WebIDL (if new element)
2. Register in `element/create.rs`
3. Add `attribute_affects_presentational_hints` entries
4. Add `synthesize_presentational_hints` entries
5. Wire in `collects.rs` (build_shape_core arms)
6. Wire in `style.rs` (if new style attributes)
7. Update `virtualmethods.rs` if needed
8. **`./mach build` passes**
9. Add test cases to `svg_test_cases.html`
10. User reviews and confirms before next phase
