/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Font resolution: the Servo → usvg font bridge used to lay out `<text>` into
//! glyph outlines.

use std::cell::RefCell;
use std::sync::Arc;

use net_traits::image_cache::FontResolver;
use resvg::usvg::{self, fontdb};
use script::SvgFontResolver;
use style::properties::ComputedValues;
use style::values::computed::FontStyle as ServoFontStyle;
use style::values::computed::font::SingleFontFamily;
use style::values::specified::font::FontStretchKeyword;

use crate::context::LayoutContext;

/// Font database and resolver used to lay out `<text>` into glyph outlines.
///
/// usvg lays text out at build time into `Text.flattened` (a group of glyph
/// outlines), reusing its own shaping/positioning engine rather than
/// reimplementing it. The resolver pulls fonts from the script thread's
/// `FontContext` on demand, exactly like the existing image-cache SVG path.
pub(crate) struct SvgFonts {
    pub(crate) resolver: usvg::FontResolver<'static>,
    pub(crate) cache: RefCell<usvg::Cache>,
}

impl SvgFonts {
    pub(crate) fn new(context: &LayoutContext) -> Self {
        let servo_resolver: Arc<dyn FontResolver> =
            Arc::new(SvgFontResolver::new(context.font_context.clone()));
        let resolver = {
            let select_font = servo_resolver.clone();
            let select_fallback = servo_resolver.clone();
            usvg::FontResolver {
                select_font: Box::new(move |font, database| select_font.resolve(font, database)),
                select_fallback: Box::new(move |ch, ids, database| {
                    select_fallback.resolve_fallback(ch, ids, database)
                }),
            }
        };
        SvgFonts {
            resolver,
            cache: RefCell::new(usvg::Cache::new(Arc::new(fontdb::Database::new()))),
        }
    }
}

/// Maps a Servo `SingleFontFamily` onto usvg's `svgtypes::FontFamily`.
fn font_family_to_usvg(family: &SingleFontFamily) -> usvg::FontFamily {
    match family {
        SingleFontFamily::FamilyName(name) => usvg::FontFamily::Named(name.name.to_string()),
        SingleFontFamily::Generic(generic) => match generic {
            style::values::computed::font::GenericFontFamily::Serif => usvg::FontFamily::Serif,
            style::values::computed::font::GenericFontFamily::SansSerif => {
                usvg::FontFamily::SansSerif
            },
            style::values::computed::font::GenericFontFamily::Cursive => usvg::FontFamily::Cursive,
            style::values::computed::font::GenericFontFamily::Fantasy => usvg::FontFamily::Fantasy,
            style::values::computed::font::GenericFontFamily::Monospace => {
                usvg::FontFamily::Monospace
            },
            _ => usvg::FontFamily::SansSerif,
        },
    }
}

/// Builds a [`usvg::Font`] from a Servo computed `font-*` style.
pub(crate) fn convert_font(computed: &ComputedValues) -> usvg::Font {
    let font = computed.get_font();

    let style = if font.font_style == ServoFontStyle::NORMAL {
        usvg::FontStyle::Normal
    } else if font.font_style == ServoFontStyle::ITALIC {
        usvg::FontStyle::Italic
    } else {
        usvg::FontStyle::Oblique
    };

    let stretch = match font.font_stretch.as_keyword() {
        Some(FontStretchKeyword::UltraCondensed) => usvg::FontStretch::UltraCondensed,
        Some(FontStretchKeyword::ExtraCondensed) => usvg::FontStretch::ExtraCondensed,
        Some(FontStretchKeyword::Condensed) => usvg::FontStretch::Condensed,
        Some(FontStretchKeyword::SemiCondensed) => usvg::FontStretch::SemiCondensed,
        Some(FontStretchKeyword::Normal) => usvg::FontStretch::Normal,
        Some(FontStretchKeyword::SemiExpanded) => usvg::FontStretch::SemiExpanded,
        Some(FontStretchKeyword::Expanded) => usvg::FontStretch::Expanded,
        Some(FontStretchKeyword::ExtraExpanded) => usvg::FontStretch::ExtraExpanded,
        Some(FontStretchKeyword::UltraExpanded) => usvg::FontStretch::UltraExpanded,
        None => usvg::FontStretch::Normal,
    };

    let weight = font.font_weight.value().round() as u16;

    let variations = font
        .clone_font_variation_settings()
        .0
        .iter()
        .map(|setting| usvg::FontVariation::new(setting.tag.0.to_be_bytes(), setting.value))
        .collect();

    let mut families: Vec<usvg::FontFamily> = font
        .font_family
        .families
        .iter()
        .map(font_family_to_usvg)
        .collect();
    if families.is_empty() {
        families.push(usvg::FontFamily::SansSerif);
    }

    usvg::Font {
        families,
        style,
        stretch,
        weight,
        variations,
    }
}
