// © 2025 <_@habnab.it>
//
// SPDX-License-Identifier: EUPL-1.2

use bevy::prelude::*;
use petgraph::{graph::NodeIndex, visit::EdgeRef, Direction, Graph};

use crate::{
    fit::FitClickedEvent,
    puzzle::{CellLoc, Puzzle},
    TopButtonAction, UpdateCellDisplay, UpdateCellIndex,
};

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

fn add_undo_state(
    mut ev_rx: EventReader<PushNewAction>,
    mut q_tree: Query<&mut UndoTree>,
    mut q_tree_loc: Query<&mut UndoTreeLocation>,
) {
    let Ok(mut tree) = q_tree.get_single_mut() else {
        return;
    };
    let Ok(mut tree_loc) = q_tree_loc.get_single_mut() else {
        return;
    };
    for ev in ev_rx.read() {
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

fn adjust_undo_state(
    mut ev_rx: EventReader<FitClickedEvent<TopButtonAction>>,
    mut q_puzzle: Query<&mut Puzzle>,
    q_tree: Query<&UndoTree>,
    mut q_tree_loc: Query<&mut UndoTreeLocation>,
    mut update_display_tx: EventWriter<UpdateCellDisplay>,
) {
    let Ok(mut puzzle) = q_puzzle.get_single_mut() else {
        return;
    };
    let Ok(tree) = q_tree.get_single() else {
        return;
    };
    let Ok(mut tree_loc) = q_tree_loc.get_single_mut() else {
        return;
    };
    for &FitClickedEvent(action) in ev_rx.read() {
        use TopButtonAction as B;
        let new_node = match action {
            B::Undo => {
                let Some(undo) = tree
                    .tree
                    .edges_directed(tree_loc.current, Direction::Outgoing)
                    .next()
                else {
                    warn!("nothing to undo");
                    continue;
                };
                info!("on undo: {undo:#?}");
                undo.target()
            }
            B::Redo => {
                let redos = tree
                    .tree
                    .edges_directed(tree_loc.current, Direction::Incoming)
                    .take(2)
                    .collect::<Vec<_>>();
                if redos.len() != 1 {
                    warn!("couldn't redo from {redos:#?}");
                    continue;
                }
                info!("on redo: {redos:#?}");
                redos[0].source()
            }
            _ => continue,
        };
        let Some(new_state) = tree.tree.node_weight(new_node) else {
            unreachable!()
        };
        tree_loc.current = new_node;
        puzzle.clone_from(new_state);
        for row in puzzle.iter_rows() {
            for col in puzzle.iter_cols() {
                update_display_tx.send(UpdateCellDisplay {
                    loc: CellLoc { row, col },
                });
            }
        }
    }
}

pub struct UndoPlugin;

impl Plugin for UndoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (add_undo_state, adjust_undo_state));
    }
}
