//! Resolves versioned placement intents into a single contiguous source patch.
use super::{Component, VisualTree, indent_at};
use omen::FlowPlacement;
use omen_macro_core::inspector::{InspectedNode, Property, adaptive_direction, removal_range};
use std::ops::Range;

impl VisualTree {
    pub(crate) fn flow_container(&self, index: usize) -> bool {
        self.nodes.get(index).is_some_and(|node| {
            matches!(
                node.component.as_str(),
                "Layout" | "AdaptiveLayout" | "AdaptiveGrid" | "Card"
            ) && node.children_end.is_some()
                && !node.properties.iter().any(|property| {
                    property.name == "stack"
                        || property.name == "mode"
                            && !(matches!(property.value.as_str(), ".horizontal" | ".vertical")
                                || adaptive_direction(&property.value).is_some())
                        || property.name == "responsive"
                })
        })
    }

    pub(crate) fn insertion_parent(
        &self,
        target: usize,
        placement: FlowPlacement,
    ) -> Option<usize> {
        let parent = match placement {
            FlowPlacement::Into if self.flow_container(target) => Some(target),
            FlowPlacement::Into | FlowPlacement::Before | FlowPlacement::After => {
                self.parent(target)
            }
            FlowPlacement::Pinned(_) => None,
        }?;
        self.flow_container(parent).then_some(parent)
    }

    pub(crate) fn insert_at(
        &self,
        source: &str,
        target: usize,
        placement: FlowPlacement,
        component: Component,
    ) -> Result<(Range<usize>, String, usize), String> {
        let parent = match placement {
            FlowPlacement::Into => Some(target),
            FlowPlacement::Pinned(_) => return Err("Select an existing pinned component.".into()),
            _ => self.parent(target),
        };
        if !parent.is_some_and(|parent| self.flow_container(parent)) {
            return Err("Select a supported flow container or one of its children.".into());
        }
        let (range, insert, offset) =
            self.insertion(source, target, placement, component.syntax())?;
        let cursor = range.start + offset;
        Ok((range, insert, cursor))
    }

    fn insertion(
        &self,
        source: &str,
        target: usize,
        placement: FlowPlacement,
        component: &str,
    ) -> Result<(Range<usize>, String, usize), String> {
        let node = self
            .nodes
            .get(target)
            .ok_or("Drop target is unavailable.")?;
        let newline = if source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let indent = indent_at(source, node.range.start);
        if placement == FlowPlacement::Before && leading_comment(source, node.range.start)
            || placement == FlowPlacement::After && trailing_comment(source, node.range.end)
        {
            return Err("Choose a different slot to keep the target’s comment attached.".into());
        }
        Ok(match placement {
            FlowPlacement::Before => (
                node.range.start..node.range.start,
                format!("{component}{newline}{indent}"),
                0,
            ),
            FlowPlacement::After => (
                node.range.end..node.range.end,
                format!("{newline}{indent}{component}"),
                newline.len() + indent.len(),
            ),
            FlowPlacement::Pinned(_) => return Err("Select an existing pinned component.".into()),
            FlowPlacement::Into => {
                let point = node.children_end.ok_or("The layout has no child block.")?;
                let start = source[..point]
                    .trim_end_matches([' ', '\t', '\r', '\n'])
                    .len();
                let child_indent = self
                    .children(Some(target))
                    .map(|child| self.nodes[child].range.start)
                    .find_map(|offset| {
                        let line = source[..offset].rsplit('\n').next()?;
                        (line == indent_at(source, offset) && line.len() > indent.len())
                            .then_some(line)
                    })
                    .map_or_else(
                        || {
                            format!(
                                "{indent}{}",
                                if indent.ends_with('\t') { "\t" } else { "    " }
                            )
                        },
                        str::to_owned,
                    );
                (
                    start..point,
                    format!("{newline}{child_indent}{component}{newline}{indent}"),
                    newline.len() + child_indent.len(),
                )
            }
        })
    }

    pub(crate) fn place(
        &self,
        source: &str,
        child: usize,
        target: usize,
        placement: FlowPlacement,
    ) -> Result<(Range<usize>, String, usize), String> {
        self.nodes
            .get(target)
            .ok_or("Drop target is unavailable.")?;
        let node = &self.nodes[child];
        let old_parent = self
            .parent(child)
            .ok_or("The root component cannot be moved.")?;
        let parent = match placement {
            FlowPlacement::Into => target,
            FlowPlacement::Pinned(pin) => return self.pin(source, child, Some((target, pin))),
            _ => self
                .parent(target)
                .ok_or("Drop next to a child inside a flow layout.")?,
        };
        if child == target || node.range.contains(&self.nodes[parent].range.start) {
            return Err("A component cannot be moved into itself or its descendants.".into());
        }
        if !self.flow_container(old_parent) || !self.flow_container(parent) {
            return Err("Drag components between supported flow layouts or cards.".into());
        }

        if old_parent == parent {
            let siblings = &self.children[parent + 1];
            let child_slot = siblings
                .iter()
                .position(|&id| id == child)
                .ok_or("Missing child.")?;
            let index = match placement {
                FlowPlacement::Into => siblings.len(),
                _ => {
                    let target_slot = siblings
                        .iter()
                        .position(|&id| id == target)
                        .ok_or("Missing target.")?;
                    target_slot + usize::from(placement == FlowPlacement::After)
                }
            };
            if index == child_slot || index == child_slot + 1 {
                return Err("The component is already in that position.".into());
            }
            let destination = siblings[if index > child_slot { index - 1 } else { index }];
            return self.move_to(source, child, destination);
        }

        if leading_comment(source, node.range.start) || trailing_comment(source, node.range.end) {
            return Err("Move this component in Code to keep its comments attached.".into());
        }
        if let Some(key) = node
            .properties
            .iter()
            .find(|property| property.name == "key")
            && self.children(Some(parent)).any(|sibling| {
                self.nodes[sibling]
                    .properties
                    .iter()
                    .any(|property| property.name == "key" && property.value == key.value)
            })
        {
            return Err("The destination already contains a child with this key.".into());
        }
        let (destination, inserted, cursor_offset) =
            self.insertion(source, target, placement, &source[node.range.clone()])?;
        let removal = removal_range(source, node.range.clone());
        if destination.end <= node.range.start {
            let removal_start = removal.start.max(destination.end);
            let range = destination.start..removal.end;
            let cursor = range.start + cursor_offset;
            let replacement = format!("{inserted}{}", &source[destination.end..removal_start]);
            Ok((range, replacement, cursor))
        } else {
            let removal_end = removal.end.min(destination.start);
            let range = removal.start..destination.end;
            let between = &source[removal_end..destination.start];
            let cursor = range.start + between.len() + cursor_offset;
            Ok((range, format!("{between}{inserted}"), cursor))
        }
    }

    pub(crate) fn pin(
        &self,
        source: &str,
        child: usize,
        drop: Option<(usize, omen::Pin)>,
    ) -> Result<(Range<usize>, String, usize), String> {
        let parent = self
            .parent(child)
            .ok_or("Select a child in a Layout to pin it.")?;
        let container = &self.nodes[parent];
        if container.component != "Layout" || container.children_end.is_none() {
            return Err("Pinning requires a parent Layout.".into());
        }
        let node = &self.nodes[child];
        let cursor = node.name_range.start;
        let existing = node
            .properties
            .iter()
            .find(|property| property.name == "pin");
        if let Some((target, pin)) = drop {
            if target != parent {
                return Err("Drop inside the pinned component’s parent Layout.".into());
            }
            let property = existing.ok_or("Pin this component before dragging it freely.")?;
            let (trailing, bottom) = pin.corner.edges();
            let corner = match (trailing, bottom) {
                (false, false) => ".topLeading",
                (true, false) => ".topTrailing",
                (false, true) => ".bottomLeading",
                (true, true) => ".bottomTrailing",
            };
            let patch = omen_macro_core::inspector::set_property(
                source,
                node,
                &property.key,
                &format!("{corner}, {}, {}", pin.x, pin.y),
                false,
            )?;
            return Ok((patch.range, patch.replacement, cursor));
        }
        if let Some(property) = existing {
            let patch = omen_macro_core::inspector::remove_modifier(source, node, &property.key)?;
            return Ok((patch.range, patch.replacement, cursor));
        }

        let needs_size = !container
            .properties
            .iter()
            .any(|property| matches!(property.name.as_str(), "frame" | "fill"));
        if needs_size && self.parent(parent).is_some() {
            return Err("Set a size on the parent Layout before pinning a child inside it.".into());
        }
        let patch = omen_macro_core::inspector::add_modifier(source, node, "pin")?;
        if needs_size {
            let end = container.range.end;
            let between = &source[patch.range.end..end];
            Ok((
                patch.range.start..end,
                format!("{}{between}.frame(.infinity, .infinity)", patch.replacement),
                cursor,
            ))
        } else {
            Ok((patch.range, patch.replacement, cursor))
        }
    }

    pub(crate) fn wrap(
        &self,
        source: &str,
        child: usize,
    ) -> Result<(Range<usize>, String, usize), String> {
        let parent = self.parent(child).ok_or("Select a child to wrap.")?;
        if !self.flow_container(parent) {
            return Err("Only a child of a supported flow container can be wrapped.".into());
        }
        let range = self.nodes[child].range.clone();
        let newline = if source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let indent = indent_at(source, range.start);
        let result = format!(
            "Layout {{{newline}{indent}    {}{newline}{indent}}}",
            &source[range.clone()]
        );
        let cursor = range.start;
        Ok((range, result, cursor))
    }
}

fn leading_comment(source: &str, start: usize) -> bool {
    let line = source[..start]
        .rsplit('\n')
        .nth(1)
        .unwrap_or_default()
        .trim();
    line.starts_with("//") || line.ends_with("*/")
}

fn trailing_comment(source: &str, end: usize) -> bool {
    let line = source[end..]
        .split('\n')
        .next()
        .unwrap_or_default()
        .trim_start();
    line.starts_with("//") || line.starts_with("/*")
}

pub(crate) fn fixed_size(node: &InspectedNode) -> Option<([f32; 2], &Property)> {
    if node
        .properties
        .iter()
        .any(|property| matches!(property.name.as_str(), "fill" | "responsive"))
    {
        return None;
    }
    let property = node
        .properties
        .iter()
        .rev()
        .find(|property| property.name == "frame")?;
    let (width, height) = property.value.trim_end_matches(',').split_once(',')?;
    let number = |value: &str| {
        value
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|value| (1. ..=4096.).contains(value))
    };
    Some(([number(width)?, number(height)?], property))
}
