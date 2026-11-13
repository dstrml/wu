//! Commit graph shown beside each row of the Git panel's History tab when
//! `git_panel.history_style` is `graph`.
//!
//! The lanes come from the Git Graph's layout. This module decides which branch each commit and
//! line belongs to, and indexes the lines per row so each list row only paints the few lines that
//! cross it.

use std::{ops::Range, rc::Rc, sync::Arc};

use collections::{HashMap, HashSet};
use git::repository::{Branch, InitialGraphCommitData};
use gpui::{App, Hsla, IntoElement, PathBuilder, Pixels, SharedString, Styled, point, px};
use smallvec::SmallVec;
use theme::ActiveTheme as _;

use crate::git_graph::{
    CommitEntry, GitGraph, GraphData, LEFT_PADDING, LINE_WIDTH, paint_commit_lines,
};

/// Distance between lanes; narrower than the Git Graph's so the subjects keep their room.
const LANE_WIDTH: Pixels = px(10.);
/// Space between the panel edge and the first lane.
const GUTTER: Pixels = px(4.);
/// Lanes beyond this are clipped, so a busy history does not eat the commit subjects.
const MAX_LANES: usize = 6;
/// Branch names treated as the default branch when no `<remote>/HEAD` ref is found.
const DEFAULT_BRANCH_NAMES: [&str; 2] = ["main", "master"];

/// Which branch a commit or line belongs to; the palette turns it into a color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorRole {
    /// The checked-out branch's own commits.
    Head,
    /// The default branch the checked-out branch grew from.
    Base,
    /// The n-th merged branch, counting merges from the top.
    Merged(usize),
}

/// Colors for the graph, taken from the active theme.
#[derive(Clone)]
pub(crate) struct GraphPalette {
    head: Hsla,
    base: Hsla,
    merged: SmallVec<[Hsla; 8]>,
}

impl GraphPalette {
    pub(crate) fn new(cx: &App) -> Self {
        let theme = cx.theme();
        let head = theme.status().warning;
        let base = theme.status().info;
        let mut merged: SmallVec<[Hsla; 8]> = theme
            .accents()
            .0
            .iter()
            .copied()
            .filter(|color| *color != head && *color != base)
            .collect();
        if merged.is_empty() {
            merged.push(theme.status().created);
        }
        Self { head, base, merged }
    }

    fn color(&self, role: ColorRole) -> Hsla {
        match role {
            ColorRole::Head => self.head,
            ColorRole::Base => self.base,
            ColorRole::Merged(ix) => self.merged[ix % self.merged.len()],
        }
    }
}

/// A branch ref on a commit, with its local and remote copies merged.
pub(crate) struct HistoryBranch {
    pub(crate) name: SharedString,
    /// Whether the branch also exists on a remote, or only there.
    pub(crate) remote: bool,
}

pub(crate) struct HistoryGraph {
    graph: GraphData,
    /// The inputs the graph was built from, to tell when it is stale.
    current_branch: Option<String>,
    branch_list: Arc<[Branch]>,
    width: Pixels,
    commit_roles: Vec<ColorRole>,
    line_roles: Vec<ColorRole>,
    /// For each commit row, the lines that run through it, including those that start or end
    /// at it.
    lines_by_row: Vec<SmallVec<[u32; 4]>>,
    branches_by_row: Vec<Vec<HistoryBranch>>,
}

impl HistoryGraph {
    pub(crate) fn empty() -> Self {
        Self::new(&[], None, Arc::from([]))
    }

    pub(crate) fn new(
        commits: &[Arc<InitialGraphCommitData>],
        current_branch: Option<&str>,
        branch_list: Arc<[Branch]>,
    ) -> Self {
        // Colors come from `roles` below, so the Git Graph's own color index is not needed.
        let mut graph = GraphData::new(1);
        graph.add_commits(commits);

        let remotes: HashSet<&str> = branch_list
            .iter()
            .filter_map(|branch| branch.ref_name.strip_prefix("refs/remotes/"))
            .filter_map(|name| name.split_once('/').map(|(remote, _)| remote))
            .collect();
        let branches_by_row: Vec<Vec<HistoryBranch>> = commits
            .iter()
            .map(|commit| branches(&commit.ref_names, &remotes))
            .collect();
        let base_row = base_row(commits, &branches_by_row, current_branch);

        let lanes = graph
            .commits
            .iter()
            .map(|entry| entry.lane + 1)
            .max()
            .unwrap_or(1)
            .min(MAX_LANES);
        let width = GUTTER + LANE_WIDTH * lanes as f32;

        let (commit_roles, line_roles) = roles(&graph, base_row);

        let mut lines_by_row: Vec<SmallVec<[u32; 4]>> = vec![SmallVec::new(); commits.len()];
        for (ix, line) in graph.lines.iter().enumerate() {
            let Range { start, end } = line.full_interval;
            for row in &mut lines_by_row[start..=end] {
                row.push(ix as u32);
            }
        }

        Self {
            graph,
            current_branch: current_branch.map(str::to_string),
            branch_list,
            width,
            commit_roles,
            line_roles,
            lines_by_row,
            branches_by_row,
        }
    }

    /// Whether the graph should be rebuilt for `commits`. The repository reloads its commits,
    /// as new `Arc`s, whenever history or refs change, so comparing pointers is enough. While
    /// the history is still loading, the graph is only rebuilt once the commit count doubles, so
    /// loading a long history stays linear.
    pub(crate) fn needs_rebuild(
        &self,
        commits: &[Arc<InitialGraphCommitData>],
        current_branch: Option<&str>,
        branch_list: &Arc<[Branch]>,
        is_loading: bool,
    ) -> bool {
        let shown = &self.graph.commits;
        let same_commit = |shown: Option<&Rc<CommitEntry>>,
                           commit: Option<&Arc<InitialGraphCommitData>>| {
            match (shown, commit) {
                (Some(shown), Some(commit)) => Arc::ptr_eq(&shown.data, commit),
                (None, None) => true,
                _ => false,
            }
        };
        let same_inputs = self.current_branch.as_deref() == current_branch
            && Arc::ptr_eq(&self.branch_list, branch_list)
            && same_commit(shown.first(), commits.first());
        if !same_inputs || shown.len() > commits.len() {
            return true;
        }
        if shown.len() == commits.len() {
            return !same_commit(shown.last(), commits.last());
        }
        !is_loading || commits.len() >= shown.len() * 2
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.graph.commits.is_empty()
    }

    pub(crate) fn width(&self) -> Pixels {
        self.width
    }

    pub(crate) fn entry(&self, row: usize) -> Option<&Rc<CommitEntry>> {
        self.graph.commits.get(row)
    }

    pub(crate) fn commit_color(&self, row: usize, palette: &GraphPalette) -> Option<Hsla> {
        self.commit_roles.get(row).map(|role| palette.color(*role))
    }

    pub(crate) fn branches(&self, row: usize) -> &[HistoryBranch] {
        self.branches_by_row.get(row).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn lane_center(lane: usize) -> Pixels {
        GUTTER + LANE_WIDTH * lane as f32 + LANE_WIDTH / 2.
    }

    /// Where a commit's marker goes: commits on clipped lanes keep a marker on the last
    /// visible lane.
    pub(crate) fn marker_center(lane: usize) -> Pixels {
        Self::lane_center(lane.min(MAX_LANES - 1))
    }

    /// Paints the lines crossing commit row `row` or, with `below`, the rows listed under it.
    pub(crate) fn render_lines(
        self: &Rc<Self>,
        row: usize,
        below: bool,
        palette: GraphPalette,
    ) -> impl IntoElement {
        let this = self.clone();
        gpui::canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let Some(indices) = this.lines_by_row.get(row) else {
                    return;
                };
                let color = |ix: usize| palette.color(this.line_roles[ix]);
                window.paint_layer(bounds, |window| {
                    if below {
                        for &ix in indices {
                            let ix = ix as usize;
                            let line = &this.graph.lines[ix];
                            if line.full_interval.end <= row {
                                continue;
                            }
                            let x = bounds.origin.x + Self::lane_center(line.column_below_row(row));
                            let mut builder = PathBuilder::stroke(LINE_WIDTH);
                            builder.move_to(point(x, bounds.origin.y));
                            builder.line_to(point(x, bounds.origin.y + bounds.size.height));
                            if let Ok(path) = builder.build() {
                                window.paint_path(path, color(ix));
                            }
                        }
                        return;
                    }
                    let lines = indices.iter().map(|&ix| {
                        let ix = ix as usize;
                        (this.graph.lines[ix].as_ref(), ix, color(ix))
                    });
                    // Shift the graph left so the first lane sits next to the panel edge
                    // instead of after the Git Graph's padding.
                    let mut shifted = bounds;
                    shifted.origin.x += GUTTER - LEFT_PADDING;
                    paint_commit_lines(
                        shifted,
                        lines,
                        row,
                        bounds.size.height,
                        px(0.),
                        LANE_WIDTH,
                        window,
                    );
                });
            },
        )
        .size_full()
    }
}

/// Branch refs on a commit, one per branch name; `remotes` tells `origin/x` from a local `a/b`.
fn branches(ref_names: &[SharedString], remotes: &HashSet<&str>) -> Vec<HistoryBranch> {
    let mut branches: Vec<HistoryBranch> = Vec::new();
    for name in ref_names {
        if name.starts_with("tag: ") || name.ends_with("/HEAD") {
            continue;
        }
        let Some(ref_name) = GitGraph::ref_name_from_decoration(name) else {
            continue;
        };
        let name = ref_name.as_ref();
        let (short, remote) = match name.split_once('/') {
            Some((remote, short)) if remotes.contains(remote) => (short, true),
            _ => (name, false),
        };
        match branches
            .iter_mut()
            .find(|branch| branch.name.as_ref() == short)
        {
            Some(branch) => branch.remote |= remote,
            None => branches.push(HistoryBranch {
                name: short.to_string().into(),
                remote,
            }),
        }
    }
    branches
}

/// First row of the default branch, where the checked-out branch's own commits end. Returns
/// `commits.len()` when there is no default branch or it is the checked-out branch itself, so
/// every lane-0 commit belongs to the checked-out branch.
fn base_row(
    commits: &[Arc<InitialGraphCommitData>],
    branches_by_row: &[Vec<HistoryBranch>],
    current_branch: Option<&str>,
) -> usize {
    let remote_head = commits
        .iter()
        .position(|commit| commit.ref_names.iter().any(|name| name.ends_with("/HEAD")));
    let default_row = remote_head.or_else(|| {
        branches_by_row.iter().position(|branches| {
            branches
                .iter()
                .any(|branch| DEFAULT_BRANCH_NAMES.contains(&branch.name.as_ref()))
        })
    });
    let on_default_branch = match (default_row, current_branch) {
        (Some(row), Some(current)) => branches_by_row[row]
            .iter()
            .any(|branch| branch.name.as_ref() == current),
        _ => false,
    };
    match default_row {
        Some(row) if !on_default_branch => row,
        _ => commits.len(),
    }
}

/// Assigns every commit and line to a branch: lane 0 is the checked-out branch down to
/// `base_row` and the default branch below it; each merged branch keeps the role of the merge
/// that brought it in.
fn roles(graph: &GraphData, base_row: usize) -> (Vec<ColorRole>, Vec<ColorRole>) {
    let main_role = |row: usize| {
        if row < base_row {
            ColorRole::Head
        } else {
            ColorRole::Base
        }
    };
    let mut commit_roles: Vec<Option<ColorRole>> = graph
        .commits
        .iter()
        .enumerate()
        .map(|(row, entry)| (entry.lane == 0).then(|| main_role(row)))
        .collect();
    let mut line_roles: Vec<ColorRole> = vec![ColorRole::Base; graph.lines.len()];

    let mut order: Vec<usize> = (0..graph.lines.len()).collect();
    order.sort_by_key(|&ix| graph.lines[ix].full_interval.start);
    let mut merge_roles: HashMap<usize, ColorRole> = HashMap::default();
    for ix in order {
        let line = &graph.lines[ix];
        let Range { start, end } = line.full_interval;
        let role = if line.starts_with_merge() {
            let next = merge_roles.len();
            *merge_roles.entry(start).or_insert(ColorRole::Merged(next))
        } else {
            commit_roles
                .get(start)
                .copied()
                .flatten()
                .unwrap_or_else(|| main_role(start))
        };
        line_roles[ix] = role;
        if let Some(slot @ None) = commit_roles.get_mut(end) {
            *slot = Some(role);
        }
    }

    let commit_roles = commit_roles
        .into_iter()
        .enumerate()
        .map(|(row, role)| role.unwrap_or_else(|| main_role(row)))
        .collect();
    (commit_roles, line_roles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git::Oid;

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap()
    }

    /// Commits in list order; each is `(id, parent ids, ref names)`.
    fn commits(spec: &[(u8, &[u8], &[&str])]) -> Vec<Arc<InitialGraphCommitData>> {
        spec.iter()
            .map(|(id, parents, refs)| {
                Arc::new(InitialGraphCommitData {
                    sha: oid(*id),
                    parents: parents.iter().map(|parent| oid(*parent)).collect(),
                    ref_names: refs.iter().map(|name| name.to_string().into()).collect(),
                })
            })
            .collect()
    }

    fn names(branches: &[HistoryBranch]) -> Vec<(&str, bool)> {
        branches
            .iter()
            .map(|branch| (branch.name.as_ref(), branch.remote))
            .collect()
    }

    #[test]
    fn test_branches_tell_remotes_from_slashed_local_names() {
        let remotes = HashSet::from_iter(["origin"]);
        let refs: Vec<SharedString> = [
            "HEAD -> feature/foo",
            "origin/feature/foo",
            "origin/main",
            "backup/x",
            "tag: v1",
            "origin/HEAD",
        ]
        .into_iter()
        .map(Into::into)
        .collect();

        assert_eq!(
            names(&branches(&refs, &remotes)),
            [("feature/foo", true), ("main", true), ("backup/x", false)]
        );
    }

    #[test]
    fn test_base_row_ignores_unrelated_branches_on_head() {
        let commits = commits(&[
            (1, &[2], &["HEAD -> topic", "tmp"]),
            (2, &[3], &[]),
            (3, &[], &["main"]),
        ]);
        let by_row: Vec<_> = commits
            .iter()
            .map(|commit| branches(&commit.ref_names, &HashSet::default()))
            .collect();

        assert_eq!(base_row(&commits, &by_row, Some("topic")), 2);
    }

    #[test]
    fn test_base_row_on_default_branch_puts_everything_on_head() {
        let commits = commits(&[(1, &[2], &["HEAD -> main"]), (2, &[], &[])]);
        let by_row: Vec<_> = commits
            .iter()
            .map(|commit| branches(&commit.ref_names, &HashSet::default()))
            .collect();

        assert_eq!(base_row(&commits, &by_row, Some("main")), commits.len());
    }

    #[test]
    fn test_merged_branch_keeps_the_role_of_its_own_merge() {
        // M1 merges A and M2 merges B, but A's commit sits below M2.
        let commits = commits(&[
            (1, &[2, 4], &[]),
            (2, &[3, 5], &[]),
            (3, &[6], &[]),
            (4, &[6], &[]),
            (5, &[6], &[]),
            (6, &[], &[]),
        ]);
        let graph = HistoryGraph::new(&commits, None, Arc::from([]));

        assert_eq!(graph.commit_roles[3], ColorRole::Merged(0));
        assert_eq!(graph.commit_roles[4], ColorRole::Merged(1));
    }

    #[test]
    fn test_needs_rebuild_while_loading_waits_for_the_history_to_double() {
        let all = commits(&[
            (1, &[2], &[]),
            (2, &[3], &[]),
            (3, &[4], &[]),
            (4, &[5], &[]),
            (5, &[], &[]),
        ]);
        let branch_list: Arc<[Branch]> = Arc::from([]);
        let graph = HistoryGraph::new(&all[..2], Some("topic"), branch_list.clone());
        let needs_rebuild = |commits: &[Arc<InitialGraphCommitData>], branch, is_loading| {
            graph.needs_rebuild(commits, branch, &branch_list, is_loading)
        };

        assert!(!needs_rebuild(&all[..2], Some("topic"), true));
        assert!(!needs_rebuild(&all[..3], Some("topic"), true));
        assert!(needs_rebuild(&all[..4], Some("topic"), true));
        assert!(needs_rebuild(&all[..3], Some("topic"), false));
        assert!(needs_rebuild(&all[..2], Some("main"), true));
        let reloaded = commits(&[(1, &[2], &[]), (2, &[], &[])]);
        assert!(needs_rebuild(&reloaded, Some("topic"), true));
    }

    #[test]
    fn test_lines_by_row_lists_each_line_on_every_row_it_spans() {
        let commits = commits(&[
            (1, &[2, 3], &[]),
            (2, &[4], &[]),
            (3, &[4], &[]),
            (4, &[], &[]),
        ]);
        let graph = HistoryGraph::new(&commits, None, Arc::from([]));

        for (ix, line) in graph.graph.lines.iter().enumerate() {
            for (row, lines) in graph.lines_by_row.iter().enumerate() {
                let spans = line.full_interval.start <= row && row <= line.full_interval.end;
                assert_eq!(lines.contains(&(ix as u32)), spans, "line {ix}, row {row}");
            }
        }
    }
}
