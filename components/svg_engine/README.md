# svg_engine — pure SVG data model

This crate holds the **data model** for SVG rendering in Servo: element types,
style properties, the document tree, and definition types.

It is intentionally dependency-light — the only external crate is
[`svgtypes`](https://crates.io/crates/svgtypes) (used for `Color`). It has no
dependency on WebRender, vello, or any rendering backend, so the model can be
shared by any renderer.

The rendering half of the engine (which consumes this model) is added in a
later change; this crate is deliberately kept to the model only.

## Modules

- `model::element` — `SvgNode`, `SvgTag`, shapes, text, image.
- `model::style` — fill/stroke paint, gradients, transforms, effects, hints.
- `model::document` — `SvgTree`, viewport, and `<defs>` definition types.
- `model::geometry` — `Point`, `PathData`.
- `model::resource` — opaque resource keys.
- `model::units` — `Length`, `Opacity`, `Id` newtypes.
- `model::error` — error types.
