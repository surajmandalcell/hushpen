//! Scans of a capture against the hook tree the same frame gave: where color may appear, which
//! lines may appear, and how tall each row of a group is.

use image::RgbaImage;
use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn right(self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    fn inflated(self, by: f32) -> Rect {
        Rect {
            x: self.x - by,
            y: self.y - by,
            w: self.w + 2.0 * by,
            h: self.h + 2.0 * by,
        }
    }

    fn has(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

/// One element the frame registered for the hook, in logical pixels of the window.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub role: Option<String>,
    pub rect: Rect,
    pub focused: bool,
    pub checked: Option<bool>,
    pub visible: bool,
}

impl Node {
    fn is(&self, role: &str) -> bool {
        self.role.as_deref() == Some(role)
    }
}

/// How far a ring or an anti-aliased edge reaches past the bounds of its control.
const BLEED: f32 = 3.0;
/// A pixel has a hue when its channels differ by more than this.
const HUE: u8 = 24;
/// The most two pixels of a line may differ from the pixels next to it and still be one line.
const EDGE: i32 = 4;
/// The shortest run of pixels that counts as a line, in logical pixels.
const MIN_LINE: f32 = 40.0;
/// The widest a push button is; a wider pressable element is a list row.
const WIDEST_BUTTON: f32 = 300.0;
/// The window lights Linux draws in the title bar: the only colored shapes outside a control.
const WINDOW_LIGHTS: Rect = Rect {
    x: 0.0,
    y: 0.0,
    w: 64.0,
    h: 52.0,
};

fn scale_of(image: &RgbaImage) -> f32 {
    image.width() as f32 / 940.0
}

/// The controls and words inside which color is allowed, each with the control that owns it.
fn color_owners(nodes: &[Node]) -> Vec<(String, Rect)> {
    let mut owners = vec![("window lights".to_owned(), WINDOW_LIGHTS)];
    for node in nodes.iter().filter(|node| node.visible) {
        // A list row that takes a press is not a push button: it has no color of its own.
        let allowed = (node.is("Button") && node.rect.w < WIDEST_BUTTON)
            || node.is("Link")
            || node.is("Slider")
            || node.is("Status")
            || (node.is("Switch") && node.checked == Some(true))
            || node.focused
            || node.id.contains("status")
            || node.id.contains(".state.");
        if allowed {
            owners.push((node.id.clone(), node.rect.inflated(BLEED)));
        }
    }
    owners
}

/// Lists each owner that holds colored pixels, and returns the colored pixels no owner holds.
pub fn colored_regions(image: &RgbaImage, nodes: &[Node]) -> (String, Vec<String>) {
    let scale = scale_of(image);
    let owners = color_owners(nodes);
    let mut held = vec![0usize; owners.len()];
    let mut stray: Vec<(u32, u32)> = Vec::new();
    for (x, y, pixel) in image.enumerate_pixels() {
        let [r, g, b, _] = pixel.0;
        if r.max(g).max(b) - r.min(g).min(b) <= HUE {
            continue;
        }
        let (lx, ly) = (x as f32 / scale, y as f32 / scale);
        match owners.iter().position(|(_, rect)| rect.has(lx, ly)) {
            Some(index) => held[index] += 1,
            None => stray.push((x, y)),
        }
    }
    let mut report = String::new();
    for ((owner, rect), count) in owners.iter().zip(held) {
        if count > 0 {
            writeln!(
                report,
                "color {count:>5} px in {owner} at ({:.0}, {:.0}) {:.0}x{:.0}",
                rect.x, rect.y, rect.w, rect.h
            )
            .unwrap();
        }
    }
    let mut problems = Vec::new();
    if let Some(&(x0, y0)) = stray.first() {
        let (mut left, mut top, mut right, mut bottom) = (x0, y0, x0, y0);
        for &(x, y) in &stray {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
        problems.push(format!(
            "{} colored pixels outside every allowed control, within ({:.0}, {:.0})-({:.0}, {:.0}), first at ({x0}, {y0}) physical",
            stray.len(),
            left as f32 / scale,
            top as f32 / scale,
            right as f32 / scale,
            bottom as f32 / scale,
        ));
    }
    (report, problems)
}

#[derive(Debug, Clone, Copy)]
struct Line {
    horizontal: bool,
    /// The row (horizontal) or column (vertical) of the line, in logical pixels.
    at: f32,
    from: f32,
    to: f32,
}

fn differs(a: [u8; 4], b: [u8; 4]) -> bool {
    (0..3).any(|channel| (i32::from(a[channel]) - i32::from(b[channel])).abs() > EDGE)
}

/// The lines of the capture: runs of at least 40 logical pixels, one or two physical pixels
/// thick, that differ from the pixels on both sides.
fn lines_of(image: &RgbaImage) -> Vec<Line> {
    let scale = scale_of(image);
    let min_run = (MIN_LINE * scale) as u32;
    let (width, height) = image.dimensions();
    let pixel = |x: u32, y: u32| image.get_pixel(x, y).0;
    let mut found = Vec::new();
    for horizontal in [true, false] {
        let (across, along) = if horizontal {
            (height, width)
        } else {
            (width, height)
        };
        let at = |lane: u32, step: u32| {
            if horizontal {
                pixel(step, lane)
            } else {
                pixel(lane, step)
            }
        };
        for thickness in 1..=2u32 {
            for lane in 1..across.saturating_sub(thickness + 1) {
                let mut run_start = None;
                for step in 0..=along {
                    let on = step < along && {
                        let here = at(lane, step);
                        (1..thickness).all(|extra| !differs(here, at(lane + extra, step)))
                            && differs(here, at(lane - 1, step))
                            && differs(here, at(lane + thickness, step))
                    };
                    match (on, run_start) {
                        (true, None) => run_start = Some(step),
                        (false, Some(start)) => {
                            if step - start >= min_run {
                                found.push(Line {
                                    horizontal,
                                    at: lane as f32 / scale,
                                    from: start as f32 / scale,
                                    to: step as f32 / scale,
                                });
                            }
                            run_start = None;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    found
}

/// Why a line may be there, or `None` when it may not.
fn reason(line: Line, nodes: &[Node]) -> Option<&'static str> {
    const TOLERANCE: f32 = 1.5;
    let visible = || nodes.iter().filter(|node| node.visible);
    if visible()
        .filter(|node| node.is("Slider"))
        .any(|node| node.rect.inflated(BLEED).has(line.from + 1.0, line.at))
    {
        return Some("the track of a slider");
    }
    for group in visible().filter(|node| node.is("Group")) {
        let rect = group.rect;
        if line.horizontal {
            if line.from < rect.x - TOLERANCE || line.to > rect.right() + TOLERANCE {
                continue;
            }
            if (line.at - rect.y).abs() <= TOLERANCE || (line.at - rect.bottom()).abs() <= TOLERANCE
            {
                return Some("group outline");
            }
            if line.at > rect.y && line.at < rect.bottom() {
                // The inset is at least the side padding of a row.
                return (line.from >= rect.x + 9.0).then_some("hairline between rows, inset");
            }
        } else {
            if line.from < rect.y - TOLERANCE || line.to > rect.bottom() + TOLERANCE {
                continue;
            }
            if (line.at - rect.x).abs() <= TOLERANCE || (line.at - rect.right()).abs() <= TOLERANCE
            {
                return Some("group outline");
            }
        }
    }
    None
}

/// Lists each line and why it is allowed; returns the lines that are not allowed.
pub fn line_report(image: &RgbaImage, nodes: &[Node]) -> (String, Vec<String>) {
    let mut report = String::new();
    let mut problems = Vec::new();
    for line in lines_of(image) {
        let kind = if line.horizontal {
            "horizontal"
        } else {
            "vertical"
        };
        let place = format!(
            "{kind} line at {:.1}, {:.0} to {:.0}",
            line.at, line.from, line.to
        );
        match reason(line, nodes) {
            Some(why) => writeln!(report, "line {place}: {why}").unwrap(),
            None => {
                writeln!(report, "line {place}: NOT ALLOWED").unwrap();
                problems.push(format!("{place} is not a group outline or a row hairline"));
            }
        }
    }
    (report, problems)
}

/// The room a row has for its text: 13 px body at 1.3 and 11 px secondary at 1.3.
const ROW_MIN: f32 = 36.0;
const ROW_MAX: f32 = 36.0 + 11.0 * 1.3 + 1.0;
const TITLE_MAX: f32 = 13.0 * 1.3 + 1.0;
const SECONDARY_MAX: f32 = 11.0 * 1.3 + 1.0;

/// The rows of every group, as `(group, row, height)`, from the elements that span a group.
pub fn rows_of(nodes: &[Node]) -> Vec<(String, String, f32)> {
    let mut rows = Vec::new();
    for group in nodes.iter().filter(|node| node.visible && node.is("Group")) {
        let rect = group.rect;
        let mut spanning: Vec<&Node> = nodes
            .iter()
            .filter(|node| {
                node.visible
                    && !node.is("Group")
                    && node.rect.x <= rect.x + 1.5
                    && node.rect.w >= rect.w - 3.0
                    && node.rect.y >= rect.y - 0.5
                    && node.rect.bottom() <= rect.bottom() + 0.5
            })
            .collect();
        spanning.sort_by(|a, b| a.rect.y.total_cmp(&b.rect.y));
        for node in spanning {
            rows.push((group.id.clone(), node.id.clone(), node.rect.h));
        }
    }
    rows
}

/// A table of the rows of every group, and the rows and lines of text that break the sizes.
pub fn row_report(nodes: &[Node]) -> (String, Vec<String>) {
    let mut report = String::new();
    let mut problems = Vec::new();
    let rows = rows_of(nodes);
    if rows.is_empty() {
        problems.push("no group has a row".to_owned());
    }
    for node in nodes
        .iter()
        .filter(|node| node.id.starts_with("content.") || node.id == "history.row.0")
    {
        writeln!(
            report,
            "node {} at ({:.1}, {:.1}) {:.1}x{:.1}",
            node.id, node.rect.x, node.rect.y, node.rect.w, node.rect.h
        )
        .unwrap();
    }
    for (group, row, height) in rows {
        writeln!(report, "row {height:>6.1} px {row} in {group}").unwrap();
        if height < ROW_MIN - 0.1 || height > ROW_MAX {
            problems.push(format!(
                "{row} in {group} is {height:.1} px high, outside {ROW_MIN} to {ROW_MAX:.1}"
            ));
        }
    }
    for node in nodes
        .iter()
        .filter(|node| node.visible && !node.is("Group"))
    {
        let limit = if node.id.ends_with(".title") {
            TITLE_MAX
        } else if node.id.ends_with(".detail") || node.id.ends_with(".help") {
            SECONDARY_MAX
        } else {
            continue;
        };
        if node.rect.h > limit {
            problems.push(format!(
                "{} is {:.1} px high: more than one line",
                node.id, node.rect.h
            ));
        }
    }
    (report, problems)
}
