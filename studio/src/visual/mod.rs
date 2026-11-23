//! File-scoped canvas metadata and syntax-aware editing operations.
mod canvas;
mod placement;
mod render;
mod view_tree;
pub use canvas::{VisualCanvas, VisualToolbar};
use omen_macro_core::inspector::{InspectedNode, SourcePatch, visual_declarations, visual_nodes};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone)]
pub struct PreviewResult {
    pub path: PathBuf,
    pub identity: u64,
    pub preview_identity: u64,
    pub(crate) declarations: Arc<[PreviewDeclaration]>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub(crate) struct PreviewDeclaration {
    pub(crate) range: Range<usize>,
    pub(crate) marked: bool,
    pub(crate) tree: Result<Arc<VisualTree>, String>,
    pub(crate) fallback: Option<(u64, Arc<VisualTree>)>,
}

impl PreviewDeclaration {
    pub(crate) fn preview(&self, identity: u64) -> Option<(u64, Arc<VisualTree>)> {
        self.tree
            .as_ref()
            .ok()
            .map(|tree| (identity, tree.clone()))
            .or_else(|| self.fallback.clone())
    }
}

impl PreviewResult {
    pub(crate) fn at(&self, cursor: usize) -> Option<&PreviewDeclaration> {
        let index = self
            .declarations
            .partition_point(|declaration| declaration.range.start <= cursor);
        self.declarations.get(index.saturating_sub(1))
    }

    pub(crate) fn selected(&self, cursor: usize, opening: bool) -> Option<&PreviewDeclaration> {
        let caret = self.at(cursor);
        let marked = self
            .declarations
            .iter()
            .find(|declaration| declaration.marked);
        if opening || caret.is_none_or(|declaration| !declaration.range.contains(&cursor)) {
            marked.or(caret)
        } else {
            caret
        }
    }
}

pub type ProjectCanvases = BTreeMap<PathBuf, BTreeMap<PathBuf, CanvasSettings>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorMode {
    #[default]
    Design,
    Code,
    Split,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile {
    Android,
    #[default]
    IPhone,
    AndroidTablet,
    IPad,
    Mac,
    Windows,
    Linux,
    WebMobile,
    WebTablet,
    WebDesktop,
    Custom,
}

impl Profile {
    pub const ALL: [Self; 11] = [
        Self::Android,
        Self::IPhone,
        Self::AndroidTablet,
        Self::IPad,
        Self::Mac,
        Self::Windows,
        Self::Linux,
        Self::WebMobile,
        Self::WebTablet,
        Self::WebDesktop,
        Self::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Android => "Android phone",
            Self::IPhone => "iPhone",
            Self::AndroidTablet => "Android tablet",
            Self::IPad => "iPad",
            Self::Mac => "macOS",
            Self::Windows => "Windows",
            Self::Linux => "Linux",
            Self::WebMobile => "Mobile browser",
            Self::WebTablet => "Tablet browser",
            Self::WebDesktop => "Desktop browser",
            Self::Custom => "Custom",
        }
    }

    pub fn shell(self) -> omen::PreviewShell {
        use omen::PreviewShell as Shell;
        match self {
            Self::Android => Shell::AndroidPhone,
            Self::IPhone => Shell::IPhone,
            Self::AndroidTablet => Shell::AndroidTablet,
            Self::IPad => Shell::IPad,
            Self::Mac => Shell::Mac,
            Self::Windows => Shell::Windows,
            Self::Linux => Shell::Linux,
            Self::WebMobile => Shell::MobileBrowser,
            Self::WebTablet => Shell::TabletBrowser,
            Self::WebDesktop => Shell::DesktopBrowser,
            Self::Custom => Shell::Custom,
        }
    }

    pub fn size(self) -> (u16, u16) {
        match self {
            Self::Android => (360, 800),
            Self::IPhone => (390, 844),
            Self::AndroidTablet => (800, 1100),
            Self::IPad => (820, 1180),
            Self::Mac | Self::Windows | Self::Linux => (1024, 680),
            Self::WebMobile => (375, 812),
            Self::WebTablet => (768, 1024),
            Self::WebDesktop => (1280, 800),
            Self::Custom => (600, 500),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisualFrame {
    pub id: u64,
    pub name: String,
    pub profile: Profile,
    pub width: u16,
    pub height: u16,
    pub dark: bool,
    pub locale: String,
    pub safe_area: bool,
    pub position: [i32; 2],
}

impl VisualFrame {
    pub fn new(id: u64, profile: Profile) -> Self {
        let (width, height) = profile.size();
        Self {
            id,
            name: profile.label().into(),
            profile,
            width,
            height,
            dark: false,
            locale: "en".into(),
            safe_area: false,
            position: [18, 18],
        }
    }
}

impl VisualFrame {
    fn is_visible(&self, viewport: [f32; 2], scale: f32, origin: [f32; 2]) -> bool {
        let x = self.position[0] as f32 * scale + origin[0];
        let y = self.position[1] as f32 * scale + origin[1];
        x < viewport[0]
            && x + self.card_width() * scale > 0.
            && y < viewport[1]
            && y + self.card_height() * scale > 0.
    }

    const LABEL_HEIGHT: f32 = 44.;

    pub fn card_width(&self) -> f32 {
        (f32::from(self.width) + 2. * self.profile.shell().insets()[0]).max(136.)
    }

    pub fn card_height(&self) -> f32 {
        let [_, top, bottom] = self.profile.shell().insets();
        f32::from(self.height) + top + bottom + Self::LABEL_HEIGHT
    }
}

impl Default for VisualFrame {
    fn default() -> Self {
        Self::new(1, Profile::default())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasSettings {
    pub mode: EditorMode,
    pub frames: Vec<VisualFrame>,
    pub active_frame: u64,
    pub interact: bool,
    pub zoom: f32,
    pub fit_to_view: bool,
    pub pan: [f32; 2],
    next_id: u64,
}

impl Default for CanvasSettings {
    fn default() -> Self {
        Self::new(EditorMode::Design, Profile::default())
    }
}

impl CanvasSettings {
    pub fn new(mode: EditorMode, profile: Profile) -> Self {
        Self {
            mode,
            frames: vec![VisualFrame::new(1, profile)],
            active_frame: 1,
            interact: false,
            zoom: 100.,
            fit_to_view: true,
            pan: [0., 0.],
            next_id: 2,
        }
    }

    pub fn add(&mut self, profile: Profile, duplicate: Option<VisualFrame>) {
        if self.frames.len() >= 12 {
            return;
        }
        let id = self.next_id;
        let Some(next_id) = id.checked_add(1) else {
            return;
        };
        self.next_id = next_id;
        let mut frame = duplicate.map_or_else(
            || VisualFrame::new(id, profile),
            |mut frame| {
                frame.id = id;
                frame.name.push_str(" copy");
                frame
            },
        );
        if let Some(right) = self
            .frames
            .iter()
            .map(|frame| frame.position[0] + frame.card_width() as i32)
            .max()
        {
            frame.position[0] = right + 36;
        }
        self.frames.push(frame);
        self.active_frame = id;
    }

    fn bounds(&self, scale: f32) -> ([f32; 2], [f32; 2]) {
        let mut frames = self.frames.iter();
        let Some(first) = frames.next() else {
            return ([0.; 2], [0.; 2]);
        };
        let origin = first.position.map(|coordinate| coordinate as f32 * scale);
        let end = [
            origin[0] + first.card_width() * scale,
            origin[1] + first.card_height() * scale,
        ];
        let (min, max) = frames.fold((origin, end), |(min, max), frame| {
            let [x, y] = frame.position.map(|coordinate| coordinate as f32 * scale);
            (
                [min[0].min(x), min[1].min(y)],
                [
                    max[0].max(x + frame.card_width() * scale),
                    max[1].max(y + frame.card_height() * scale),
                ],
            )
        });
        (min.map(|axis| axis - 18.), max.map(|axis| axis + 18.))
    }

    pub fn extents(&self, scale: f32) -> [f32; 2] {
        let (min, max) = self.bounds(scale);
        [max[0] - min[0], max[1] - min[1]]
    }

    pub fn origin(&self, viewport: [f32; 2], scale: f32) -> [f32; 2] {
        if !self.fit_to_view {
            return self.pan.map(|axis| -axis);
        }
        let (min, max) = self.bounds(scale);
        std::array::from_fn(|axis| {
            -min[axis] + ((viewport[axis] - (max[axis] - min[axis])) / 2.).max(0.)
        })
    }

    pub fn visible_zoom(&self, viewport: [f32; 2]) -> f32 {
        if !self.fit_to_view {
            return self.zoom;
        }
        let (mut low, mut high) = (10_u16, 100_u16);
        while low < high {
            let zoom = (low + high).div_ceil(2);
            let [width, height] = self.extents(f32::from(zoom) / 100.);
            if width <= viewport[0] && height <= viewport[1] {
                low = zoom;
            } else {
                high = zoom - 1;
            }
        }
        f32::from(low)
    }

    pub fn has_visible_frame(&self, viewport: [f32; 2]) -> bool {
        let scale = self.visible_zoom(viewport) / 100.;
        let origin = self.origin(viewport, scale);
        self.frames
            .iter()
            .any(|frame| frame.is_visible(viewport, scale, origin))
    }

    pub fn recover_view(&mut self, viewport: [f32; 2]) {
        self.fit(viewport);
        if !self.has_visible_frame(viewport) {
            self.center(viewport);
        }
    }

    pub fn fit(&mut self, viewport: [f32; 2]) {
        self.fit_to_view = true;
        self.zoom = self.visible_zoom(viewport);
        self.pan = [0., 0.];
    }

    pub fn begin_navigation(&mut self, viewport: [f32; 2]) {
        if self.fit_to_view {
            self.zoom = self.visible_zoom(viewport);
            self.pan = self.origin(viewport, self.zoom / 100.).map(|axis| -axis);
            self.fit_to_view = false;
        }
    }

    pub fn pan_by(&mut self, viewport: [f32; 2], delta: [f32; 2]) {
        if delta == [0., 0.] {
            return;
        }
        self.begin_navigation(viewport);
        for (axis, step) in self.pan.iter_mut().zip(delta) {
            *axis -= step;
        }
    }

    pub fn zoom_by(&mut self, viewport: [f32; 2], delta: i16) {
        let zoom = self.visible_zoom(viewport) + f32::from(delta);
        self.zoom_at(viewport, zoom, viewport.map(|axis| axis / 2.));
    }

    pub fn zoom_at(&mut self, viewport: [f32; 2], zoom: f32, anchor: [f32; 2]) {
        let zoom = zoom.clamp(10., 200.);
        if !zoom.is_finite() || zoom == self.visible_zoom(viewport) {
            return;
        }
        self.begin_navigation(viewport);
        let ratio = f32::from(zoom) / self.zoom;
        for (pan, anchor) in self.pan.iter_mut().zip(anchor) {
            *pan = (*pan + anchor) * ratio - anchor;
        }
        self.zoom = zoom;
    }

    pub fn center(&mut self, viewport: [f32; 2]) {
        self.begin_navigation(viewport);
        if let Some(frame) = self
            .frames
            .iter()
            .find(|frame| frame.id == self.active_frame)
        {
            let scale = self.zoom / 100.;
            self.pan = [
                frame.position[0] as f32 * scale + (frame.card_width() * scale - viewport[0]) / 2.,
                frame.position[1] as f32 * scale + (frame.card_height() * scale - viewport[1]) / 2.,
            ];
        }
    }

    pub(crate) fn sanitize_camera(&mut self) {
        self.zoom = self.zoom.clamp(10., 200.);
        for axis in &mut self.pan {
            *axis = axis.clamp(-150_000., 150_000.);
            if !axis.is_finite() {
                *axis = 0.;
            }
        }
    }

    pub fn sanitize(&mut self) {
        self.sanitize_camera();
        self.frames.truncate(12);
        let mut ids = std::collections::HashSet::new();
        self.frames.retain_mut(|frame| {
            frame.width = frame.width.clamp(120, 4096);
            frame.height = frame.height.clamp(120, 4096);
            frame.position = frame.position.map(|axis| axis.clamp(-70_000, 70_000));
            ids.insert(frame.id)
        });
        if !ids.contains(&self.active_frame) {
            self.active_frame = self.frames.first().map_or(0, |frame| frame.id);
        }
        if let Some(max) = ids.into_iter().max() {
            self.next_id = self.next_id.max(max.saturating_add(1));
        }
    }
}

#[derive(Clone, Debug)]
pub struct VisualTree {
    pub nodes: Vec<InspectedNode>,
    parents: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    source: Arc<str>,
    pub(crate) assets: BTreeMap<usize, String>,
    pub(crate) diagnostic: String,
    pub(crate) parameters: Vec<String>,
    pub(crate) linked_views: Arc<BTreeMap<String, Arc<VisualTree>>>,
}

impl VisualTree {
    pub fn parse(source: &str) -> Result<Self, String> {
        Ok(Self::from_nodes(source.into(), visual_nodes(source)?))
    }

    fn from_nodes(source: Arc<str>, nodes: Vec<InspectedNode>) -> Self {
        let mut parents = Vec::with_capacity(nodes.len());
        let mut ancestors: Vec<usize> = Vec::new();
        let mut children = vec![Vec::new(); nodes.len() + 1];
        for (index, node) in nodes.iter().enumerate() {
            while ancestors
                .last()
                .is_some_and(|&parent| nodes[parent].range.end <= node.range.start)
            {
                ancestors.pop();
            }
            let parent = ancestors.last().copied();
            parents.push(parent);
            children[parent.map_or(0, |index| index + 1)].push(index);
            ancestors.push(index);
        }
        Self {
            nodes,
            parents,
            children,
            source,
            assets: BTreeMap::new(),
            diagnostic: String::new(),
            parameters: Vec::new(),
            linked_views: Arc::default(),
        }
    }

    pub(crate) fn for_preview(
        source: &str,
        root: Option<&Path>,
    ) -> Result<Vec<PreviewDeclaration>, String> {
        let declarations = visual_declarations(source)?;
        if declarations.is_empty() {
            return Err("Design requires a top-level named view! declaration.".into());
        }
        let source: Arc<str> = source.into();
        let mut links = BTreeMap::new();
        let mut previews: Vec<_> = declarations
            .into_iter()
            .map(|declaration| PreviewDeclaration {
                range: declaration.range,
                marked: declaration.preview,
                fallback: None,
                tree: declaration.nodes.map(|nodes| {
                    let mut tree = Self::from_nodes(source.clone(), nodes);
                    tree.parameters = declaration.parameters;
                    tree.prepare_preview(root);
                    let tree = Arc::new(tree);
                    if let Some(name) = tree.declaration_name() {
                        links.insert(name.to_owned(), tree.clone());
                    }
                    tree
                }),
            })
            .collect();
        let links = Arc::new(links);
        for preview in &mut previews {
            if let Ok(view) = &mut preview.tree {
                let view = Arc::make_mut(view);
                view.linked_views = links.clone();
                view.diagnostic = render::diagnostics(view).join(" ");
            }
        }
        Ok(previews)
    }

    pub(crate) fn declaration_name(&self) -> Option<&str> {
        self.nodes.first()?.path.first().map(String::as_str)
    }

    fn prepare_preview(&mut self, root: Option<&Path>) {
        if let Some(root) = root {
            for (index, node) in self
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| node.component == "Image")
            {
                let Some(path) = node
                    .properties
                    .iter()
                    .find(|property| property.key == "arg:0")
                    .and_then(|property| syn::parse_str::<syn::LitStr>(&property.value).ok())
                    .map(|literal| root.join(literal.value()))
                    .and_then(|path| path.canonicalize().ok())
                    .filter(|path| path.starts_with(root) && path.is_file())
                else {
                    continue;
                };
                if let Some(path) = path.to_str() {
                    self.assets.insert(index, path.to_owned());
                }
            }
        }
    }

    pub(crate) fn same_structure(&self, other: &Self) -> bool {
        self.nodes.first().and_then(|node| node.path.first())
            == other.nodes.first().and_then(|node| node.path.first())
            && self.parents == other.parents
            && self.nodes.iter().zip(&other.nodes).all(|(left, right)| {
                left.component == right.component
                    && (left.component != "TextField"
                        || left.properties.first().map(|property| &property.value)
                            == right.properties.first().map(|property| &property.value))
            })
    }

    pub fn parent(&self, node: usize) -> Option<usize> {
        self.parents.get(node).copied().flatten()
    }

    pub fn selected(&self, cursor: usize) -> Option<usize> {
        self.nodes
            .iter()
            .rposition(|node| node.range.contains(&cursor))
    }

    pub fn children(&self, parent: Option<usize>) -> impl Iterator<Item = usize> + '_ {
        let slot = parent.and_then(|index| index.checked_add(1)).unwrap_or(0);
        self.children.get(slot).into_iter().flatten().copied()
    }

    pub fn edit(
        &self,
        source: &str,
        node: usize,
        edit: ComponentEdit,
    ) -> Result<(SourcePatch, usize), String> {
        if &*self.source != source {
            return Err("The selected component has changed.".into());
        }
        let selected = self.nodes.get(node).ok_or("Select a component first.")?;
        let (range, replacement, cursor) = match edit {
            ComponentEdit::Insert(component) => {
                let parent = self
                    .insertion_parent(node, omen::FlowPlacement::Into)
                    .ok_or("Select a supported flow container.")?;
                self.insert_at(source, parent, omen::FlowPlacement::Into, component)?
            }
            ComponentEdit::InsertAt {
                target,
                placement,
                component,
            } => self.insert_at(source, target, placement, component)?,
            ComponentEdit::ResizeFixed(dimensions) => {
                let (_, property) = placement::fixed_size(selected)
                    .ok_or("Set literal width and height in the Inspector before resizing.")?;
                let [width, height] = dimensions;
                let patch = omen_macro_core::inspector::set_property(
                    source,
                    selected,
                    &property.key,
                    &format!("{width}, {height}"),
                    false,
                )?;
                return validate_patch(
                    source,
                    patch.range,
                    patch.replacement,
                    selected.name_range.start,
                );
            }
            ComponentEdit::Duplicate => {
                self.parent(node)
                    .ok_or("Select a child component to duplicate.")?;
                if selected
                    .properties
                    .iter()
                    .any(|property| property.modifier_range.is_some() && property.name == "key")
                {
                    return Err(
                        "Duplicate keyed components in Code so each sibling can have its own key."
                            .into(),
                    );
                }
                let end = selected.range.end;
                let indent = indent_at(source, selected.range.start);
                let newline = if source.contains("\r\n") {
                    "\r\n"
                } else {
                    "\n"
                };
                (
                    end..end,
                    format!("{newline}{indent}{}", &source[selected.range.clone()]),
                    end + newline.len() + indent.len(),
                )
            }
            ComponentEdit::Delete => {
                let parent = self
                    .parent(node)
                    .ok_or("The root component must remain in the view.")?;
                (
                    omen_macro_core::inspector::removal_range(source, selected.range.clone()),
                    String::new(),
                    self.nodes[parent].name_range.start,
                )
            }
            ComponentEdit::Move(up) => {
                let parent = self
                    .parent(node)
                    .ok_or("The root component cannot be reordered.")?;
                let siblings = &self.children[parent + 1];
                let index = siblings
                    .iter()
                    .position(|&sibling| sibling == node)
                    .ok_or("Missing sibling.")?;
                let target = index
                    .checked_add_signed(if up { -1 } else { 1 })
                    .and_then(|index| siblings.get(index))
                    .ok_or("There is no adjacent component in that direction.")?;
                self.move_to(source, node, *target)?
            }
            ComponentEdit::MoveTo(target) => self.move_to(source, node, target)?,
            ComponentEdit::Place { target, placement } => {
                self.place(source, node, target, placement)?
            }
            ComponentEdit::TogglePin => self.pin(source, node, None)?,
            ComponentEdit::Wrap => self.wrap(source, node)?,
        };
        validate_patch(source, range, replacement, cursor)
    }
    fn move_to(
        &self,
        source: &str,
        node: usize,
        target: usize,
    ) -> Result<(Range<usize>, String, usize), String> {
        let parent = self
            .parent(node)
            .ok_or("The root component cannot be reordered.")?;
        if self.parent(target) != Some(parent) || node == target {
            return Err("Drop the component on a sibling in the same layout.".into());
        }
        let siblings = &self.children[parent + 1];
        let up = target < node;
        let mut components = siblings
            .iter()
            .copied()
            .filter(|&index| index >= node.min(target) && index <= node.max(target))
            .collect::<Vec<_>>();
        let first = &self.nodes[node.min(target)];
        let last = &self.nodes[node.max(target)];
        let gaps = components.windows(2).map(|pair| {
            let left = &self.nodes[pair[0]];
            let right = &self.nodes[pair[1]];
            let gap = &source[left.range.end..right.range.start];
            if gap.chars().all(char::is_whitespace) {
                Ok(gap)
            } else {
                Err("Move the component in Code to preserve the intervening comments or syntax.")
            }
        }).collect::<Result<Vec<_>, _>>()?;
        if up {
            components.rotate_right(1);
        } else {
            components.rotate_left(1);
        }
        let range = first.range.start..last.range.end;
        let mut replacement = String::with_capacity(range.len());
        let mut cursor = range.start;
        for (slot, index) in components.into_iter().enumerate() {
            if index == node {
                cursor = range.start + replacement.len();
            }
            replacement.push_str(&source[self.nodes[index].range.clone()]);
            if let Some(gap) = gaps.get(slot) {
                replacement.push_str(gap);
            }
        }
        Ok((range, replacement, cursor))
    }
}

fn indent_at(source: &str, offset: usize) -> &str {
    let prefix = &source[..offset];
    let line = prefix.rsplit('\n').next().unwrap_or_default();
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

fn validate_patch(
    source: &str,
    range: Range<usize>,
    replacement: String,
    cursor: usize,
) -> Result<(SourcePatch, usize), String> {
    let expected = source
        .get(range.clone())
        .ok_or("The component range is invalid.")?
        .to_owned();
    let mut updated = source.to_owned();
    updated.replace_range(range.clone(), &replacement);
    let declarations = visual_declarations(&updated)?;
    let declaration = declarations
        .into_iter()
        .find(|declaration| declaration.range.contains(&cursor))
        .ok_or("The edited view declaration could not be found.")?;
    declaration.nodes?;
    Ok((
        SourcePatch {
            range,
            expected,
            replacement,
        },
        cursor,
    ))
}

#[derive(Clone, Copy, Debug)]
pub enum ComponentEdit {
    Insert(Component),
    InsertAt {
        target: usize,
        placement: omen::FlowPlacement,
        component: Component,
    },
    ResizeFixed([f32; 2]),
    Duplicate,
    Delete,
    Move(bool),
    MoveTo(usize),
    Place {
        target: usize,
        placement: omen::FlowPlacement,
    },
    Wrap,
    TogglePin,
}

#[derive(Clone, Copy, Debug)]
pub enum Component {
    Layout,
    AdaptiveLayout,
    AdaptiveGrid,
    Text,
    Button,
    Image,
    TextField,
    Gap,
    Scroll,
}

impl Component {
    pub const ALL: [Self; 9] = [
        Self::Layout,
        Self::AdaptiveLayout,
        Self::AdaptiveGrid,
        Self::Text,
        Self::Button,
        Self::Image,
        Self::TextField,
        Self::Gap,
        Self::Scroll,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Layout => "Layout",
            Self::AdaptiveLayout => "Adaptive layout",
            Self::AdaptiveGrid => "Adaptive grid",
            Self::Text => "Text",
            Self::Button => "Button",
            Self::Image => "Image",
            Self::TextField => "Text field",
            Self::Gap => "Gap",
            Self::Scroll => "Scroll layout",
        }
    }
    fn syntax(self) -> &'static str {
        match self {
            Self::Layout => "Layout {}",
            Self::AdaptiveLayout => "AdaptiveLayout {}",
            Self::AdaptiveGrid => "AdaptiveGrid {}.min_item_width(240).spacing(16)",
            Self::Text => r#"Text("Text")"#,
            Self::Button => r#"Button("Button") {}"#,
            Self::Image => r#"Image("assets/image.png")"#,
            Self::TextField => r#"TextField("Text field")"#,
            Self::Gap => "Gap(8)",
            Self::Scroll => "Layout {}.scrollable()",
        }
    }
}
