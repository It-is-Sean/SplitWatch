use ratatui::layout::{Constraint, Direction, Layout, Rect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const DEFAULT_SPLIT_RATIO: u16 = 50;
const MIN_SPLIT_RATIO: u16 = 20;
const MAX_SPLIT_RATIO: u16 = 80;
pub const MIN_PANE_WIDTH: u16 = 24;
pub const MIN_PANE_HEIGHT: u16 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitAxis {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayoutNode {
    Leaf {
        pane: usize,
    },
    Split {
        axis: SplitAxis,
        #[serde(default = "default_split_ratio")]
        ratio_percent: u16,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PresetLayout {
    Named(String),
    Tree(LayoutNode),
}

impl PresetLayout {
    pub fn into_tree(self, pane_count: usize) -> Option<LayoutNode> {
        match self {
            PresetLayout::Named(name) => match name.as_str() {
                "grid" | "main-right-stack" => Some(default_layout_tree(pane_count)),
                _ => None,
            },
            PresetLayout::Tree(tree) => Some(tree),
        }
    }

    pub fn from_tree(tree: LayoutNode) -> Self {
        PresetLayout::Tree(tree)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    pub rows: usize,
    pub cols: usize,
}

pub fn grid_for_count(count: usize) -> Grid {
    match count {
        0 | 1 => Grid { rows: 1, cols: 1 },
        2 => Grid { rows: 1, cols: 2 },
        3 | 4 => Grid { rows: 2, cols: 2 },
        5 | 6 => Grid { rows: 2, cols: 3 },
        n => {
            let cols = (n as f64).sqrt().ceil() as usize;
            let rows = n.div_ceil(cols);
            Grid { rows, cols }
        }
    }
}

pub fn default_layout_tree(count: usize) -> LayoutNode {
    match count.max(1) {
        1 => LayoutNode::Leaf { pane: 0 },
        2 => split(
            SplitAxis::Vertical,
            LayoutNode::Leaf { pane: 0 },
            LayoutNode::Leaf { pane: 1 },
        ),
        3 => split(
            SplitAxis::Vertical,
            LayoutNode::Leaf { pane: 0 },
            split(
                SplitAxis::Horizontal,
                LayoutNode::Leaf { pane: 1 },
                LayoutNode::Leaf { pane: 2 },
            ),
        ),
        n => balanced_tree(0, n),
    }
}

fn balanced_tree(start: usize, count: usize) -> LayoutNode {
    if count <= 1 {
        return LayoutNode::Leaf { pane: start };
    }
    let left_count = count / 2;
    let right_count = count - left_count;
    split(
        SplitAxis::Horizontal,
        balanced_tree(start, left_count),
        balanced_tree(start + left_count, right_count),
    )
}

fn split(axis: SplitAxis, first: LayoutNode, second: LayoutNode) -> LayoutNode {
    LayoutNode::Split {
        axis,
        ratio_percent: default_split_ratio(),
        first: Box::new(first),
        second: Box::new(second),
    }
}

const fn default_split_ratio() -> u16 {
    DEFAULT_SPLIT_RATIO
}

pub fn pane_rects(area: Rect, layout: &LayoutNode) -> Vec<(usize, Rect)> {
    let mut rects = Vec::new();
    collect_rects(area, layout, &mut rects);
    rects.sort_by_key(|(idx, _)| *idx);
    rects
}

pub fn can_split_rect(rect: Rect, axis: SplitAxis) -> bool {
    rect.width >= MIN_PANE_WIDTH
        && rect.height >= MIN_PANE_HEIGHT
        && match axis {
            SplitAxis::Vertical => rect.width >= MIN_PANE_WIDTH.saturating_mul(2),
            SplitAxis::Horizontal => rect.height >= MIN_PANE_HEIGHT.saturating_mul(2),
        }
}

pub fn layout_meets_min_pane_size(area: Rect, layout: &LayoutNode) -> bool {
    pane_rects(area, layout)
        .into_iter()
        .all(|(_, rect)| rect.width >= MIN_PANE_WIDTH && rect.height >= MIN_PANE_HEIGHT)
}

fn collect_rects(area: Rect, layout: &LayoutNode, rects: &mut Vec<(usize, Rect)>) {
    match layout {
        LayoutNode::Leaf { pane } => rects.push((*pane, area)),
        LayoutNode::Split {
            axis,
            ratio_percent,
            first,
            second,
        } => {
            let direction = match axis {
                SplitAxis::Vertical => Direction::Horizontal,
                SplitAxis::Horizontal => Direction::Vertical,
            };
            let first_percent = (*ratio_percent).clamp(MIN_SPLIT_RATIO, MAX_SPLIT_RATIO);
            let second_percent = 100u16.saturating_sub(first_percent);
            let chunks = Layout::default()
                .direction(direction)
                .constraints([
                    Constraint::Percentage(first_percent),
                    Constraint::Percentage(second_percent),
                ])
                .split(area);
            collect_rects(chunks[0], first, rects);
            collect_rects(chunks[1], second, rects);
        }
    }
}

pub fn split_pane(
    layout: &mut LayoutNode,
    target: usize,
    axis: SplitAxis,
    new_pane: usize,
) -> bool {
    match layout {
        LayoutNode::Leaf { pane } if *pane == target => {
            *layout = split(
                axis,
                LayoutNode::Leaf { pane: target },
                LayoutNode::Leaf { pane: new_pane },
            );
            true
        }
        LayoutNode::Leaf { .. } => false,
        LayoutNode::Split { first, second, .. } => {
            split_pane(first, target, axis, new_pane) || split_pane(second, target, axis, new_pane)
        }
    }
}

pub fn adjust_split_ratio_for_pane(
    layout: &mut LayoutNode,
    target: usize,
    axis: SplitAxis,
    delta: i16,
) -> bool {
    match layout {
        LayoutNode::Leaf { .. } => false,
        LayoutNode::Split {
            axis: split_axis,
            ratio_percent,
            first,
            second,
        } => {
            let first_has = contains_pane(first, target);
            let second_has = contains_pane(second, target);

            if *split_axis == axis && (first_has || second_has) {
                let signed_delta = if first_has { delta } else { -delta };
                let next = (*ratio_percent as i16 + signed_delta)
                    .clamp(MIN_SPLIT_RATIO as i16, MAX_SPLIT_RATIO as i16)
                    as u16;
                let changed = next != *ratio_percent;
                *ratio_percent = next;
                return changed;
            }

            if first_has && adjust_split_ratio_for_pane(first, target, axis, delta) {
                return true;
            }
            if second_has && adjust_split_ratio_for_pane(second, target, axis, delta) {
                return true;
            }
            false
        }
    }
}

pub fn shift_pane_indices(layout: &mut LayoutNode, from: usize, delta: isize) {
    match layout {
        LayoutNode::Leaf { pane } => {
            if *pane >= from {
                *pane = ((*pane as isize) + delta) as usize;
            }
        }
        LayoutNode::Split { first, second, .. } => {
            shift_pane_indices(first, from, delta);
            shift_pane_indices(second, from, delta);
        }
    }
}

pub fn remove_pane(layout: LayoutNode, target: usize) -> Option<LayoutNode> {
    match layout {
        LayoutNode::Leaf { pane } => {
            if pane == target {
                None
            } else {
                Some(LayoutNode::Leaf { pane })
            }
        }
        LayoutNode::Split {
            axis,
            ratio_percent,
            first,
            second,
        } => {
            let first = remove_pane(*first, target);
            let second = remove_pane(*second, target);
            match (first, second) {
                (Some(first), Some(second)) => Some(LayoutNode::Split {
                    axis,
                    ratio_percent,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(node), None) | (None, Some(node)) => Some(node),
                (None, None) => None,
            }
        }
    }
}

pub fn contains_pane(layout: &LayoutNode, target: usize) -> bool {
    match layout {
        LayoutNode::Leaf { pane } => *pane == target,
        LayoutNode::Split { first, second, .. } => {
            contains_pane(first, target) || contains_pane(second, target)
        }
    }
}

pub fn validate_layout_tree(layout: &LayoutNode, pane_count: usize) -> bool {
    let mut panes = Vec::new();
    collect_panes(layout, &mut panes);
    if panes.len() != pane_count {
        return false;
    }
    let unique = panes.iter().copied().collect::<BTreeSet<_>>();
    unique.len() == pane_count && unique.iter().copied().eq(0..pane_count)
}

fn collect_panes(layout: &LayoutNode, panes: &mut Vec<usize>) {
    match layout {
        LayoutNode::Leaf { pane } => panes.push(*pane),
        LayoutNode::Split { first, second, .. } => {
            collect_panes(first, panes);
            collect_panes(second, panes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Grid, LayoutNode, MIN_PANE_HEIGHT, MIN_PANE_WIDTH, SplitAxis, adjust_split_ratio_for_pane,
        can_split_rect, default_layout_tree, grid_for_count, layout_meets_min_pane_size,
        pane_rects, remove_pane,
    };
    use ratatui::layout::Rect;

    #[test]
    fn uses_expected_fixed_layouts() {
        assert_eq!(grid_for_count(1), Grid { rows: 1, cols: 1 });
        assert_eq!(grid_for_count(2), Grid { rows: 1, cols: 2 });
        assert_eq!(grid_for_count(3), Grid { rows: 2, cols: 2 });
        assert_eq!(grid_for_count(4), Grid { rows: 2, cols: 2 });
        assert_eq!(grid_for_count(5), Grid { rows: 2, cols: 3 });
        assert_eq!(grid_for_count(6), Grid { rows: 2, cols: 3 });
    }

    #[test]
    fn uses_roughly_square_layout_for_larger_counts() {
        assert_eq!(grid_for_count(7), Grid { rows: 3, cols: 3 });
        assert_eq!(grid_for_count(10), Grid { rows: 3, cols: 4 });
    }

    #[test]
    fn builds_three_pane_main_right_stack() {
        let layout = default_layout_tree(3);
        match layout {
            LayoutNode::Split {
                axis: SplitAxis::Vertical,
                ratio_percent: 50,
                ..
            } => {}
            _ => panic!("expected vertical split root for 3-pane layout"),
        }
    }

    #[test]
    fn collapses_tree_on_remove() {
        let layout = LayoutNode::Split {
            axis: SplitAxis::Vertical,
            ratio_percent: 50,
            first: Box::new(LayoutNode::Leaf { pane: 0 }),
            second: Box::new(LayoutNode::Leaf { pane: 1 }),
        };
        let collapsed = remove_pane(layout, 1).unwrap();
        assert_eq!(collapsed, LayoutNode::Leaf { pane: 0 });
    }

    #[test]
    fn computes_rects() {
        let rects = pane_rects(Rect::new(0, 0, 100, 40), &default_layout_tree(2));
        assert_eq!(rects.len(), 2);
    }

    #[test]
    fn respects_split_ratio_in_rects() {
        let layout = LayoutNode::Split {
            axis: SplitAxis::Vertical,
            ratio_percent: 70,
            first: Box::new(LayoutNode::Leaf { pane: 0 }),
            second: Box::new(LayoutNode::Leaf { pane: 1 }),
        };
        let rects = pane_rects(Rect::new(0, 0, 100, 20), &layout);
        assert_eq!(rects[0].1.width, 70);
        assert_eq!(rects[1].1.width, 30);
    }

    #[test]
    fn adjusts_nearest_matching_split_for_focused_pane() {
        let mut layout = default_layout_tree(3);
        assert!(adjust_split_ratio_for_pane(
            &mut layout,
            0,
            SplitAxis::Vertical,
            5
        ));
        match layout {
            LayoutNode::Split { ratio_percent, .. } => assert_eq!(ratio_percent, 55),
            _ => panic!("expected split root"),
        }
    }

    #[test]
    fn blocks_split_when_rect_is_below_minimum() {
        assert!(!can_split_rect(
            Rect::new(0, 0, MIN_PANE_WIDTH * 2 - 1, MIN_PANE_HEIGHT),
            SplitAxis::Vertical
        ));
        assert!(!can_split_rect(
            Rect::new(0, 0, MIN_PANE_WIDTH, MIN_PANE_HEIGHT * 2 - 1),
            SplitAxis::Horizontal
        ));
    }

    #[test]
    fn detects_layouts_that_violate_minimum_pane_size() {
        let layout = LayoutNode::Split {
            axis: SplitAxis::Vertical,
            ratio_percent: 80,
            first: Box::new(LayoutNode::Leaf { pane: 0 }),
            second: Box::new(LayoutNode::Leaf { pane: 1 }),
        };
        assert!(!layout_meets_min_pane_size(
            Rect::new(0, 0, MIN_PANE_WIDTH * 2, MIN_PANE_HEIGHT),
            &layout
        ));
    }
}
