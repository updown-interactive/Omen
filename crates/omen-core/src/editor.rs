//! Persistent text snapshots and a renderer-independent code-editor bridge.
use crate::Value;
use ropey::Rope;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    borrow::Cow,
    fmt,
    ops::{Bound, Range, RangeBounds},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// A cheap, persistent UTF-8 document snapshot backed by Ropey (MIT).
#[derive(Clone)]
pub struct TextSnapshot {
    rope: Rope,
    identity: u64,
}
impl Default for TextSnapshot {
    fn default() -> Self {
        Self::from("")
    }
}
impl From<&str> for TextSnapshot {
    fn from(text: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self {
            rope: Rope::from_str(text),
            identity: NEXT.fetch_add(1, Ordering::Relaxed),
        }
    }
}
impl From<String> for TextSnapshot {
    fn from(text: String) -> Self {
        Self::from(text.as_str())
    }
}
impl TextSnapshot {
    /// Byte length.
    pub fn len(&self) -> usize {
        self.rope.len_bytes()
    }
    /// Whether no text is present.
    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }
    /// Stable identity shared by clones; edits allocate a fresh identity.
    pub fn identity(&self) -> u64 {
        self.identity
    }
    /// Whether snapshots share the same editor history state.
    pub fn same_snapshot(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
    /// Number of logical lines, including a trailing empty line.
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }
    /// Line containing a byte offset.
    pub fn byte_to_line(&self, byte: usize) -> usize {
        self.rope.byte_to_line(byte.min(self.len()))
    }
    /// Byte offset at a line start.
    pub fn line_start(&self, line: usize) -> usize {
        self.rope.line_to_byte(line.min(self.line_count()))
    }
    /// Line content without its line ending.
    pub fn line(&self, line: usize) -> Cow<'_, str> {
        let start = self.line_start(line);
        let end = if line + 1 < self.line_count() {
            self.line_start(line + 1)
        } else {
            self.len()
        };
        let text = self.slice(start..end);
        match text {
            Cow::Borrowed(text) => Cow::Borrowed(text.trim_end_matches(['\r', '\n'])),
            Cow::Owned(mut text) => {
                while text.ends_with(['\r', '\n']) {
                    text.pop();
                }
                Cow::Owned(text)
            }
        }
    }
    /// Read a byte slice, allocating only when it spans rope chunks.
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Cow<'_, str> {
        let start = match range.start_bound() {
            Bound::Included(value) => *value,
            Bound::Excluded(value) => value + 1,
            Bound::Unbounded => 0,
        }
        .min(self.len());
        let end = match range.end_bound() {
            Bound::Included(value) => value + 1,
            Bound::Excluded(value) => *value,
            Bound::Unbounded => self.len(),
        }
        .min(self.len())
        .max(start);
        let start = self.rope.char_to_byte(self.rope.byte_to_char(start));
        let end = self.rope.char_to_byte(self.rope.byte_to_char(end));
        let slice = self.rope.byte_slice(start..end);
        slice
            .as_str()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(slice.to_string()))
    }
    /// UTF-16 code-unit offset corresponding to a byte offset.
    pub fn byte_to_utf16(&self, byte: usize) -> usize {
        self.rope
            .char_to_utf16_cu(self.rope.byte_to_char(byte.min(self.len())))
    }
    /// UTF-8 byte offset corresponding to a clamped UTF-16 offset.
    pub fn utf16_to_byte(&self, offset: usize) -> usize {
        self.rope.char_to_byte(
            self.rope
                .utf16_cu_to_char(offset.min(self.rope.len_utf16_cu())),
        )
    }
    /// UTF-16 length used by native IME and LSP.
    pub fn len_utf16(&self) -> usize {
        self.rope.len_utf16_cu()
    }
    /// Replace a byte range without copying the remaining document.
    pub fn replace(&mut self, range: Range<usize>, inserted: &str) {
        let start = self.rope.byte_to_char(range.start.min(self.len()));
        let end = self
            .rope
            .byte_to_char(range.end.min(self.len()).max(range.start.min(self.len())));
        if self.rope.slice(start..end) == inserted {
            return;
        }
        self.rope.remove(start..end);
        self.rope.insert(start, inserted);
        static NEXT_EDIT: AtomicU64 = AtomicU64::new(1 << 32);
        self.identity = NEXT_EDIT.fetch_add(1, Ordering::Relaxed);
    }
    /// Match ASCII brackets near the caret with bounded work, excluding literals/comments.
    pub fn bracket_pair(&self, cursor: usize) -> Option<(usize, usize)> {
        let cursor = cursor.min(self.len());
        let at = self.rope.byte_to_char(cursor);
        let here = if at < self.rope.len_chars() {
            Some((cursor, self.rope.char(at)))
        } else {
            None
        };
        let before = if at > 0 {
            Some((self.rope.char_to_byte(at - 1), self.rope.char(at - 1)))
        } else {
            None
        };
        let (target, _) = here
            .filter(|(_, ch)| "()[]{}".contains(*ch))
            .or_else(|| before.filter(|(_, ch)| "()[]{}".contains(*ch)))?;
        let start = self
            .rope
            .char_to_byte(self.rope.byte_to_char(target.saturating_sub(65536)));
        let text = self.slice(start..(target + 65536).min(self.len()));
        let bytes = text.as_bytes();
        let mut stack = Vec::new();
        let mut index = 0;
        let mut block = 0_u32;
        let mut string = false;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            let next = bytes.get(index + 1).copied();
            if block > 0 {
                if byte == b'/' && next == Some(b'*') {
                    block += 1;
                    index += 2;
                    continue;
                }
                if byte == b'*' && next == Some(b'/') {
                    block -= 1;
                    index += 2;
                    continue;
                }
                index += 1;
                continue;
            }
            if string {
                if byte == b'"' && !escaped {
                    string = false;
                }
                escaped = byte == b'\\' && !escaped;
                index += 1;
                continue;
            }
            if byte == b'"' {
                string = true;
                index += 1;
                continue;
            }
            if byte == b'/' && next == Some(b'/') {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                continue;
            }
            if byte == b'/' && next == Some(b'*') {
                block = 1;
                index += 2;
                continue;
            }
            if b"([{ ".contains(&byte) && byte != b' ' {
                stack.push((byte, start + index));
            } else if let Some(kind) = b")]}".iter().position(|closing| *closing == byte)
                && let Some((opening, offset)) = stack.pop()
            {
                if opening != b"([{ "[kind] {
                    stack.clear();
                } else if target == offset || target == start + index {
                    return Some((offset, start + index));
                }
            }
            index += 1;
        }
        None
    }
    /// Whether the snapshot contains a literal substring (for non-rendering callers).
    pub fn contains(&self, needle: &str) -> bool {
        self.to_string().contains(needle)
    }
}
impl fmt::Display for TextSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.rope.fmt(f)
    }
}
impl fmt::Debug for TextSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextSnapshot")
            .field("bytes", &self.len())
            .field("identity", &self.identity)
            .finish()
    }
}
impl PartialEq for TextSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.same_snapshot(other) || self.rope == other.rope
    }
}
impl Eq for TextSnapshot {}
impl PartialEq<str> for TextSnapshot {
    fn eq(&self, other: &str) -> bool {
        self.rope == other
    }
}
impl PartialEq<&str> for TextSnapshot {
    fn eq(&self, other: &&str) -> bool {
        self.rope == *other
    }
}
impl PartialEq<String> for TextSnapshot {
    fn eq(&self, other: &String) -> bool {
        self.rope == other.as_str()
    }
}
impl PartialEq<TextSnapshot> for String {
    fn eq(&self, other: &TextSnapshot) -> bool {
        other == self
    }
}
impl Serialize for TextSnapshot {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for TextSnapshot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from)
    }
}

/// Token categories shared by syntax engines and native renderers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxKind {
    /// Control-flow keyword.
    ControlFlow,
    /// Module or namespace.
    Namespace,
    /// Local variable or binding.
    Variable,
    /// Function parameter.
    Parameter,
    /// Struct field or property.
    Property,
    /// Constant or enum variant.
    Constant,
    /// Lifetime label.
    Lifetime,
    /// Attribute or derive annotation.
    Attribute,
    /// Arithmetic, comparison or assignment operator.
    Operator,
    /// Delimiter or punctuation.
    Punctuation,
    /// Boolean literal.
    Boolean,
    /// Escape sequence within a literal.
    Escape,
    /// Reserved language word.
    Keyword,
    /// Text/string literal.
    String,
    /// Source comment.
    Comment,
    /// Numeric literal.
    Number,
    /// Named type.
    Type,
    /// An Omen declarative view modifier.
    ViewModifier,
    /// Function/method.
    Function,
    /// Macro, attribute or Omen DSL directive.
    Macro,
}
/// A versioned UTF-8 syntax span.
#[derive(Clone, Debug)]
pub struct SyntaxSpan {
    /// Byte range.
    pub range: Range<usize>,
    /// Token category.
    pub kind: SyntaxKind,
}
/// A byte range with a message reported by a language server.
#[derive(Clone, Debug)]
pub struct EditorDiagnostic {
    /// UTF-8 byte range in the current document version.
    pub range: Range<usize>,
    /// Human-readable diagnostic.
    pub message: String,
    /// Whether this is an error rather than a warning/information.
    pub error: bool,
}
/// An additional edit applied in the same undo step as a completion.
#[derive(Clone, Debug)]
pub struct EditorCompletionEdit {
    /// Original UTF-8 range.
    pub range: Range<usize>,
    /// Replacement source.
    pub insert: String,
}
/// A completion with an explicit replacement range, supplied by the language server.
#[derive(Clone, Debug)]
pub struct EditorCompletion {
    /// User-facing label.
    pub label: String,
    /// Plain insertion text, including any snippet defaults.
    pub insert: String,
    /// Ordered editable byte ranges relative to insertion text; empty for plain completions.
    pub placeholders: Vec<Range<usize>>,
    /// Auxiliary source edits such as a missing import.
    pub additional_edits: Vec<EditorCompletionEdit>,
    /// Replacement byte range.
    pub range: Range<usize>,
}
/// Shared local editor state. Text clones are persistent, not whole-document copies.
#[derive(Clone, Debug, Default)]
pub struct EditorState {
    /// Current document snapshot.
    pub text: TextSnapshot,
    /// Additional spacing between graphemes, in logical pixels.
    pub letter_spacing: f32,
    /// External reload generation; changing it clears native undo history.
    pub history_epoch: u64,
    /// Native UTF-8 selection/caret.
    pub selection: Range<usize>,
    /// Additional UTF-8 selections for native multi-cursor editing.
    pub secondary_selections: Vec<Range<usize>>,
    /// Versioned diagnostic ranges.
    pub diagnostics: Arc<Vec<EditorDiagnostic>>,
    /// Sorted syntax spans for this snapshot.
    pub highlights: Arc<Vec<SyntaxSpan>>,
    /// Snapshot identity for the syntax spans.
    pub highlight_identity: Option<u64>,
    /// Current completion results.
    pub completions: Arc<Vec<EditorCompletion>>,
    /// Markdown hover documentation for the current request.
    pub hover: Option<String>,
    /// Available code-action titles for a transient native popup.
    pub actions: Arc<Vec<String>>,
    /// Byte offset of the current pointer hover request.
    pub hover_offset: Option<usize>,
    /// Currently selected completion row.
    pub completion_index: usize,
    /// One-shot externally inserted snippet navigation request.
    pub snippet_request: Option<EditorSnippet>,
    /// Select and scroll to an externally chosen range without moving keyboard focus.
    pub selection_request: Option<Range<usize>>,
    /// One-shot selection/reveal request from find, diagnostics or navigation.
    pub reveal: Option<Range<usize>>,
    /// Highlighted search matches in the current document.
    pub search_matches: Arc<Vec<Range<usize>>>,
    /// Currently active search match range.
    pub active_search_match: Option<Range<usize>>,
}
/// Events delivered from the code control without materializing full strings.
#[derive(Clone, Debug)]
pub enum EditorEvent {
    /// Content changed; the state contains its new snapshot.
    Edited,
    /// Selection changed.
    Selection,
    /// Keyboard focus left this editor.
    Blur,
    /// Request completion at the cursor.
    Complete,
    /// Request hover information.
    Hover,
    /// Request a definition location.
    Definition,
    /// Request document formatting.
    Format,
    /// Request information under the mouse without moving the caret.
    HoverAt(usize),
    /// Open the host's symbol rename input.
    Rename,
    /// Request quick fixes and refactorings.
    Actions,
    /// Apply an item from the current code-action popup.
    ApplyAction(usize),
    /// Open the document find/replace widget.
    Find,
}
/// A declarative document and its host callback, shared with a native TextEditor.
#[derive(Clone)]
pub struct EditorDocument {
    /// Observable document data.
    pub state: Value<EditorState>,
    callback: Rc<dyn Fn(EditorEvent)>,
}
impl EditorDocument {
    /// Connect an editor model to application services.
    pub fn new(state: Value<EditorState>, callback: impl Fn(EditorEvent) + 'static) -> Self {
        Self {
            state,
            callback: Rc::new(callback),
        }
    }
    /// Emit an editor event to its host.
    pub fn emit(&self, event: EditorEvent) {
        (self.callback)(event);
    }
}
impl PartialEq for EditorDocument {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state && Rc::ptr_eq(&self.callback, &other.callback)
    }
}
impl fmt::Debug for EditorDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EditorDocument(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_keep_large_documents_persistent_and_index_utf16() {
        let source = (0..100_000)
            .map(|index| format!("let item_{index} = \"👩‍💻\";\n"))
            .collect::<String>();
        let original = TextSnapshot::from(source.as_str());
        let mut edited = original.clone();
        let row = 75_000;
        let start = edited.line_start(row);
        let utf16 = edited.byte_to_utf16(start + 4);
        assert_eq!(edited.utf16_to_byte(utf16), start + 4);
        edited.replace(start + 4..start + 4, "new_");
        assert!(!edited.same_snapshot(&original));
        assert_eq!(original.line(row), "let item_75000 = \"👩‍💻\";");
        assert_eq!(edited.line(row), "let new_item_75000 = \"👩‍💻\";");
        assert_eq!(edited.line_count(), 100_001);
        assert_eq!(original.clone().identity(), original.identity());
    }
    #[test]
    fn bracket_matching_ignores_strings_and_comments() {
        let text = TextSnapshot::from("fn main() { let s = \"}\"; /* } */ if true { } }\n");
        let opening = text.to_string().find('{').unwrap();
        assert_eq!(text.bracket_pair(opening), Some((opening, text.len() - 2)));
        assert_eq!(text.slice(0..2), "fn");
        let unicode = TextSnapshot::from("é");
        assert_eq!(
            unicode.slice(1..2),
            "é",
            "byte slices clamp to Unicode scalar boundaries"
        );
    }
}

/// Native snippet navigation using absolute UTF-8 ranges in the current buffer.
#[derive(Clone, Debug)]
pub struct EditorSnippet {
    /// Editable ranges in Tab order, including an optional final empty caret.
    pub stops: Vec<Range<usize>>,
    /// Currently selected stop.
    pub active: usize,
}
impl EditorSnippet {
    /// Track an edit within the active placeholder, shifting later stops.
    /// Returns false when an edit leaves the snippet and should end navigation.
    pub fn edit(&mut self, range: &Range<usize>, inserted: usize) -> bool {
        let Some(active) = self.stops.get(self.active).cloned() else {
            return false;
        };
        if range.start < active.start || range.end > active.end {
            return false;
        }
        let removed = range.end - range.start;
        for (index, stop) in self.stops.iter_mut().enumerate() {
            if index == self.active {
                stop.end = stop.end - removed + inserted;
            } else if stop.start >= range.end {
                stop.start = stop.start - removed + inserted;
                stop.end = stop.end - removed + inserted;
            }
        }
        true
    }
    /// Move to an adjacent placeholder while the caret remains in the active slot.
    pub fn advance(&mut self, selection: &Range<usize>, backwards: bool) -> Option<Range<usize>> {
        let active = self.stops.get(self.active)?;
        if selection.start < active.start || selection.end > active.end {
            return None;
        }
        self.active = if backwards {
            self.active.saturating_sub(1)
        } else {
            self.active + 1
        };
        self.stops.get(self.active).cloned()
    }
}

#[cfg(test)]
mod snippet_tests {
    use super::*;
    #[test]
    fn unicode_edits_shift_later_stops_and_backwards_navigation_is_stable() {
        let mut snippet = EditorSnippet {
            stops: vec![5..9, 14..18, 20..20],
            active: 0,
        };
        assert!(snippet.edit(&(5..9), "👋".len()));
        assert_eq!(snippet.advance(&(9..9), false), Some(14..18));
        assert!(snippet.edit(&(14..18), 1));
        assert_eq!(snippet.advance(&(15..15), true), Some(5..9));
        assert_eq!(snippet.advance(&(9..9), false), Some(14..15));
        assert_eq!(snippet.advance(&(15..15), false), Some(17..17));
        assert!(snippet.advance(&(17..17), false).is_none());
    }
    #[test]
    fn editing_or_moving_outside_a_placeholder_ends_navigation() {
        let mut snippet = EditorSnippet {
            stops: vec![5..9, 15..15],
            active: 0,
        };
        assert!(!snippet.edit(&(0..1), 2));
        assert!(snippet.advance(&(10..10), false).is_none());
    }
}
