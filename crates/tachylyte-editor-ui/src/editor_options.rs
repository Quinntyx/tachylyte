//! State and events for editor presentation controls.
//!
//! This module intentionally does not edit document text or depend on GPUI.  A
//! `MarkdownEditor` can own an [`EditorOptions`] and translate its events into
//! toolbar and menu actions at the application boundary.

/// Whether long lines remain on one visual row or wrap to the editor width.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LineWrap {
    #[default]
    NoWrap,
    Wrap,
}

/// Explicit placeholders for features whose engines are not implemented yet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SpellcheckState {
    /// Spellcheck is not implemented; this is not an enabled spellchecker.
    #[default]
    Unavailable,
}

/// Explicit placeholder for Vim emulation, kept separate from ordinary editor state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VimModeState {
    /// Vim mode is not implemented; the editor remains in its normal mode.
    #[default]
    Unavailable,
}

/// Actions exposed by a toolbar button.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolbarAction {
    ToggleLineWrap,
    ToggleLineNumbers,
    ToggleFoldMarkers,
    Spellcheck,
    VimMode,
}

/// Actions exposed by an editor menu.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    ToggleLineWrap,
    ToggleLineNumbers,
    ToggleFoldMarkers,
    Spellcheck,
    VimMode,
}

/// Notifications emitted after an option changes or a placeholder is invoked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorOptionsEvent {
    LineWrapChanged(LineWrap),
    LineNumbersChanged(bool),
    FoldMarkersChanged(bool),
    SpellcheckPlaceholder,
    VimModePlaceholder,
    Toolbar(ToolbarAction),
    Menu(MenuAction),
}

/// UI-only options for a Markdown editor.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EditorOptions {
    line_wrap: LineWrap,
    show_line_numbers: bool,
    show_fold_markers: bool,
    events: Vec<EditorOptionsEvent>,
}

impl EditorOptions {
    pub fn line_wrap(&self) -> LineWrap {
        self.line_wrap
    }
    pub fn show_line_numbers(&self) -> bool {
        self.show_line_numbers
    }
    pub fn show_fold_markers(&self) -> bool {
        self.show_fold_markers
    }
    pub fn spellcheck_state(&self) -> SpellcheckState {
        SpellcheckState::Unavailable
    }
    pub fn vim_mode_state(&self) -> VimModeState {
        VimModeState::Unavailable
    }

    pub fn set_line_wrap(&mut self, value: LineWrap) {
        if self.line_wrap != value {
            self.line_wrap = value;
            self.events.push(EditorOptionsEvent::LineWrapChanged(value));
        }
    }
    pub fn toggle_line_wrap(&mut self) {
        self.set_line_wrap(match self.line_wrap {
            LineWrap::NoWrap => LineWrap::Wrap,
            LineWrap::Wrap => LineWrap::NoWrap,
        });
    }
    pub fn set_line_numbers(&mut self, value: bool) {
        if self.show_line_numbers != value {
            self.show_line_numbers = value;
            self.events
                .push(EditorOptionsEvent::LineNumbersChanged(value));
        }
    }
    pub fn toggle_line_numbers(&mut self) {
        self.set_line_numbers(!self.show_line_numbers);
    }
    pub fn set_fold_markers(&mut self, value: bool) {
        if self.show_fold_markers != value {
            self.show_fold_markers = value;
            self.events
                .push(EditorOptionsEvent::FoldMarkersChanged(value));
        }
    }
    pub fn toggle_fold_markers(&mut self) {
        self.set_fold_markers(!self.show_fold_markers);
    }

    pub fn toolbar(&mut self, action: ToolbarAction) {
        self.events.push(EditorOptionsEvent::Toolbar(action));
        self.apply(action);
    }
    pub fn menu(&mut self, action: MenuAction) {
        self.events.push(EditorOptionsEvent::Menu(action));
        self.apply_menu(action);
    }
    pub fn take_events(&mut self) -> Vec<EditorOptionsEvent> {
        std::mem::take(&mut self.events)
    }

    fn apply(&mut self, action: ToolbarAction) {
        match action {
            ToolbarAction::ToggleLineWrap => self.toggle_line_wrap(),
            ToolbarAction::ToggleLineNumbers => self.toggle_line_numbers(),
            ToolbarAction::ToggleFoldMarkers => self.toggle_fold_markers(),
            ToolbarAction::Spellcheck => {
                self.events.push(EditorOptionsEvent::SpellcheckPlaceholder)
            }
            ToolbarAction::VimMode => self.events.push(EditorOptionsEvent::VimModePlaceholder),
        }
    }
    fn apply_menu(&mut self, action: MenuAction) {
        match action {
            MenuAction::ToggleLineWrap => self.toggle_line_wrap(),
            MenuAction::ToggleLineNumbers => self.toggle_line_numbers(),
            MenuAction::ToggleFoldMarkers => self.toggle_fold_markers(),
            MenuAction::Spellcheck => self.events.push(EditorOptionsEvent::SpellcheckPlaceholder),
            MenuAction::VimMode => self.events.push(EditorOptionsEvent::VimModePlaceholder),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_safe() {
        let options = EditorOptions::default();
        assert_eq!(options.line_wrap(), LineWrap::NoWrap);
        assert!(!options.show_line_numbers());
        assert!(!options.show_fold_markers());
        assert_eq!(options.spellcheck_state(), SpellcheckState::Unavailable);
    }
    #[test]
    fn toolbar_emits_intent_and_state_change() {
        let mut options = EditorOptions::default();
        options.toolbar(ToolbarAction::ToggleLineWrap);
        assert_eq!(options.line_wrap(), LineWrap::Wrap);
        assert_eq!(
            options.take_events(),
            vec![
                EditorOptionsEvent::Toolbar(ToolbarAction::ToggleLineWrap),
                EditorOptionsEvent::LineWrapChanged(LineWrap::Wrap)
            ]
        );
    }
}
