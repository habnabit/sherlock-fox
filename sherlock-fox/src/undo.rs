// © 2025 <_@habnab.it>
//
// SPDX-License-Identifier: EUPL-1.2

use bevy::prelude::*;
use petgraph::{graph::NodeIndex, Graph};

use crate::{puzzle::Puzzle, UpdateCellIndex};

#[derive(Debug, Event, Reflect)]
pub struct PushNewAction {
    pub new_state: Puzzle,
    pub action: Action,
}

#[derive(Debug, Clone, Reflect)]
pub struct Action {
    pub update: UpdateCellIndex,
    pub update_count: usize,
    pub inferred_count: usize,
}

#[derive(Debug, Component, Reflect)]
pub struct UndoTree {
    #[reflect(ignore)]
    pub tree: Graph<Puzzle, Action>,
    pub root: NodeIndex,
}

#[derive(Debug, Component, Reflect)]
pub struct UndoTreeLocation {
    pub current: NodeIndex,
}

pub fn add_undo_state(
    mut reader: EventReader<PushNewAction>,
    mut q_tree: Query<&mut UndoTree>,
    mut q_tree_loc: Query<&mut UndoTreeLocation>,
) {
    let Ok(mut tree) = q_tree.get_single_mut() else {
        return;
    };
    let Ok(mut tree_loc) = q_tree_loc.get_single_mut() else {
        return;
    };
    for ev in reader.read() {
        info!(
            "tree in: {tree_loc:?} nodes={} edges={}",
            tree.tree.node_count(),
            tree.tree.edge_count()
        );
        let new_node = tree.tree.add_node(ev.new_state.clone());
        tree.tree
            .add_edge(new_node, tree_loc.current, ev.action.clone());
        tree_loc.current = new_node;
        info!(
            "tree out: {tree_loc:?} nodes={} edges={}",
            tree.tree.node_count(),
            tree.tree.edge_count()
        );
    }
}
