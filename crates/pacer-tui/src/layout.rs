//! The body mosaic: how the Projects, Worktrees and Sessions panels and the
//! terminal pane tile the space under the Workspaces bar.
//!
//! The arrangement is a binary split tree whose leaves are the four tiles.
//! Every split stacks its two children side by side ([`Dir::Row`]) or one
//! above the other ([`Dir::Col`]) with a one-cell rule between them, and
//! remembers one number: the extent of its *fixed* child, rule included —
//! the side that does not contain the terminal (the first child when
//! neither does), so a column's "width" is what it always was. The
//! terminal side absorbs whatever is left, so resizing the screen or hiding
//! a panel never shifts a sidebar the user sized by hand.
//!
//! Callers see three verbs: [`PanelLayout::resolve`] turns the tree into
//! screen rects plus the draggable boundaries between them, given which
//! panels are shown; [`PanelLayout::move_panel`] re-homes a panel beside its
//! neighbour in a direction (or onto the body's edge when it has none);
//! [`PanelLayout::set_boundary`] is the splitter drag. Hidden panels stay in
//! the tree so their place and size survive a toggle: a split with one
//! hidden side collapses to the other at resolve time.
//!
//! Split ids are pre-order positions in the full tree, so the default
//! three-column layout numbers its boundaries 0, 1, 2 left to right — the
//! index the splitter hit-tests and grips have always used.

use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

use crate::app::{MIN_PANEL_W, MIN_TERM_W};

/// A panel can't be stacked shorter than this (rule included, like
/// `MIN_PANEL_W`): the header's blank spacer, title row and gap, two list
/// rows, and the rule under it.
pub const MIN_PANEL_H: u16 = 6;
/// The terminal pane always keeps at least this many rows.
pub const MIN_TERM_H: u16 = 5;
/// Height a panel gets when it first lands above or below something after
/// living in a full-height column, where its old height means nothing.
const DEFAULT_STACK_H: u16 = 12;

/// One tile of the mosaic. Panels are the logical sidebar indices the rest
/// of the TUI uses: 0 Projects, 1 Worktrees, 2 Sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Leaf {
    Panel(usize),
    Terminal,
}

/// How a split lays its children out: `Row` side by side (a vertical rule
/// between them), `Col` stacked (a horizontal rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dir {
    Row,
    Col,
}

/// Where a moved panel lands relative to its neighbour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
    Above,
    Below,
}

impl Side {
    fn dir(self) -> Dir {
        match self {
            Side::Left | Side::Right => Dir::Row,
            Side::Above | Side::Below => Dir::Col,
        }
    }

    /// The moved panel comes first (left of / above the neighbour).
    fn first(self) -> bool {
        matches!(self, Side::Left | Side::Above)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Node {
    Leaf(Leaf),
    Split {
        dir: Dir,
        /// Extent of the fixed child along `dir`, in cells, counting the
        /// rule between the two children.
        size: u16,
        first: Box<Node>,
        second: Box<Node>,
    },
}

impl Node {
    fn split(dir: Dir, size: u16, first: Node, second: Node) -> Node {
        Node::Split {
            dir,
            size,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    fn contains(&self, leaf: Leaf) -> bool {
        match self {
            Node::Leaf(l) => *l == leaf,
            Node::Split { first, second, .. } => first.contains(leaf) || second.contains(leaf),
        }
    }

    /// Any leaf of this subtree is on screen.
    fn shown(&self, visible: [bool; 3]) -> bool {
        match self {
            Node::Leaf(Leaf::Terminal) => true,
            Node::Leaf(Leaf::Panel(p)) => visible.get(*p).copied().unwrap_or(false),
            Node::Split { first, second, .. } => first.shown(visible) || second.shown(visible),
        }
    }

    fn split_count(&self) -> usize {
        match self {
            Node::Leaf(_) => 0,
            Node::Split { first, second, .. } => 1 + first.split_count() + second.split_count(),
        }
    }

    /// The first child is the one whose size the split remembers.
    fn fixed_first(&self) -> bool {
        match self {
            Node::Split { first, .. } => !first.contains(Leaf::Terminal),
            Node::Leaf(_) => true,
        }
    }

    /// Smallest extent this subtree can be squeezed to along `dir` without
    /// breaking a tile's minimum. Rules are not counted: a panel's minimum
    /// size names the rule, so its content may be one cell less.
    fn min_extent(&self, dir: Dir, visible: [bool; 3]) -> u16 {
        match self {
            Node::Leaf(Leaf::Terminal) => match dir {
                Dir::Row => MIN_TERM_W,
                Dir::Col => MIN_TERM_H,
            },
            Node::Leaf(Leaf::Panel(p)) => {
                if !visible.get(*p).copied().unwrap_or(false) {
                    0
                } else {
                    match dir {
                        Dir::Row => MIN_PANEL_W - 1,
                        Dir::Col => MIN_PANEL_H - 1,
                    }
                }
            }
            Node::Split {
                dir: d,
                first,
                second,
                ..
            } => {
                let a = first.min_extent(dir, visible);
                let b = second.min_extent(dir, visible);
                if *d == dir && first.shown(visible) && second.shown(visible) {
                    a + 1 + b
                } else {
                    a.max(b)
                }
            }
        }
    }

    /// Drop `leaf` from the tree; its parent split collapses to the sibling.
    fn remove(self, leaf: Leaf) -> Node {
        match self {
            Node::Split {
                dir,
                size,
                first,
                second,
            } => {
                if matches!(*first, Node::Leaf(l) if l == leaf) {
                    *second
                } else if matches!(*second, Node::Leaf(l) if l == leaf) {
                    *first
                } else {
                    Node::split(dir, size, first.remove(leaf), second.remove(leaf))
                }
            }
            leaf_node => leaf_node,
        }
    }

    /// Put `new` on `side` of the `target` leaf, `size` cells (rule
    /// included) along that axis, `room` being the target's current extent.
    ///
    /// When the target already sits in a split running the same way, `new`
    /// joins that run as the target's neighbour instead of nesting inside
    /// the target's cell — so walking a column past its neighbours swaps
    /// places with them and every column keeps the width it had. Otherwise
    /// the target's cell is what gets divided.
    fn insert_beside(self, target: Leaf, side: Side, new: Leaf, size: u16, room: u16) -> Node {
        // `new_first`: `new` goes before the target along the axis. Joining
        // a run, it lands past the target (a swap); entering a cell across
        // the other axis, it stops on the near side — moving down into a
        // column lands on top of that column, and only the next step down
        // walks past.
        let pair = |target: Node, room: u16, new_first: bool| -> Node {
            // Dividing the target's cell: leave it its minimum.
            let keep = match (side.dir(), target.contains(Leaf::Terminal)) {
                (Dir::Row, true) => MIN_TERM_W,
                (Dir::Col, true) => MIN_TERM_H,
                (Dir::Row, false) => MIN_PANEL_W - 1,
                (Dir::Col, false) => MIN_PANEL_H - 1,
            };
            let size = size.min(room.saturating_sub(keep)).max(1);
            let (first, second) = if new_first {
                (Node::Leaf(new), target)
            } else {
                (target, Node::Leaf(new))
            };
            let fixed_first = !first.contains(Leaf::Terminal);
            // Whichever side the split remembers, `new` ends up `size`.
            let new_first = matches!(first, Node::Leaf(l) if l == new);
            let s = if fixed_first == new_first {
                size
            } else {
                room.saturating_sub(size)
            };
            Node::split(side.dir(), s, first, second)
        };
        match self {
            Node::Leaf(l) if l == target => pair(Node::Leaf(target), room, !side.first()),
            Node::Leaf(l) => Node::Leaf(l),
            Node::Split {
                dir,
                size: s,
                first,
                second,
            } if dir == side.dir()
                && (matches!(*first, Node::Leaf(l) if l == target)
                    || matches!(*second, Node::Leaf(l) if l == target)) =>
            {
                let target_first = matches!(*first, Node::Leaf(l) if l == target);
                match (target_first, side.first()) {
                    // [target | rest] with new before target: new leads
                    // the run, target keeps its split.
                    (true, true) => Node::split(
                        dir,
                        size,
                        Node::Leaf(new),
                        Node::split(dir, s, *first, *second),
                    ),
                    // [target | rest] with new after target: new takes
                    // the front of `rest`.
                    (true, false) => {
                        let rest_fixed = !second.contains(Leaf::Terminal);
                        let inner = Node::split(dir, size, Node::Leaf(new), *second);
                        // `rest` was this split's stretchy side; if it was
                        // the fixed side instead, keep its extent.
                        let s = if rest_fixed { s + size } else { s };
                        Node::split(dir, s, *first, inner)
                    }
                    // [rest | target] with new before target: new goes on
                    // the end of `rest`'s side.
                    (false, true) => {
                        let rest_fixed = !first.contains(Leaf::Terminal);
                        // `rest` keeps its extent when it is the fixed side;
                        // when target was, its side grows by `new`.
                        let s = if rest_fixed { s } else { s + size };
                        Node::split(
                            dir,
                            s,
                            *first,
                            Node::split(dir, size, Node::Leaf(new), *second),
                        )
                    }
                    // [rest | target] with new after target: target and
                    // new pair up in target's cell.
                    (false, false) => Node::split(dir, s, *first, pair(*second, room, false)),
                }
            }
            Node::Split {
                dir,
                size: s,
                first,
                second,
            } => Node::split(
                dir,
                s,
                first.insert_beside(target, side, new, size, room),
                second.insert_beside(target, side, new, size, room),
            ),
        }
    }

    /// `a` and `b` are the two leaves of one split running along `dir`:
    /// swap them. The split's size stays with the leaf it named, or — when
    /// neither is the terminal and the name passes to the other — flips to
    /// the rest of `total`, the split's whole extent. False if they are
    /// not such siblings.
    fn swap_siblings(&mut self, a: Leaf, b: Leaf, dir: Dir, total: u16) -> bool {
        let Node::Split {
            dir: d,
            size,
            first,
            second,
        } = self
        else {
            return false;
        };
        let pair = matches!((&**first, &**second), (Node::Leaf(x), Node::Leaf(y))
            if (*x == a && *y == b) || (*x == b && *y == a));
        if pair && *d == dir {
            std::mem::swap(first, second);
            if !first.contains(Leaf::Terminal) && !second.contains(Leaf::Terminal) {
                *size = total.saturating_sub(*size);
            }
            return true;
        }
        first.swap_siblings(a, b, dir, total) || second.swap_siblings(a, b, dir, total)
    }

    fn size_mut(&mut self, id: usize) -> Option<&mut u16> {
        let mut next = 0;
        self.size_mut_walk(id, &mut next)
    }

    fn size_mut_walk(&mut self, id: usize, next: &mut usize) -> Option<&mut u16> {
        match self {
            Node::Leaf(_) => None,
            Node::Split {
                size,
                first,
                second,
                ..
            } => {
                let mine = *next;
                *next += 1;
                if mine == id {
                    return Some(size);
                }
                if let Some(s) = first.size_mut_walk(id, next) {
                    return Some(s);
                }
                second.size_mut_walk(id, next)
            }
        }
    }

    /// The size cell of the nearest split that fixes `leaf`'s extent along
    /// `dir`: what "the panel's width" means in a tree.
    fn fixing_size_mut(&mut self, leaf: Leaf, dir: Dir) -> Option<&mut u16> {
        let Node::Split {
            dir: d,
            size,
            first,
            second,
        } = self
        else {
            return None;
        };
        let fixed_first = !first.contains(Leaf::Terminal);
        let (child, is_fixed) = if first.contains(leaf) {
            (first, fixed_first)
        } else if second.contains(leaf) {
            (second, !fixed_first)
        } else {
            return None;
        };
        // Borrow dance: probe the child first, fall back to this split.
        if child.fixing_size_mut(leaf, dir).is_some() {
            return child.fixing_size_mut(leaf, dir);
        }
        (is_fixed && *d == dir).then_some(size)
    }
}

/// A tile placed on screen. Panel rects exclude the rules around them, so a
/// panel draws no border of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub leaf: Leaf,
    pub area: Rect,
}

/// A draggable boundary between a split's two shown children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Boundary {
    pub id: usize,
    pub dir: Dir,
    /// The rule: one cell wide (`Row`) or one cell tall (`Col`).
    pub rule: Rect,
    /// The split's whole area, both children and the rule.
    pub span: Rect,
    fixed_first: bool,
    min_first: u16,
    min_second: u16,
}

impl Boundary {
    /// Screen coordinate where the second child starts: x for a `Row`
    /// split, y for a `Col` split. The rule is the cell just before it.
    pub fn pos(&self) -> u16 {
        match self.dir {
            Dir::Row => self.rule.x + 1,
            Dir::Col => self.rule.y + 1,
        }
    }

    /// The two cells straddling the rule, the mouse grab zone.
    pub fn grab(&self) -> Rect {
        match self.dir {
            Dir::Row => Rect {
                width: 2,
                ..self.rule
            },
            Dir::Col => Rect {
                height: 2,
                ..self.rule
            },
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolved {
    pub leaves: Vec<Placed>,
    pub boundaries: Vec<Boundary>,
}

impl Resolved {
    pub fn area(&self, leaf: Leaf) -> Option<Rect> {
        self.leaves.iter().find(|p| p.leaf == leaf).map(|p| p.area)
    }

    pub fn boundary(&self, id: usize) -> Option<&Boundary> {
        self.boundaries.iter().find(|b| b.id == id)
    }

    /// The tile touching `leaf` on `side`, when there is one: the nearest
    /// in that direction among those overlapping it across the other axis,
    /// the widest overlap winning.
    fn neighbour(&self, leaf: Leaf, side: Side) -> Option<Leaf> {
        let me = self.area(leaf)?;
        let (mine_lo, mine_hi) = match side.dir() {
            Dir::Row => (me.y, me.y + me.height),
            Dir::Col => (me.x, me.x + me.width),
        };
        self.leaves
            .iter()
            .filter(|p| p.leaf != leaf)
            .filter_map(|p| {
                let r = p.area;
                let distance = match side {
                    Side::Right if r.x >= me.x + me.width => r.x - (me.x + me.width),
                    Side::Left if r.x + r.width <= me.x => me.x - (r.x + r.width),
                    Side::Below if r.y >= me.y + me.height => r.y - (me.y + me.height),
                    Side::Above if r.y + r.height <= me.y => me.y - (r.y + r.height),
                    _ => return None,
                };
                let (lo, hi) = match side.dir() {
                    Dir::Row => (r.y, r.y + r.height),
                    Dir::Col => (r.x, r.x + r.width),
                };
                let overlap = hi.min(mine_hi).saturating_sub(lo.max(mine_lo));
                (overlap > 0).then_some((distance, std::cmp::Reverse(overlap), p.leaf))
            })
            .min_by_key(|(distance, overlap, _)| (*distance, *overlap))
            .map(|(_, _, leaf)| leaf)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelLayout {
    root: Node,
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self::columns(crate::app::DEFAULT_PANEL_WIDTHS)
    }
}

impl PanelLayout {
    /// The classic layout: three columns left of the terminal, at `widths`.
    pub fn columns(widths: [u16; 3]) -> Self {
        let root = Node::split(
            Dir::Row,
            widths[0],
            Node::Leaf(Leaf::Panel(0)),
            Node::split(
                Dir::Row,
                widths[1],
                Node::Leaf(Leaf::Panel(1)),
                Node::split(
                    Dir::Row,
                    widths[2],
                    Node::Leaf(Leaf::Panel(2)),
                    Node::Leaf(Leaf::Terminal),
                ),
            ),
        );
        Self { root }
    }

    /// A tree that lost a tile (a hand-edited blob, an older schema) is
    /// unusable: every panel and the terminal must appear exactly once.
    pub fn is_complete(&self) -> bool {
        let mut seen = [0usize; 4];
        fn walk(n: &Node, seen: &mut [usize; 4]) {
            match n {
                Node::Leaf(Leaf::Panel(p)) if *p < 3 => seen[*p] += 1,
                Node::Leaf(Leaf::Panel(_)) => seen[3] += 100,
                Node::Leaf(Leaf::Terminal) => seen[3] += 1,
                Node::Split { first, second, .. } => {
                    walk(first, seen);
                    walk(second, seen);
                }
            }
        }
        walk(&self.root, &mut seen);
        seen == [1, 1, 1, 1]
    }

    /// Lay the shown tiles out over `area`.
    pub fn resolve(&self, area: Rect, visible: [bool; 3]) -> Resolved {
        let mut out = Resolved::default();
        let mut next = 0;
        place(&self.root, area, visible, &mut next, &mut out);
        out.boundaries.sort_by_key(|b| b.id);
        out
    }

    /// Drag boundary `id` so its rule's far edge lands at screen coordinate
    /// `pos` (x for a `Row` split, y for a `Col`), within the split's
    /// minimums. `area`/`visible` are what the screen currently shows.
    pub fn set_boundary(&mut self, id: usize, pos: i32, area: Rect, visible: [bool; 3]) {
        let resolved = self.resolve(area, visible);
        let Some(b) = resolved.boundary(id) else {
            return;
        };
        let (start, extent) = match b.dir {
            Dir::Row => (b.span.x, b.span.width),
            Dir::Col => (b.span.y, b.span.height),
        };
        let pos = pos.clamp(start as i32, (start + extent) as i32) as u16;
        // `pos` is where the second child starts; the first child's extent
        // plus its rule is exactly that far from the split's start.
        let first_size = pos.saturating_sub(start);
        let (want, min, other_min) = if b.fixed_first {
            (first_size, b.min_first, b.min_second)
        } else {
            (extent.saturating_sub(first_size), b.min_second, b.min_first)
        };
        if extent < min + 1 + other_min {
            return; // too small to honour the minimums; leave it alone
        }
        let (min, max) = (min + 1, extent - other_min);
        if let Some(size) = self.root.size_mut(id) {
            *size = want.clamp(min, max);
        }
    }

    /// Move panel `idx` to `side` of the tile it touches there; at the edge
    /// of the body it becomes a full strip along that edge instead.
    pub fn move_panel(&mut self, idx: usize, side: Side, area: Rect, visible: [bool; 3]) {
        let leaf = Leaf::Panel(idx);
        if !self.root.contains(leaf) {
            return;
        }
        let before = self.resolve(area, visible);
        let Some(mine) = before.area(leaf) else {
            return; // hidden: nowhere to move from
        };
        let my_size = 1 + match side.dir() {
            Dir::Row => mine.width,
            Dir::Col => mine.height,
        };
        let size = landing_size(side.dir(), my_size);
        let target = before.neighbour(leaf, side);
        // The neighbour is this panel's own sibling in a split running the
        // same way: a swap, with nothing to take apart.
        if let Some(target) = target {
            let extent = |r: Rect| match side.dir() {
                Dir::Row => r.width,
                Dir::Col => r.height,
            };
            let total = my_size + before.area(target).map_or(0, extent);
            if self.root.swap_siblings(leaf, target, side.dir(), total) {
                return;
            }
        }
        let root = std::mem::replace(&mut self.root, Node::Leaf(Leaf::Terminal));
        let root = root.remove(leaf);
        self.root = match target {
            Some(target) => {
                let room = before.area(target).map_or(0, |r| match side.dir() {
                    Dir::Row => r.width,
                    Dir::Col => r.height,
                });
                root.insert_beside(target, side, leaf, size, room)
            }
            None => {
                if side.first() {
                    Node::split(side.dir(), size, Node::Leaf(leaf), root)
                } else {
                    Node::split(side.dir(), size, root, Node::Leaf(leaf))
                }
            }
        };
    }

    /// The width a `Row` split fixes for panel `idx` — its column width in
    /// the classic layout. `None` when nothing fixes it (it stretches).
    pub fn panel_width(&self, idx: usize) -> Option<u16> {
        let mut tree = self.clone();
        tree.root
            .fixing_size_mut(Leaf::Panel(idx), Dir::Row)
            .copied()
    }

    pub fn set_panel_width(&mut self, idx: usize, width: u16) {
        if let Some(size) = self.root.fixing_size_mut(Leaf::Panel(idx), Dir::Row) {
            *size = width;
        }
    }

    /// Cap every remembered size (a restored blob may quote a screen far
    /// wider than this one; the draw re-clamps to what fits).
    pub fn clamp_sizes(&mut self, max: u16) {
        fn walk(n: &mut Node, max: u16) {
            if let Node::Split {
                size,
                first,
                second,
                ..
            } = n
            {
                *size = (*size).min(max);
                walk(first, max);
                walk(second, max);
            }
        }
        walk(&mut self.root, max);
    }
}

/// Size (rule included) a panel asks for where it lands: what it had along
/// the new axis when that was a real choice. Whatever the screen can't fit,
/// the draw re-clamps.
fn landing_size(dir: Dir, had: u16) -> u16 {
    let min = match dir {
        Dir::Row => MIN_PANEL_W,
        Dir::Col => MIN_PANEL_H,
    };
    if dir == Dir::Col && had > DEFAULT_STACK_H * 2 {
        DEFAULT_STACK_H // came from a full-height column
    } else {
        had.max(min)
    }
}

fn place(node: &Node, area: Rect, visible: [bool; 3], next: &mut usize, out: &mut Resolved) {
    match node {
        Node::Leaf(leaf) => {
            if node.shown(visible) {
                out.leaves.push(Placed { leaf: *leaf, area });
            }
        }
        Node::Split {
            dir,
            size,
            first,
            second,
        } => {
            let id = *next;
            *next += 1;
            match (first.shown(visible), second.shown(visible)) {
                (true, false) => {
                    place(first, area, visible, next, out);
                    *next += second.split_count();
                }
                (false, true) => {
                    *next += first.split_count();
                    place(second, area, visible, next, out);
                }
                (false, false) => {
                    *next += first.split_count() + second.split_count();
                }
                (true, true) => {
                    let extent = match dir {
                        Dir::Row => area.width,
                        Dir::Col => area.height,
                    };
                    let min_first = first.min_extent(*dir, visible);
                    let min_second = second.min_extent(*dir, visible);
                    let fixed_first = node.fixed_first();
                    let (fixed_min, other_min) = if fixed_first {
                        (min_first, min_second)
                    } else {
                        (min_second, min_first)
                    };
                    // The fixed side's size counts the rule; the other side
                    // gets what remains.
                    let fixed = if extent >= fixed_min + 1 + other_min {
                        (*size).clamp(fixed_min + 1, extent - other_min)
                    } else {
                        // Too small for both minimums: share what there is.
                        (extent * (fixed_min + 1) / (fixed_min + 1 + other_min)).min(extent)
                    };
                    let first_extent = if fixed_first {
                        fixed.saturating_sub(1)
                    } else {
                        extent.saturating_sub(fixed)
                    };
                    let (a, rule, b) = match dir {
                        Dir::Row => (
                            Rect {
                                width: first_extent,
                                ..area
                            },
                            Rect {
                                x: area.x + first_extent,
                                width: (area.width > first_extent) as u16,
                                ..area
                            },
                            Rect {
                                x: area.x + first_extent + 1,
                                width: area.width.saturating_sub(first_extent + 1),
                                ..area
                            },
                        ),
                        Dir::Col => (
                            Rect {
                                height: first_extent,
                                ..area
                            },
                            Rect {
                                y: area.y + first_extent,
                                height: (area.height > first_extent) as u16,
                                ..area
                            },
                            Rect {
                                y: area.y + first_extent + 1,
                                height: area.height.saturating_sub(first_extent + 1),
                                ..area
                            },
                        ),
                    };
                    place(first, a, visible, next, out);
                    place(second, b, visible, next, out);
                    // A drag never squeezes another panel: the side holding
                    // the terminal gives up only the terminal's slack, so
                    // its floor is what it shows now minus that.
                    let term = out.area(Leaf::Terminal).map_or(0, |t| match dir {
                        Dir::Row => t.width,
                        Dir::Col => t.height,
                    });
                    let floor = |node: &Node, shown: u16, min: u16| {
                        if node.contains(Leaf::Terminal) {
                            let term_min = match dir {
                                Dir::Row => MIN_TERM_W,
                                Dir::Col => MIN_TERM_H,
                            };
                            shown.saturating_sub(term.saturating_sub(term_min)).max(min)
                        } else {
                            min
                        }
                    };
                    let (a_extent, b_extent) = match dir {
                        Dir::Row => (a.width, b.width),
                        Dir::Col => (a.height, b.height),
                    };
                    out.boundaries.push(Boundary {
                        id,
                        dir: *dir,
                        rule,
                        span: area,
                        fixed_first,
                        min_first: floor(first, a_extent, min_first),
                        min_second: floor(second, b_extent, min_second),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [bool; 3] = [true; 3];
    fn body() -> Rect {
        Rect::new(0, 0, 120, 35)
    }

    #[test]
    fn default_columns_number_their_boundaries_left_to_right() {
        let layout = PanelLayout::default();
        let r = layout.resolve(body(), ALL);
        let pos: Vec<(usize, u16)> = r.boundaries.iter().map(|b| (b.id, b.pos())).collect();
        assert_eq!(pos, vec![(0, 20), (1, 42), (2, 74)]);
        assert_eq!(r.area(Leaf::Panel(0)), Some(Rect::new(0, 0, 19, 35)));
        assert_eq!(r.area(Leaf::Terminal), Some(Rect::new(74, 0, 46, 35)));
        assert!(layout.is_complete());
    }

    #[test]
    fn hidden_panel_collapses_its_split_and_keeps_ids() {
        let layout = PanelLayout::default();
        let r = layout.resolve(body(), [false, true, true]);
        let ids: Vec<usize> = r.boundaries.iter().map(|b| b.id).collect();
        assert_eq!(ids, vec![1, 2]);
        assert_eq!(r.area(Leaf::Panel(1)).unwrap().x, 0);
        assert_eq!(r.area(Leaf::Panel(0)), None);
    }

    #[test]
    fn terminal_keeps_its_minimum_when_the_screen_shrinks() {
        let layout = PanelLayout::columns([40, 40, 40]);
        let r = layout.resolve(Rect::new(0, 0, 100, 35), ALL);
        assert!(r.area(Leaf::Terminal).unwrap().width >= MIN_TERM_W);
        for p in 0..3 {
            assert!(r.area(Leaf::Panel(p)).unwrap().width >= MIN_PANEL_W - 1);
        }
    }

    #[test]
    fn set_boundary_moves_the_fixed_side_within_limits() {
        let mut layout = PanelLayout::default();
        layout.set_boundary(0, 30, body(), ALL);
        assert_eq!(layout.panel_width(0), Some(30));
        layout.set_boundary(0, 2, body(), ALL);
        assert_eq!(layout.panel_width(0), Some(MIN_PANEL_W));
        layout.set_boundary(0, 500, body(), ALL);
        let r = layout.resolve(body(), ALL);
        assert_eq!(r.area(Leaf::Terminal).unwrap().width, MIN_TERM_W);
    }

    #[test]
    fn move_right_swaps_with_the_neighbour_and_past_the_terminal_lands_on_its_right() {
        let mut layout = PanelLayout::default();
        layout.move_panel(0, Side::Right, body(), ALL);
        let r = layout.resolve(body(), ALL);
        assert!(r.area(Leaf::Panel(1)).unwrap().x < r.area(Leaf::Panel(0)).unwrap().x);
        assert_eq!(layout.panel_width(0), Some(20));
        // Sessions right of the terminal.
        layout.move_panel(2, Side::Right, body(), ALL);
        let r = layout.resolve(body(), ALL);
        let term = r.area(Leaf::Terminal).unwrap();
        let sessions = r.area(Leaf::Panel(2)).unwrap();
        assert!(sessions.x > term.x + term.width);
        assert_eq!(sessions.x + sessions.width, 120);
        assert!(term.width >= MIN_TERM_W);
        assert!(layout.is_complete());
    }

    #[test]
    fn move_above_at_the_top_becomes_a_full_width_strip() {
        let mut layout = PanelLayout::default();
        layout.move_panel(1, Side::Above, body(), ALL);
        let r = layout.resolve(body(), ALL);
        let w = r.area(Leaf::Panel(1)).unwrap();
        assert_eq!((w.x, w.y, w.width), (0, 0, 120));
        assert_eq!(w.height, DEFAULT_STACK_H - 1);
        let term = r.area(Leaf::Terminal).unwrap();
        assert_eq!(term.y, DEFAULT_STACK_H);
        let top = r.boundaries.iter().find(|b| b.dir == Dir::Col).unwrap();
        assert_eq!(top.pos(), DEFAULT_STACK_H);
        // Dragging the horizontal rule resizes the strip.
        layout.set_boundary(top.id, 20, body(), ALL);
        let r = layout.resolve(body(), ALL);
        assert_eq!(r.area(Leaf::Panel(1)).unwrap().height, 19);
    }

    #[test]
    fn move_below_from_a_strip_stacks_over_the_tile_beneath() {
        let mut layout = PanelLayout::default();
        layout.move_panel(1, Side::Above, body(), ALL);
        layout.move_panel(1, Side::Below, body(), ALL);
        let r = layout.resolve(body(), ALL);
        let w = r.area(Leaf::Panel(1)).unwrap();
        // It sank into the terminal's column (the widest tile under it),
        // landing on top of the terminal.
        let term = r.area(Leaf::Terminal).unwrap();
        assert!(w.y == 0 && w.height < 35);
        assert_eq!((term.x, term.width), (w.x, w.width));
        assert_eq!(term.y, w.y + w.height + 1);
        // The next step down walks past the terminal; the one after that
        // is the bottom edge, a full-width strip.
        layout.move_panel(1, Side::Below, body(), ALL);
        let r = layout.resolve(body(), ALL);
        let w = r.area(Leaf::Panel(1)).unwrap();
        let term = r.area(Leaf::Terminal).unwrap();
        assert_eq!((w.x, w.width), (term.x, term.width));
        assert!(w.y > term.y);
        layout.move_panel(1, Side::Below, body(), ALL);
        let r = layout.resolve(body(), ALL);
        let w = r.area(Leaf::Panel(1)).unwrap();
        assert_eq!((w.x, w.width, w.y + w.height), (0, 120, 35));
        assert!(layout.is_complete());
    }

    #[test]
    fn moving_a_hidden_panel_is_a_no_op() {
        let mut layout = PanelLayout::default();
        let before = layout.clone();
        layout.move_panel(0, Side::Right, body(), [false, true, true]);
        assert_eq!(layout, before);
    }

    #[test]
    fn blob_roundtrips() {
        let mut layout = PanelLayout::default();
        layout.move_panel(2, Side::Below, body(), ALL);
        let json = serde_json::to_string(&layout).unwrap();
        let back: PanelLayout = serde_json::from_str(&json).unwrap();
        assert_eq!(back, layout);
    }
}
