use crate::api::{Hn, Item};
use futures::future::{join_all, BoxFuture, FutureExt};
use std::collections::HashSet;
use std::sync::Arc;

pub struct Node {
    pub item: Arc<Item>,
    pub children: Vec<Arc<Node>>,
}

#[derive(Clone)]
pub struct Flat {
    pub node: Arc<Node>,
    pub depth: usize,
    pub hidden_children: usize,
}

// a comment that fails to load is simply omitted; whole subtrees load
// concurrently (no limit, like the TS version)
pub fn load_tree(hn: Arc<Hn>, id: u64, depth: usize, max_depth: usize) -> BoxFuture<'static, Option<Arc<Node>>> {
    async move {
        let item = hn.fetch_item(id).await.ok().flatten()?;
        if item.deleted || item.dead {
            return None;
        }
        let kids = item.kids.clone().unwrap_or_default();
        let children = if depth < max_depth && !kids.is_empty() {
            join_all(kids.into_iter().map(|k| load_tree(hn.clone(), k, depth + 1, max_depth)))
                .await
                .into_iter()
                .flatten()
                .collect()
        } else {
            vec![]
        };
        Some(Arc::new(Node { item, children }))
    }
    .boxed()
}

pub async fn load_forest(hn: Arc<Hn>, ids: Vec<u64>, max_depth: usize) -> Vec<Arc<Node>> {
    join_all(ids.into_iter().map(|id| load_tree(hn.clone(), id, 0, max_depth))).await.into_iter().flatten().collect()
}

pub fn flatten(tree: &[Arc<Node>], collapsed: &HashSet<u64>) -> Vec<Flat> {
    fn walk(nodes: &[Arc<Node>], depth: usize, collapsed: &HashSet<u64>, out: &mut Vec<Flat>) {
        for n in nodes {
            let is_collapsed = collapsed.contains(&n.item.id);
            out.push(Flat { node: n.clone(), depth, hidden_children: if is_collapsed { count_all(n) } else { 0 } });
            if !is_collapsed {
                walk(&n.children, depth + 1, collapsed, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(tree, 0, collapsed, &mut out);
    out
}

fn count_all(n: &Node) -> usize {
    n.children.len() + n.children.iter().map(|c| count_all(c)).sum::<usize>()
}

/// index of the top-level comment whose thread contains flat[from]
pub fn root_index_of(flat: &[Flat], from: usize) -> Option<usize> {
    if flat.is_empty() {
        return None;
    }
    (0..=from.min(flat.len() - 1)).rev().find(|&i| flat[i].depth == 0)
}

/// next (forward) or previous top-level comment. Going backward from a
/// nested comment lands on its own thread's root first.
pub fn next_root_index(flat: &[Flat], from: usize, forward: bool) -> Option<usize> {
    if forward {
        (from + 1..flat.len()).find(|&i| flat[i].depth == 0)
    } else {
        (0..from.min(flat.len())).rev().find(|&i| flat[i].depth == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ItemType;

    fn node(id: u64, children: Vec<Arc<Node>>) -> Arc<Node> {
        let item = Item {
            id,
            kind: ItemType::Comment,
            by: None,
            time: None,
            text: None,
            dead: false,
            deleted: false,
            parent: None,
            kids: None,
            url: None,
            score: None,
            title: None,
            descendants: None,
        };
        Arc::new(Node { item: Arc::new(item), children })
    }

    // 1 ─ 2 ─ 3      roots: 1, 4
    //   └ 5
    // 4
    fn sample() -> Vec<Arc<Node>> {
        vec![node(1, vec![node(2, vec![node(3, vec![])]), node(5, vec![])]), node(4, vec![])]
    }

    #[test]
    fn flatten_and_collapse() {
        let t = sample();
        let f = flatten(&t, &HashSet::new());
        assert_eq!(f.iter().map(|x| (x.node.item.id, x.depth)).collect::<Vec<_>>(), [(1, 0), (2, 1), (3, 2), (5, 1), (4, 0)]);
        let f = flatten(&t, &HashSet::from([1]));
        assert_eq!(f.iter().map(|x| (x.node.item.id, x.hidden_children)).collect::<Vec<_>>(), [(1, 3), (4, 0)]);
    }

    #[test]
    fn thread_navigation() {
        let f = flatten(&sample(), &HashSet::new());
        assert_eq!(root_index_of(&f, 2), Some(0));
        assert_eq!(root_index_of(&f, 4), Some(4));
        assert_eq!(next_root_index(&f, 1, true), Some(4));
        assert_eq!(next_root_index(&f, 4, true), None);
        assert_eq!(next_root_index(&f, 3, false), Some(0));
        assert_eq!(next_root_index(&f, 0, false), None);
        assert_eq!(root_index_of(&[], 0), None);
    }
}
