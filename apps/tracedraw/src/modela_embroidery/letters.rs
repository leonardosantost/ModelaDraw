//! Lettering: text sewn as satin columns along each letter's centre line,
//! in the built-in centre-line font.
//!
//! A lettering object is a path whose subpaths are the letters' strokes, in
//! the object's own space with the first baseline starting at its origin;
//! its transform places it, so it can be moved and turned like any object.
//! Its outline part is a satin with the lettering settings, so the text
//! stays editable: changing it lays the strokes out again where they are.

use super::sew::{self, Slot, Slots};
use tracedraw_core::geometry::{Affine, BezPath};
use tracedraw_core::{Color, Command, Fill, Shape, ShapeId, ShapeKind, Stroke};
use tracedraw_emb_stitch::lettering::{layout, Lettering};
use tracedraw_emb_stitch::{StitchSettings, StitchType};

/// The lettering settings of `shape`, when it is a lettering object.
pub fn lettering_of(shape: &Shape) -> Option<Lettering> {
    match sew::slots(shape).stroke {
        Slot::Set(s) => s.lettering,
        Slot::Auto => None,
    }
}

/// The strokes of `l` in object space (mm, y up, the first baseline from
/// the origin) and the characters the font does not have.
pub fn strokes(l: &Lettering) -> (BezPath, Vec<char>) {
    let (path, missing) = layout(l);
    // The layout is y down; the page is y up.
    (Affine::scale_non_uniform(1.0, -1.0) * path, missing)
}

/// The stitch settings a lettering object stores.
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

/// A new lettering object, its first baseline starting at `x`, `y` (page
/// mm), in `color`.
pub fn new_shape(id: ShapeId, l: &Lettering, x: f64, y: f64, color: Color) -> (Shape, Vec<char>) {
    let (path, missing) = strokes(l);
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
    s.name = Some(l.text.lines().next().unwrap_or_default().to_string());
    s.data = sew::data_with(&s, &settings(l));
    (s, missing)
}

/// The commands that change lettering object `shape` (as stored, in its
/// own space) to `l`: the strokes laid out again, the settings and the
/// outline width.
pub fn edit(shape: &Shape, l: &Lettering) -> Vec<Command> {
    let (path, _) = strokes(l);
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
            data: sew::data_with(shape, &settings(l)),
        },
    ];
    if let Some(st) = &shape.stroke {
        let w = l.sanitized().column_width();
        if (st.width - w).abs() > 1e-9 {
            let mut st = st.clone();
            st.width = w;
            cmds.push(Command::SetStroke {
                shapes: vec![shape.id],
                stroke: Some(st),
            });
        }
    }
    cmds
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
        let (s, missing) = new_shape(ShapeId(1), &l, 20.0, 50.0, Color::BLACK);
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
