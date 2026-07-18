//! Pure geometry and presentation helpers used by the Canvas projection.
use tachylyte_structured::{CanvasDocument, Edge, Node, Point, Rect};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FitTransform {
    pub pan: Point,
    pub zoom: f64,
}

pub(crate) fn normalize_rect(r: Rect) -> Rect {
    let (x, width) = if r.width < 0. {
        (r.x + r.width, -r.width)
    } else {
        (r.x, r.width)
    };
    let (y, height) = if r.height < 0. {
        (r.y + r.height, -r.height)
    } else {
        (r.y, r.height)
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}
pub(crate) fn contains_rect(outer: Rect, inner: Rect) -> bool {
    let (a, b) = (normalize_rect(outer), normalize_rect(inner));
    b.x >= a.x && b.y >= a.y && b.x + b.width <= a.x + a.width && b.y + b.height <= a.y + a.height
}

pub(crate) fn fit_transform(
    document: &CanvasDocument,
    viewport: Rect,
    padding: f64,
) -> FitTransform {
    let bounds = document
        .nodes
        .iter()
        .map(Node::rect)
        .reduce(union)
        .unwrap_or_default();
    let p = padding.max(0.);
    let zw = (viewport.width - 2. * p) / bounds.width.max(1.);
    let zh = (viewport.height - 2. * p) / bounds.height.max(1.);
    let zoom = zw.min(zh).clamp(0.1, 8.0);
    FitTransform {
        zoom,
        pan: Point {
            x: viewport.x + (viewport.width - bounds.width * zoom) / 2. - bounds.x * zoom,
            y: viewport.y + (viewport.height - bounds.height * zoom) / 2. - bounds.y * zoom,
        },
    }
}
fn union(a: Rect, b: Rect) -> Rect {
    let a = normalize_rect(a);
    let b = normalize_rect(b);
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    Rect {
        x,
        y,
        width: (a.x + a.width).max(b.x + b.width) - x,
        height: (a.y + a.height).max(b.y + b.height) - y,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Side {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}
pub(crate) fn side(value: Option<&str>) -> Side {
    match value.unwrap_or("").to_ascii_lowercase().as_str() {
        "left" => Side::Left,
        "right" => Side::Right,
        "top" => Side::Top,
        "bottom" => Side::Bottom,
        _ => Side::Center,
    }
}
pub(crate) fn endpoint(node: &Node, requested: Option<&str>) -> Point {
    let r = normalize_rect(node.rect());
    match side(requested) {
        Side::Left => Point {
            x: r.x,
            y: r.y + r.height / 2.,
        },
        Side::Right => Point {
            x: r.x + r.width,
            y: r.y + r.height / 2.,
        },
        Side::Top => Point {
            x: r.x + r.width / 2.,
            y: r.y,
        },
        Side::Bottom => Point {
            x: r.x + r.width / 2.,
            y: r.y + r.height,
        },
        Side::Center => Point {
            x: r.x + r.width / 2.,
            y: r.y + r.height / 2.,
        },
    }
}
pub(crate) fn orthogonal_route(from: &Node, to: &Node, edge: &Edge) -> Vec<Point> {
    let a = endpoint(from, edge.from_side.as_deref());
    let b = endpoint(to, edge.to_side.as_deref());
    let bend = Point { x: b.x, y: a.y };
    vec![a, bend, b]
}

pub(crate) fn grid_lines(bounds: Rect, spacing: f64) -> (Vec<f64>, Vec<f64>) {
    if !spacing.is_finite() || spacing <= 0. {
        return (Vec::new(), Vec::new());
    }
    let r = normalize_rect(bounds);
    let xs = (0..=((r.width / spacing).floor() as usize))
        .map(|i| r.x + i as f64 * spacing)
        .collect();
    let ys = (0..=((r.height / spacing).floor() as usize))
        .map(|i| r.y + i as f64 * spacing)
        .collect();
    (xs, ys)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NodeType {
    Text,
    File,
    Link,
    Group,
}
pub(crate) fn node_type(node: &Node) -> NodeType {
    match node.kind.to_ascii_lowercase().as_str() {
        "file" => NodeType::File,
        "link" | "url" => NodeType::Link,
        "group" => NodeType::Group,
        _ => NodeType::Text,
    }
}
pub(crate) fn card_preview(node: &Node) -> Option<&str> {
    node.text
        .as_deref()
        .or(node.file.as_deref())
        .or(node.url.as_deref())
}
pub(crate) fn card_label(node: &Node) -> String {
    card_preview(node)
        .unwrap_or(&node.id)
        .lines()
        .next()
        .unwrap_or("")
        .to_owned()
}
pub(crate) fn parse_color(value: Option<&str>) -> [u8; 4] {
    let Some(raw) = value.map(str::trim) else {
        return [128, 128, 128, 255];
    };
    let palette = match raw {
        "1" => Some(0xd95555),
        "2" => Some(0xd9822b),
        "3" => Some(0xc49a2a),
        "4" => Some(0x4e9f6e),
        "5" => Some(0x3e9bb5),
        "6" => Some(0x8064a2),
        _ => None,
    };
    if let Some(rgb) = palette {
        return [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255];
    }
    let Some(s) = raw.strip_prefix('#') else {
        return [128, 128, 128, 255];
    };
    let hex = |x: &str| u8::from_str_radix(x, 16).ok();
    match s.len() {
        6 => [hex(&s[0..2]), hex(&s[2..4]), hex(&s[4..6]), Some(255)],
        8 => [hex(&s[0..2]), hex(&s[2..4]), hex(&s[4..6]), hex(&s[6..8])],
        _ => [None, None, None, None],
    }
    .map(|x| x.unwrap_or(128))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalize_and_sides() {
        let n = Node {
            id: "n".into(),
            kind: "text".into(),
            x: 10.,
            y: 20.,
            width: 20.,
            height: 10.,
            text: None,
            file: None,
            url: None,
            color: None,
            extra: Default::default(),
        };
        assert!(contains_rect(
            Rect {
                x: 0.,
                y: 0.,
                width: 40.,
                height: 40.
            },
            n.rect()
        ));
        assert_eq!(endpoint(&n, Some("left")), Point { x: 10., y: 25. });
    }
    #[test]
    fn colors() {
        assert_eq!(parse_color(Some("#123456")), [18, 52, 86, 255]);
        assert_eq!(parse_color(Some("bad")), [128, 128, 128, 255]);
    }
}
