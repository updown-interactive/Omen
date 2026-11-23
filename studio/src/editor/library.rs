//! View library metadata layered over the compiler's canonical snippets.
use omen_macro_core::editor::{DslCompletion, view_library};
#[derive(Clone, Debug)]
pub struct LibraryItem {
    pub snippet: DslCompletion,
    pub category: &'static str,
    pub summary: &'static str,
    pub documentation: &'static str,
    pub icon: &'static str,
}
impl LibraryItem {
    pub fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        query.split_whitespace().all(|word| {
            self.snippet.name.to_lowercase().contains(word)
                || self.summary.to_lowercase().contains(word)
                || self.category.to_lowercase().contains(word)
        })
    }
}
pub fn items(source: &str, cursor: usize) -> Vec<LibraryItem> {
    view_library(source, cursor).into_iter().map(|snippet| {
        let (category, summary, documentation, icon) = match snippet.name.as_str() {
            "Text" => ("Display", "Display a string of text.", "Use Text for titles, labels and paragraphs. Set fontSize, weight and color with modifiers. Its string argument is an editable snippet placeholder.", "file"),
            "Button" => ("Controls", "Run an action when clicked or activated.", "Provide a visible title and an action body. Keyboard users can activate a focused button. Replace the action placeholder with your environment or state method.", "plus"),
            "Layout" => ("Containers", "Arrange child views along an axis.", "Layout groups child views. Choose .mode(.vertical), .mode(.horizontal) or .mode(.stack), then use spacing, padding and alignment to arrange the content.", "files"),
            "AdaptiveLayout" => ("Containers", "Switch between a column and a row as space changes.", "Use adaptive direction to choose the arrangement for each width category.", "files"),
            "AdaptiveGrid" => ("Containers", "Wrap items into equal-width columns.", "Set .min_item_width(240) and spacing to control how many columns fit.", "files"),
            "Card" => ("Containers", "Group related content in a card.", "Place child views inside the card body. Customize padding, background and cornerRadius using Omen modifiers.", "files"),
            "List" => ("Containers", "Render a collection using stable item identities.", "Supply your items and an identity closure, then describe each row with the item binding. Use .virtualized(28.) for fixed-height rows in a large collection.", "files"),
            "TextField" => ("Input", "Edit a single line of text.", "The title is a placeholder and accessible label. Bind a String state field using .value($text). Changes flow through Omen's existing reactive binding runtime.", "file-code"),
            "TextEditor" => ("Input", "Edit multiple lines of text.", "Bind text with .value($text), or attach an EditorDocument with .document(document) for the native code editor. The inserted snippet starts with a simple text binding.", "file-code"),
            "SecureField" => ("Input", "Enter masked text such as a password.", "Provide a meaningful accessible label and bind the value using .value($password). The native input hides entered characters.", "file-code"),
            "Checkbox" => ("Controls", "Select or clear a boolean value.", "Use .checked($checked) to bind a boolean. The visible label explains the setting and is also exposed to assistive technology.", "check"),
            "Toggle" => ("Controls", "Switch a boolean setting on or off.", "Use .checked($enabled) with reactive boolean state. Keep the label concise and describe the enabled behavior.", "check"),
            "Image" => ("Media", "Display an image asset.", "Replace the path placeholder with a local image asset. Use frame to control the image dimensions.", "file-image"),
            "Icon" => ("Media", "Display a framework SVG icon.", "Use a bundled icon name such as settings, search or spanner, or a local SVG path. Set frame and color to style the icon.", "file-image"),
            "Progress" => ("Display", "Show fractional progress.", "Supply a progress fraction between 0.0 and 1.0. Bind an expression to reflect changing application progress.", "refresh"),
            "Gap" => ("Layout", "Add space along the parent layout’s axis.", "Adds width in a row and height in a column. Gap() uses eight logical pixels. Supply a size to change it.", "file"),
            "Divider" => ("Display", "Separate adjacent sections.", "Insert a divider between related groups. Omen uses the active theme to render the separator.", "file"),
            "NavigationStack" => ("Navigation", "Host a stack of navigable views.", "Bind an existing Navigator and place the initial content inside the stack. Replace the navigator placeholder with your state or environment binding.", "files"),
            "NavigationBar" => ("Navigation", "Add a titled navigation bar.", "Provide a title and child content. Use this view with the existing Omen navigation APIs.", "files"),
            "TabView" => ("Navigation", "Switch between labeled content tabs.", "Bind the selected tab identifier and describe each page with a Tab child. Each identifier must match your selection value's type.", "files"),
            "Tab" => ("Navigation", "Define one page inside a TabView.", "Supply a stable tab identifier, visible title and content. Insert this component as a child of TabView.", "file"),
            _ => ("Views", "An Omen view component.", "Insert the component and replace its snippet placeholders.", "file-omen-view"),
        };
        LibraryItem { snippet, category, summary, documentation, icon }
    }).collect()
}
