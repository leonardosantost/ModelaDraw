//! The embroidery icons: the bar's views, overlays and tools, the player,
//! the docker tabs and the machine file menu rows.
//!
//! Each icon is outlines on a 24 x 24 grid (SVG path data, circles and
//! rounded rectangles) stroked in the icon colour, a few parts filled or
//! dashed. The stitch, hoop, jump, underlay and density icons are the
//! embroidery set credited in `ATTRIBUTION.md`; the others are Lucide
//! icons (ISC).

use crate::theme::Tokens;
use egui::{
    epaint, Color32, Painter, Pos2, Rect, Response, Sense, Shape as EShape, Stroke, Ui, Vec2,
};
use kurbo::{BezPath, PathEl, Shape as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    // The views.
    Vector,
    Stitches,
    Realistic,
    // The overlays.
    Jumps,
    Underlay,
    Density,
    // The tools.
    Lettering,
    Satin,
    RemoveOverlaps,
    SortByColor,
    CenterInHoop,
    MatchThreads,
    TurnHoop,
    // The sew-out player.
    Play,
    Pause,
    Rewind,
    Step,
    // The dockers and the menu.
    Embroidery,
    Sequence,
    Hoop,
    OpenMachine,
    ExportMachine,
}

/// The outline of one part of an icon, on the 24 x 24 grid.
#[derive(Clone, Copy)]
enum Geo {
    Path(&'static str),
    Circle(f64, f64, f64),
    /// x, y, width, height and the corner radius.
    Rect(f64, f64, f64, f64, f64),
}

/// One part of an icon: its outline, its stroke width (grid units), its
/// dashes (dash and gap), whether it is stroked and how opaque its fill is
/// (0: not filled).
#[derive(Clone, Copy)]
struct El {
    geo: Geo,
    width: f32,
    dash: Option<(f32, f32)>,
    stroke: bool,
    fill: f32,
}

/// The stroke width of a part that sets none.
const WIDTH: f32 = 1.6;

const fn p(d: &'static str) -> El {
    El {
        geo: Geo::Path(d),
        width: WIDTH,
        dash: None,
        stroke: true,
        fill: 0.0,
    }
}

const fn pw(d: &'static str, width: f32) -> El {
    El { width, ..p(d) }
}

const fn pd(d: &'static str, width: f32, dash: f32, gap: f32) -> El {
    El {
        width,
        dash: Some((dash, gap)),
        ..p(d)
    }
}

const fn c(x: f64, y: f64, r: f64) -> El {
    El {
        geo: Geo::Circle(x, y, r),
        ..p("")
    }
}

/// A filled dot, not stroked.
const fn dot(x: f64, y: f64, r: f64) -> El {
    El {
        stroke: false,
        fill: 1.0,
        ..c(x, y, r)
    }
}

const fn r(x: f64, y: f64, w: f64, h: f64, rx: f64) -> El {
    El {
        geo: Geo::Rect(x, y, w, h, rx),
        ..p("")
    }
}

/// A filled square, not stroked, `fill` opaque.
const fn cell(x: f64, y: f64, fill: f32) -> El {
    El {
        stroke: false,
        fill,
        ..r(x, y, 4.0, 4.0, 0.0)
    }
}

fn parts(icon: Icon) -> &'static [El] {
    use Icon::*;
    match icon {
        Vector => {
            const P: &[El] = &[
            p("M15.707 21.293a1 1 0 0 1-1.414 0l-1.586-1.586a1 1 0 0 1 0-1.414l5.586-5.586a1 1 0 0 1 1.414 0l1.586 1.586a1 1 0 0 1 0 1.414z"),
            p("m18 13-1.375-6.874a1 1 0 0 0-.746-.776L3.235 2.028a1 1 0 0 0-1.207 1.207L5.35 15.879a1 1 0 0 0 .776.746L13 18"),
            p("m2.3 2.3 7.286 7.286"),
            c(11.0, 11.0, 2.0),
        ];
            P
        }
        Stitches | Embroidery => {
            const P: &[El] = &[
                pw("M3 17 8 7l5 10 5-10 3 6", 1.2),
                dot(3.0, 17.0, 1.7),
                dot(8.0, 7.0, 1.7),
                dot(13.0, 17.0, 1.7),
                dot(18.0, 7.0, 1.7),
                dot(21.0, 13.0, 1.7),
            ];
            P
        }
        Realistic => {
            const P: &[El] = &[
            p("M17 13.44 4.442 17.082A2 2 0 0 0 4.982 21H19a2 2 0 0 0 .558-3.921l-1.115-.32A2 2 0 0 1 17 14.837V7.66"),
            p("m7 10.56 12.558-3.642A2 2 0 0 0 19.018 3H5a2 2 0 0 0-.558 3.921l1.115.32A2 2 0 0 1 7 9.163v7.178"),
        ];
            P
        }
        Jumps => {
            const P: &[El] = &[
                p("M3 18l2-3 2 3 2-3"),
                p("M15 9l2-3 2 3 2-3"),
                pd("M9 15C10 9 12 8 15 9", WIDTH, 2.2, 2.2),
                p("M6.5 6.5l3 3M9.5 6.5l-3 3"),
            ];
            P
        }
        Underlay => {
            const P: &[El] = &[
                p("M3 8l4.5-4 4.5 4 4.5-4L21 8"),
                pd("M3 14h18", 1.4, 2.5, 2.0),
                pd("M3 20h18", 1.4, 2.5, 2.0),
            ];
            P
        }
        Density => {
            const P: &[El] = &[
                r(3.0, 3.0, 18.0, 18.0, 2.0),
                cell(8.0, 8.0, 0.45),
                cell(12.0, 8.0, 1.0),
                cell(12.0, 12.0, 0.7),
                cell(8.0, 12.0, 0.25),
            ];
            P
        }
        Lettering => {
            const P: &[El] = &[
                p("M12 4v16"),
                p("M4 7V5a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v2"),
                p("M9 20h6"),
            ];
            P
        }
        Satin => {
            const P: &[El] = &[
                p("M3 9c5-5 13-6 18-3"),
                p("M5 18c4-4 10-5 15-3"),
                pw("M5 8.2 6.5 17", 1.4),
                pw("M9.5 6.2 10 15.6", 1.4),
                pw("M14 5.4 13.6 15", 1.4),
                pw("M18.5 5.5 17.4 15.2", 1.4),
            ];
            P
        }
        RemoveOverlaps => {
            const P: &[El] = &[
            p("M10 22a2 2 0 0 1-2-2"),
            p("M16 22h-2"),
            p("M16 4a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h3a1 1 0 0 0 1-1v-5a2 2 0 0 1 2-2h5a1 1 0 0 0 1-1z"),
            p("M20 8a2 2 0 0 1 2 2"),
            p("M22 14v2"),
            p("M22 20a2 2 0 0 1-2 2"),
        ];
            P
        }
        SortByColor => {
            const P: &[El] = &[
                p("m3 16 4 4 4-4"),
                p("M7 20V4"),
                p("M11 4h10"),
                p("M11 8h7"),
                p("M11 12h4"),
            ];
            P
        }
        CenterInHoop => {
            const P: &[El] = &[
                p("M12 2v20"),
                p("m15 19-3 3-3-3"),
                p("m19 9 3 3-3 3"),
                p("M2 12h20"),
                p("m5 9-3 3 3 3"),
                p("m9 5 3-3 3 3"),
            ];
            P
        }
        MatchThreads => {
            const P: &[El] = &[
            p("M11 17a4 4 0 0 1-8 0V5a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2Z"),
            p("M16.7 13H19a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H7"),
            dot(7.0, 17.0, 0.9),
            p("m11 8 2.3-2.3a2.4 2.4 0 0 1 3.404.004L18.6 7.6a2.4 2.4 0 0 1 .026 3.434L9.9 19.8"),
        ];
            P
        }
        TurnHoop => {
            const P: &[El] = &[
                p("M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"),
                p("M21 3v5h-5"),
            ];
            P
        }
        Play => {
            const P: &[El] = &[p(
            "M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z",
        )];
            P
        }
        Pause => {
            const P: &[El] = &[r(14.0, 3.0, 5.0, 18.0, 1.0), r(5.0, 3.0, 5.0, 18.0, 1.0)];
            P
        }
        Rewind => {
            const P: &[El] = &[
            p("M17.971 4.285A2 2 0 0 1 21 6v12a2 2 0 0 1-3.029 1.715l-9.997-5.998a2 2 0 0 1-.003-3.432z"),
            p("M3 20V4"),
        ];
            P
        }
        Step => {
            const P: &[El] = &[
            p("M21 4v16"),
            p("M6.029 4.285A2 2 0 0 0 3 6v12a2 2 0 0 0 3.029 1.715l9.997-5.998a2 2 0 0 0 .003-3.432z"),
        ];
            P
        }
        Sequence => {
            const P: &[El] = &[
                p("M11 5h10"),
                p("M11 12h10"),
                p("M11 19h10"),
                p("M4 4h1v5"),
                p("M4 9h2"),
                p("M6.5 20H3.4c0-1 2.6-1.925 2.6-3.5a1.5 1.5 0 0 0-2.6-1.02"),
            ];
            P
        }
        Hoop => {
            const P: &[El] = &[
                r(3.0, 4.0, 18.0, 17.0, 5.0),
                El {
                    width: 1.4,
                    dash: Some((2.2, 2.0)),
                    ..r(7.0, 8.0, 10.0, 9.0, 1.0)
                },
                p("M10 4V2h4v2"),
            ];
            P
        }
        OpenMachine => {
            const P: &[El] = &[p(
            "m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2",
        )];
            P
        }
        ExportMachine => {
            const P: &[El] = &[
            p("M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"),
            p("M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7"),
            p("M7 3v4a1 1 0 0 0 1 1h7"),
        ];
            P
        }
    }
}

/// How finely the outlines are flattened, grid units.
const TOLERANCE: f64 = 0.05;

fn outline(geo: Geo) -> BezPath {
    match geo {
        Geo::Path(d) => BezPath::from_svg(d).unwrap_or_default(),
        Geo::Circle(x, y, r) => kurbo::Circle::new((x, y), r).to_path(TOLERANCE),
        Geo::Rect(x, y, w, h, rx) => {
            kurbo::RoundedRect::new(x, y, x + w, y + h, rx).to_path(TOLERANCE)
        }
    }
}

/// The subpaths of `path` as polylines on the grid.
fn polylines(path: &BezPath) -> Vec<Vec<(f64, f64)>> {
    let mut out: Vec<Vec<(f64, f64)>> = vec![];
    kurbo::flatten(path, TOLERANCE, |el| match el {
        PathEl::MoveTo(q) => out.push(vec![(q.x, q.y)]),
        PathEl::LineTo(q) => {
            if let Some(line) = out.last_mut() {
                line.push((q.x, q.y));
            }
        }
        PathEl::ClosePath => {
            if let Some(line) = out.last_mut() {
                if let Some(&first) = line.first() {
                    line.push(first);
                }
            }
        }
        _ => {}
    });
    out
}

/// Draws `icon` filling `r` (square) in `color`.
pub fn draw(painter: &Painter, r: Rect, icon: Icon, color: Color32) {
    let s = r.width() / 24.0;
    let at = |x: f64, y: f64| r.min + Vec2::new(x as f32, y as f32) * s;
    for el in parts(icon) {
        if el.fill > 0.0 {
            let fill = color.gamma_multiply(el.fill);
            match el.geo {
                Geo::Circle(x, y, rad) => {
                    painter.circle_filled(at(x, y), rad as f32 * s, fill);
                }
                Geo::Rect(x, y, w, h, rx) => {
                    painter.rect_filled(
                        Rect::from_min_max(at(x, y), at(x + w, y + h)),
                        rx as f32 * s,
                        fill,
                    );
                }
                Geo::Path(_) => {}
            }
        }
        if !el.stroke {
            continue;
        }
        let stroke = Stroke::new(el.width * s, color);
        for line in polylines(&outline(el.geo)) {
            let pts: Vec<Pos2> = line.iter().map(|&(x, y)| at(x, y)).collect();
            let length: f32 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
            match (pts.first(), el.dash) {
                // A zero-length segment is a round dot.
                (Some(&first), _) if length < 0.2 * s => {
                    painter.circle_filled(first, stroke.width / 2.0, color);
                }
                (_, Some((dash, gap))) => {
                    painter.extend(EShape::dashed_line(&pts, stroke, dash * s, gap * s));
                }
                _ => {
                    painter.add(epaint::PathShape::line(pts, stroke));
                }
            }
        }
    }
}

/// Draws `icon` in the icon colour, for a docker tab.
pub fn draw_tab(painter: &Painter, r: Rect, icon: Icon) {
    draw(painter, r, icon, Tokens::icon());
}

/// The icon of a menu row, by its key.
pub fn for_menu(key: &str) -> Option<Icon> {
    match key {
        "menu.file.machine_open" => Some(Icon::OpenMachine),
        "menu.file.machine_export" => Some(Icon::ExportMachine),
        _ => None,
    }
}

/// The side of the bar's icon buttons and of their icons.
const BUTTON: f32 = 24.0;
const ICON: f32 = 16.0;

/// An icon button; `on` draws it pushed in.
pub fn button(ui: &mut Ui, icon: Icon, tip: &str, on: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(BUTTON), Sense::click());
    crate::ui::propbar::frame(ui, rect, resp.hovered(), on);
    draw(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(ICON)),
        icon,
        Tokens::icon(),
    );
    resp.on_hover_text(tip)
}

/// A button with an icon before its text.
pub fn labeled(ui: &mut Ui, icon: Icon, text: &str, tip: &str) -> Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, ui.visuals().text_color());
    let pad = 6.0;
    let size = Vec2::new(pad + ICON + 4.0 + galley.size().x + pad, BUTTON);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    crate::ui::propbar::frame(ui, rect, resp.hovered(), false);
    if !resp.hovered() {
        ui.painter().rect_stroke(
            rect.shrink(0.5),
            2.0,
            Stroke::new(1.0, Tokens::border()),
            egui::StrokeKind::Middle,
        );
    }
    let ir = Rect::from_center_size(
        egui::pos2(rect.left() + pad + ICON / 2.0, rect.center().y),
        Vec2::splat(ICON),
    );
    draw(ui.painter(), ir, icon, Tokens::icon());
    let text_at = egui::pos2(ir.right() + 4.0, rect.center().y - galley.size().y / 2.0);
    ui.painter()
        .galley(text_at, galley, ui.visuals().text_color());
    resp.on_hover_text(tip)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Icon; 22] = [
        Icon::Vector,
        Icon::Stitches,
        Icon::Realistic,
        Icon::Jumps,
        Icon::Underlay,
        Icon::Density,
        Icon::Lettering,
        Icon::Satin,
        Icon::RemoveOverlaps,
        Icon::SortByColor,
        Icon::CenterInHoop,
        Icon::MatchThreads,
        Icon::TurnHoop,
        Icon::Play,
        Icon::Pause,
        Icon::Rewind,
        Icon::Step,
        Icon::Embroidery,
        Icon::Sequence,
        Icon::Hoop,
        Icon::OpenMachine,
        Icon::ExportMachine,
    ];

    #[test]
    fn every_outline_parses_and_stays_on_the_grid() {
        for icon in ALL {
            for el in parts(icon) {
                let path = outline(el.geo);
                assert!(!path.elements().is_empty(), "{icon:?}: empty outline");
                let b = path.bounding_box();
                assert!(
                    b.x0 >= 0.0 && b.y0 >= 0.0 && b.x1 <= 24.0 && b.y1 <= 24.0,
                    "{icon:?}: {b:?} off the grid"
                );
            }
        }
    }

    #[test]
    fn every_icon_has_lines_to_draw() {
        for icon in ALL {
            let lines: usize = parts(icon)
                .iter()
                .filter(|el| el.stroke)
                .map(|el| {
                    polylines(&outline(el.geo))
                        .iter()
                        .filter(|l| l.len() > 1)
                        .count()
                })
                .sum();
            assert!(lines > 0, "{icon:?} has nothing to stroke");
        }
        // An arc command flattens into many points, not a straight chord.
        let arc = polylines(&outline(parts(Icon::TurnHoop)[0].geo));
        assert!(arc[0].len() > 20);
    }
}
