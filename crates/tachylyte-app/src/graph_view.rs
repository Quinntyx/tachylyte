//! Small, data-only graph scene primitives for the workspace shell.

use gpui::{div, prelude::*, px, rgb, AnyElement};

const PURPLE: u32 = 0x7852ee;
const PURPLE_TINT: u32 = 0xf0ebff;
const INK: u32 = 0x292433;
const MUTED: u32 = 0x756d7f;
const LINE: u32 = 0xd8d1df;

/// Builds a compact graph scene for the supplied note paths.
///
/// The scene always starts at a purple `Current / Welcome` node, followed by
/// one card for each path. Consecutive cards are separated by visible edge
/// glyphs, keeping the helper useful even when the caller has no link graph
/// beyond a list of notes.
///
/// `selected_path` is compared literally with each entry in `note_paths`.
/// Matching cards receive the same purple accent as the current node.
pub fn graph_scene(note_paths: &[String], selected_path: &str) -> AnyElement {
    let mut scene = div()
        .w_full()
        .min_h(px(180.))
        .p_5()
        .flex()
        .items_center()
        .gap_3()
        .overflow_hidden()
        .bg(rgb(0xfaf8fc));

    scene = scene.child(graph_node("Current / Welcome", true, true));

    for (index, path) in note_paths.iter().enumerate() {
        scene = scene.child(graph_edge(index));
        scene = scene.child(graph_node(path, false, path == selected_path));
    }

    scene.into_any_element()
}

fn graph_edge(index: usize) -> impl IntoElement {
    div()
        .id(("graph-edge", index))
        .flex()
        .items_center()
        .gap_1()
        .text_color(rgb(MUTED))
        .child(div().w(px(28.)).h(px(1.)).bg(rgb(LINE)))
        .child("›")
}

fn graph_node(label: &str, current: bool, selected: bool) -> impl IntoElement {
    let accent = current || selected;
    let border = if accent { PURPLE } else { LINE };
    let background = if current { PURPLE_TINT } else { 0xffffff };
    let caption = if current {
        "START"
    } else if selected {
        "SELECTED"
    } else {
        "NOTE"
    };

    div()
        .flex_shrink_0()
        .min_w(px(150.))
        .max_w(px(240.))
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(rgb(border))
        .bg(rgb(background))
        .text_color(rgb(INK))
        .child(div().text_xs().text_color(rgb(PURPLE)).child(caption))
        .child(div().mt_1().text_sm().child(label.to_owned()))
}
