/// How the traversal reacts after emitting an Error event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorReaction {
    /// Emit Error(..), then keep going.
    Continue,
    /// Emit Error(..), then stop producing events immediately.
    FailFast,
}

#[derive(Debug, Clone)]
pub struct ErrorPolicy {
    pub on_stat_failed: ErrorReaction,
    pub on_expand_failed: ErrorReaction,
    pub on_read_dir_entry_failed: ErrorReaction,
}

impl Default for ErrorPolicy {
    fn default() -> Self {
        Self {
            on_stat_failed: ErrorReaction::Continue,
            on_expand_failed: ErrorReaction::Continue,
            on_read_dir_entry_failed: ErrorReaction::Continue,
        }
    }
}