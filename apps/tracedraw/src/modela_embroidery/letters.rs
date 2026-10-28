//! Lettering: text sewn in the built-in centre-line font or in a digitized
//! embroidery font from the font library.
//!
//! In the built-in font a lettering object is a path whose subpaths are the
//! letters' strokes, in the object's own space with the first baseline
//! starting at its origin; its transform places it, so it can be moved and
//! turned like any object. Its outline part is a satin with the lettering
//! settings, so the text stays editable: changing it lays the strokes out
//! again where they are.
//!
//! In a digitized font it is a group of the font's pieces (satin columns,
//! fills and runs, each with the stitches the font gives it), the lettering
//! settings kept in the group's data under [`LETTERING_KEY`]; changing them
//! lays the pieces out again inside the group.

use super::sew::{self, Slot, Slots};
use tracedraw_core::geometry::{Affine, BezPath};
use tracedraw_core::{Color, Command, Fill, Shape, ShapeId, ShapeKind, Stroke};
use tracedraw_emb_stitch::lettering::{layout, Lettering, FONTS};
use tracedraw_emb_stitch::{StitchSettings, StitchType};

/// The object data key holding a digitized lettering group's settings.
pub const LETTERING_KEY: &str = "modela.embroidery.lettering";

/// Whether `font` is the built-in font (or names no library font).
pub fn is_builtin(font: &str) -> bool {
    FONTS.iter().any(|f| f.0 == font)
}

/// The lettering settings of `shape`, when it is a lettering object.
pub fn lettering_of(shape: &Shape) -> Option<Lettering> {
    if let ShapeKind::Group { .. } = shape.kind {
        return shape
            .data
            .iter()
            .find(|(k, _)| k == LETTERING_KEY)
            .and_then(|(_, v)| serde_json::from_str::<Lettering>(v).ok());
    }
    match sew::slots(shape).stroke {
        Slot::Set(s) => s.lettering,
        Slot::Auto => None,
    }
}

/// The colour lettering object `shape` sews in: its outline's, or its first
/// piece's.
pub fn color_of(shape: &Shape) -> Option<Color> {
    match &shape.kind {
        ShapeKind::Group { children } => children.iter().find_map(color_of),
        _ => shape
            .stroke
            .as_ref()
            .map(|s| s.color)
            .or(match &shape.fill {
                Fill::Solid(c) => Some(*c),
                _ => None,
            }),
    }
}

/// The strokes of `l` in object space (mm, y up, the first baseline from
/// the origin) and the characters the font does not have.
pub fn strokes(l: &Lettering) -> (BezPath, Vec<char>) {
    let (path, missing) = layout(l);
    // The layout is y down; the page is y up.
    (Affine::scale_non_uniform(1.0, -1.0) * path, missing)
}

/// The stitch settings a built-in lettering object stores.
fn settings(l: &Lettering) -> Slots {
    let l = l.sanitized();
    let mut s = StitchSettings::of(StitchType::Satin);
    s.satin.width = l.column_width();
    s.lettering = Some(l);
    Slots {
        fill: Slot::Set(StitchSettings::of(StitchType::None)),
        stroke: Slot::Set(s),
        stroke_first: false,
    }
}

fn off() -> Slot {
    Slot::Set(StitchSettings::of(StitchType::None))
}

/// The pieces of `l` in a digitized font, in the group's space (mm, y up),
/// each a converted object: the font's colours when it has several, else
/// `color`. With the characters the font does not have.
fn pieces(
    l: &Lettering,
    color: Color,
    ids: &mut dyn FnMut() -> ShapeId,
) -> Result<(Vec<Shape>, Vec<char>), String> {
    let font = tracedraw_emb_fonts::library::load(&l.font)?;
    let (placed, missing) = tracedraw_emb_fonts::layout(&font, &l.sanitized());
    let colours: std::collections::BTreeSet<[u8; 3]> = placed
        .iter()
        .flat_map(|g| g.pieces.iter())
        .flat_map(|p| [p.fill_rgb, p.stroke_rgb])
        .flatten()
        .collect();
    let own = colours.len() > 1;
    let paint = |rgb: Option<[u8; 3]>| match rgb {
        Some(c) if own => Color::rgb8(c[0], c[1], c[2]),
        _ => color,
    };
    let flip = Affine::scale_non_uniform(1.0, -1.0);
    let mut out = vec![];
    for g in &placed {
        for p in &g.pieces {
            let mut s = Shape::new(
                ids(),
                ShapeKind::Path {
                    path: flip * p.path.clone(),
                    closed: p.fill.is_some(),
                },
            );
            s.name = Some(g.name.clone());
            s.fill = match p.fill {
                Some(_) => Fill::Solid(paint(p.fill_rgb.or(p.stroke_rgb))),
                None => Fill::None,
            };
            let slots = Slots {
                fill: p.fill.clone().map_or_else(off, Slot::Set),
                stroke: p.stroke.clone().map_or_else(off, Slot::Set),
                stroke_first: false,
            };
            s.stroke = p.stroke.as_ref().map(|_| {
                let mut st = Stroke::hairline(paint(p.stroke_rgb.or(p.fill_rgb)));
                if let Some(w) = sew::stroke_width_for(&slots, true) {
                    st.width = w;
                }
                st
            });
            s.data = sew::data_with(&s, &slots);
            out.push(s);
        }
    }
    if out.is_empty() {
        return Err(format!(
            "the font {} has none of these letters",
            font.info.name
        ));
    }
    Ok((out, missing))
}

/// The text new lettering in `font` starts with: `text` when the font has
/// all its letters, else in capitals when it has those, else some letters
/// the font has.
pub fn sample_text(font: &str, text: &str) -> String {
    if is_builtin(font) {
        return text.to_string();
    }
    let Ok(f) = tracedraw_emb_fonts::library::load(font) else {
        return text.to_string();
    };
    let fits = |t: &str| {
        let l = Lettering {
            text: t.to_string(),
            font: font.to_string(),
            ..Default::default()
        };
        tracedraw_emb_fonts::layout(&f, &l).1.is_empty()
    };
    if fits(text) {
        return text.to_string();
    }
    let upper = text.to_uppercase();
    if fits(&upper) {
        return upper;
    }
    let mut letters: Vec<&String> = f
        .glyphs
        .keys()
        .filter(|n| n.chars().count() == 1 && n.chars().all(char::is_alphanumeric))
        .collect();
    letters.sort();
    let some: String = letters.into_iter().take(6).map(String::as_str).collect();
    if some.is_empty() {
        text.to_string()
    } else {
        some
    }
}

/// A digitized lettering group's data: its settings, nothing else of the
/// embroidery's.
fn group_data(shape: &Shape, l: &Lettering) -> Vec<(String, String)> {
    let mut data: Vec<(String, String)> = shape
        .data
        .iter()
        .filter(|(k, _)| k != LETTERING_KEY && k != sew::STITCH_KEY)
        .cloned()
        .collect();
    data.push((
        LETTERING_KEY.to_string(),
        serde_json::to_string(&l.sanitized()).unwrap_or_default(),
    ));
    data
}

/// A new lettering object, its first baseline starting at `x`, `y` (page
/// mm), in `color`; new ids from `ids`. With the characters the font does
/// not have and the lettering's width (mm).
pub fn new_shape(
    id: ShapeId,
    l: &Lettering,
    x: f64,
    y: f64,
    color: Color,
    ids: &mut dyn FnMut() -> ShapeId,
) -> Result<(Shape, Vec<char>, f64), String> {
    let name = Some(l.text.lines().next().unwrap_or_default().to_string());
    if is_builtin(&l.font) {
        let (path, missing) = strokes(l);
        let width = kurbo::Shape::bounding_box(&path).width();
        let mut s = Shape::new(
            id,
            ShapeKind::Path {
                path,
                closed: false,
            },
        );
        s.transform = Affine::translate((x, y));
        s.fill = Fill::None;
        let mut st = Stroke::hairline(color);
        st.width = l.sanitized().column_width();
        s.stroke = Some(st);
        s.name = name;
        s.data = sew::data_with(&s, &settings(l));
        return Ok((s, missing, width));
    }
    let (children, missing) = pieces(l, color, ids)?;
    let width = children
        .iter()
        .map(|c| kurbo::Shape::bounding_box(&c.page_path()))
        .reduce(|a, b| a.union(b))
        .map_or(0.0, |b| b.width());
    let mut g = Shape::new(id, ShapeKind::Group { children });
    g.transform = Affine::translate((x, y));
    g.name = name;
    g.data = group_data(&g, l);
    Ok((g, missing, width))
}

/// The commands that change lettering object `shape` (as stored, in its
/// own space) to `l`: laid out again, as a path in the built-in font or a
/// group of pieces in a digitized one (either may become the other), with
/// the settings and the outline width; new ids from `ids`. With the
/// characters the font does not have.
pub fn edit(
    shape: &Shape,
    l: &Lettering,
    ids: &mut dyn FnMut() -> ShapeId,
) -> Result<(Vec<Command>, Vec<char>), String> {
    let color = color_of(shape).unwrap_or(Color::BLACK);
    if !is_builtin(&l.font) {
        let (children, missing) = pieces(l, color, ids)?;
        let cmds = vec![
            Command::SetShapeKind {
                shape: shape.id,
                kind: ShapeKind::Group { children },
            },
            Command::SetObjectData {
                shape: shape.id,
                data: group_data(shape, l),
            },
        ];
        return Ok((cmds, missing));
    }
    let (path, missing) = strokes(l);
    // From a group: back to the built-in font's path, without the group's
    // lettering data.
    let mut base = shape.clone();
    base.data.retain(|(k, _)| k != LETTERING_KEY);
    let mut cmds = vec![
        Command::SetShapeKind {
            shape: shape.id,
            kind: ShapeKind::Path {
                path,
                closed: false,
            },
        },
        Command::SetObjectData {
            shape: shape.id,
            data: sew::data_with(&base, &settings(l)),
        },
    ];
    let w = l.sanitized().column_width();
    let stroke = shape
        .stroke
        .clone()
        .unwrap_or_else(|| Stroke::hairline(color));
    if shape.stroke.is_none() || (stroke.width - w).abs() > 1e-9 {
        let mut st = stroke;
        st.width = w;
        cmds.push(Command::SetStroke {
            shapes: vec![shape.id],
            stroke: Some(st),
        });
    }
    if !matches!(shape.fill, Fill::None) {
        cmds.push(Command::SetFill {
            shapes: vec![shape.id],
            fill: Fill::None,
        });
    }
    Ok((cmds, missing))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape as _;

    #[test]
    fn lettering_sews_as_satin_and_stays_editable() {
        let l = Lettering {
            text: "AB".into(),
            height: 10.0,
            ..Default::default()
        };
        let (s, missing, _) = new_shape(ShapeId(1), &l, 20.0, 50.0, Color::BLACK, &mut || {
            ShapeId(99)
        })
        .unwrap();
        assert!(missing.is_empty());
        assert_eq!(lettering_of(&s).map(|x| x.text), Some("AB".into()));
        // The letters stand on the baseline: up from y = 50 on the page.
        let b = s.page_path().bounding_box();
        assert!(b.y0 >= 49.0 && b.y1 > 55.0 && b.y1 < 62.0, "{b:?}");
        let fabric = tracedraw_emb_stitch::Fabric::default_fabric();
        let parts = sew::sewn(&s, fabric);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].1.kind, StitchType::Satin);
        // The column width suits the height, not the outline.
        assert!((parts[0].1.satin.width - l.column_width()).abs() < 1e-9);
    }

    #[test]
    fn missing_characters_are_reported() {
        let l = Lettering {
            text: "A\u{2603}".into(),
            ..Default::default()
        };
        let (_, missing) = strokes(&l);
        assert_eq!(missing, vec!['\u{2603}']);
    }
}
