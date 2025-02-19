use bevy::prelude::*;
use rand::Rng;

use crate::{
    puzzle::{CellLoc, CellLocIndex, Puzzle, PuzzleCellSelection, UpdateCellIndexOperation},
    UpdateCellIndex,
};

pub type PuzzleAdvance = Option<UpdateCellIndex>;

pub trait PuzzleClue: std::fmt::Debug {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance;
    fn display(&self);
    fn spawn_into<'s, 'p: 's>(
        &'s self,
        puzzle: &'p Puzzle,
    ) -> Box<dyn FnOnce(&mut ChildBuilder) + 's>;
}

#[derive(Reflect, Asset, Debug)]
#[reflect(from_reflect = false)]
pub struct DynPuzzleClue(#[reflect(ignore)] Box<(dyn PuzzleClue + Sync + Send + 'static)>);

// impl std::fmt::Debug for DynPuzzleClue {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_tuple("DynPuzzleClue").finish()
//     }
// }

impl FromReflect for DynPuzzleClue {
    fn from_reflect(reflect: &dyn PartialReflect) -> Option<Self> {
        todo!()
    }
}

impl std::ops::Deref for DynPuzzleClue {
    type Target = dyn PuzzleClue;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<C: PuzzleClue + Sync + Send + 'static> From<C> for DynPuzzleClue {
    fn from(value: C) -> Self {
        Self::new(value)
    }
}

impl DynPuzzleClue {
    fn new(clue: impl PuzzleClue + Sync + Send + 'static) -> Self {
        DynPuzzleClue(Box::new(clue))
    }
}

#[derive(Debug, Component, Default, Reflect)]
pub struct PuzzleClues {
    pub clues: Vec<Handle<DynPuzzleClue>>,
}

#[derive(Debug, Component, Clone, Reflect)]
pub struct SameColumnClue {
    loc: CellLoc,
    row2: usize,
    row3: Option<usize>,
}

impl SameColumnClue {
    pub fn new_random<R: Rng>(rng: &mut R, puzzle: &Puzzle) -> Option<Self> {
        let n_rows = puzzle.rows.len();
        let mut rows = rand::seq::index::sample(rng, n_rows, n_rows).into_iter();
        let first_row = rows.next()?;
        let cell_nr = rng.random_range(0..puzzle.max_column) as isize;
        let loc = CellLoc {
            row_nr: first_row,
            cell_nr,
        };
        let row2 = rows.next()?;
        let row3 = if rng.random_ratio(1, 3) {
            rows.next()
        } else {
            None
        };
        Some(SameColumnClue { loc, row2, row3 })
    }

    fn loc2(&self) -> CellLoc {
        CellLoc {
            row_nr: self.row2,
            ..self.loc
        }
    }

    fn loc3(&self) -> Option<CellLoc> {
        try {
            CellLoc {
                row_nr: self.row3?,
                ..self.loc
            }
        }
    }
}

struct ImplicationResolver<'p, IT> {
    puzzle: &'p Puzzle,
    cells: Vec<CellLocIndex>,
    actions: Vec<IT>,
}

impl<'p, IT: std::fmt::Debug> std::fmt::Debug for ImplicationResolver<'p, IT> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImplicationResolver")
            .field("puzzle", &(self.puzzle as *const Puzzle as usize))
            .field("cells", &self.cells)
            .field("actions", &self.actions)
            .finish()
    }
}

#[derive(Clone, Debug)]
struct SelectionProxy {
    index: CellLocIndex,
    is_enabled: bool,
    is_solo: bool,
    is_void: bool,
}

impl SelectionProxy {
    fn from_puzzle_and_index(puzzle: &Puzzle, index: CellLocIndex) -> Self {
        let sel = puzzle.cell_selection(index.loc);
        let is_enabled = sel.is_enabled(index.index);
        let is_solo = sel.is_solo(index.index);
        let is_void = sel.is_void();
        SelectionProxy {
            index,
            is_enabled,
            is_solo,
            is_void,
        }
    }

    fn is_enabled_not_solo(&self) -> bool {
        self.is_enabled && !self.is_solo
    }
}

type IfThen2<R> = fn(&SelectionProxy, &SelectionProxy) -> Option<R>;
type IfThen3<R> = fn(&SelectionProxy, &SelectionProxy, &SelectionProxy) -> Option<R>;
type IfThen5<R> = fn(
    &SelectionProxy,
    &SelectionProxy,
    &SelectionProxy,
    &SelectionProxy,
    &SelectionProxy,
) -> Option<R>;

#[derive(Debug, Clone, Copy)]
struct ImplicationWidth {
    min: isize,
    max: isize,
    cellspan: usize,
}

impl<'p> ImplicationResolver<'p, ()> {
    fn new_unit(puzzle: &'p Puzzle) -> Self {
        ImplicationResolver {
            puzzle,
            cells: Vec::default(),
            actions: Vec::default(),
        }
    }
}

impl<'p, IT> ImplicationResolver<'p, IT> {
    fn add_loc(&mut self, loc: CellLoc) {
        let index = self.puzzle.cell_answer_index(loc);
        self.cells.push(CellLocIndex { loc, index });
    }

    fn width(&self) -> ImplicationWidth {
        use itertools::{Itertools, MinMaxResult::*};
        let (min, max) = match self.cells.iter().map(|i| i.loc.cell_nr).minmax() {
            OneElement(c) => (c, c),
            MinMax(a, b) => (a, b),
            NoElements => unreachable!(),
        };
        ImplicationWidth {
            min,
            max,
            cellspan: max.abs_diff(min),
        }
    }

    fn iter_all_cols<IT2>(&self) -> impl Iterator<Item = ImplicationResolver<IT2>> {
        let width = self.width();
        (-(width.cellspan as isize)..self.puzzle.max_column as isize).map(move |shift| {
            let shift = shift - width.min;
            let cells = self.cells.iter().map(|&c| c.shift_loc(shift)).collect();
            ImplicationResolver {
                cells,
                actions: Vec::default(),
                puzzle: self.puzzle,
            }
        })
    }

    fn if_then(&mut self, if_then_fn: IT) -> &mut Self {
        self.actions.push(if_then_fn);
        self
    }
}

impl<'p, R> ImplicationResolver<'p, IfThen2<R>> {
    fn iter_perm_2s(&self) -> impl Iterator<Item = R> + use<'_, R> {
        use itertools::Itertools;
        let proxies = self
            .cells
            .iter()
            .map(|&c| SelectionProxy::from_puzzle_and_index(self.puzzle, c))
            .collect::<Vec<_>>();
        let actions_iter = self.actions.iter();
        proxies
            .into_iter()
            .permutations(2)
            .flat_map(move |mut sels| {
                let (Some(s2), Some(s1)) = (sels.pop(), sels.pop()) else {
                    unreachable!()
                };
                info!("incoming perm 2:\n  s1={s1:?}\n  s2={s2:?}");
                actions_iter.clone().filter_map(move |a| (a)(&s1, &s2))
            })
    }
}

impl<'p, R> ImplicationResolver<'p, IfThen3<R>> {
    fn iter_reflected_2s(&self) -> impl Iterator<Item = R> + use<'_, R> {
        use itertools::Itertools;
        let proxy = |c| SelectionProxy::from_puzzle_and_index(self.puzzle, c);
        let proxies = self
            .cells
            .iter()
            .permutations(2)
            .map(move |mut locs| {
                let (Some(&loc2), Some(&loc1)) = (locs.pop(), locs.pop()) else {
                    unreachable!()
                };
                let loc2_p = loc2.reflect_loc_about(loc1);
                (proxy(loc2_p), proxy(loc1), proxy(loc2))
            })
            .inspect(|t| {
                info!(
                    "incoming reflected 2:\n  s1={:?}\n  s2={:?}\n  s3={:?}",
                    t.0, t.1, t.2
                )
            })
            .filter(|(_, s2, _)| !s2.is_void)
            .collect::<Vec<_>>();
        let actions_iter = self.actions.iter();
        proxies.into_iter().flat_map(move |(s1, s2, s3)| {
            actions_iter.clone().filter_map(move |a| (a)(&s1, &s2, &s3))
        })
    }
}

impl<'p, R> ImplicationResolver<'p, IfThen5<R>> {
    fn iter_reflected_3s(&self) -> impl Iterator<Item = R> + use<'_, R> {
        let proxy = |c| SelectionProxy::from_puzzle_and_index(self.puzzle, c);
        let &[loc1, loc2, loc3] = &self.cells[..] else {
            unreachable!();
        };
        let loc1_p = loc1.reflect_loc_about(loc2);
        let loc3_p = loc3.reflect_loc_about(loc2);
        let s1 = proxy(loc1);
        let s2 = proxy(loc3_p);
        let s3 = proxy(loc2);
        let s4 = proxy(loc3);
        let s5 = proxy(loc1_p);

        // let proxies = vec![(loc1, loc2, loc3), (loc1_p, loc2, loc3_p)]
        //     .into_iter()
        //     .map(|(l1, l2, l3)| (proxy(l1), proxy(l2), proxy(l3)))
        //     .inspect(|t| {
        //         info!(
        //             "incoming reflected 3:\n  s1={:?}\n  s2={:?}\n  s3={:?}",
        //             t.0, t.1, t.2
        //         )
        //     })
        //     .collect::<Vec<_>>();
        // proxies.into_iter().flat_map(move |(s1, s2, s3)| {
        self.actions
            .iter()
            .filter_map(move |a| (a)(&s1, &s2, &s3, &s4, &s5))
        // })

        // let proxies = self
        //     .cells
        //     .iter()
        //     .permutations(2)
        //     .map(move |mut locs| {
        //         let (Some(&loc2), Some(&loc1)) = (locs.pop(), locs.pop()) else {
        //             unreachable!()
        //         };
        //         let loc2_p = loc2.reflect_loc_about(loc1);
        //         (proxy(loc2_p), proxy(loc1), proxy(loc2))
        //     })
        //     .inspect(|t| {
        //         info!(
        //             "incoming reflected 2:\n  s1={:?}\n  s2={:?}\n  s3={:?}",
        //             t.0, t.1, t.2
        //         )
        //     })
        //     .filter(|(_, s2, _)| !s2.is_void)
        //     .collect::<Vec<_>>();
        // let proxy = PuzzleProxy(self.puzzle);
        // let actions_iter = self.actions.iter();
        // std::mem::swap(&mut loc1_p.loc.cell_nr, &mut loc3_p.loc.cell_nr);
        //
        //     .into_iter()
        //     .flat_map(move |(loc1, loc2, loc3)| {
        //         actions_iter.clone().filter_map(move |a| {
        //             if (a.when_fn)(proxy, loc1, loc2, loc3) {
        //                 Some((a.then_fn)(loc1))
        //             } else {
        //                 None
        //             }
        //         })
        //     })
    }
}

// struct ImplicationBuilder<'p, 'r, IT> {
//     resolver: &'r mut ImplicationResolver<'p, IT>,
//     when_fn: Wh,
// }

// impl<'p, 'r, R> ImplicationBuilder<'p, 'r, When2, Then2<R>> {
//     fn then2(self, then_fn: Then2<R>) -> &'r mut ImplicationResolver<'p, When2, Then2<R>> {
//         let action = ImplicationAction {
//             when_fn: self.when_fn,
//             then_fn,
//         };
//         self.resolver.actions.push(action);
//         self.resolver
//     }
// }

// impl<'p, 'r, R> ImplicationBuilder<'p, 'r, When3, Then3<R>> {
//     fn then3(self, then_fn: Then3<R>) -> &'r mut ImplicationResolver<'p, When3, Then3<R>> {
//         let action = ImplicationAction {
//             when_fn: self.when_fn,
//             then_fn,
//         };
//         self.resolver.actions.push(action);
//         self.resolver
//     }
// }

impl PuzzleClue for SameColumnClue {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance {
        let mut resolver = ImplicationResolver::new_unit(puzzle);
        resolver.add_loc(self.loc);
        resolver.add_loc(self.loc2());
        if let Some(loc3) = self.loc3() {
            resolver.add_loc(loc3);
        }
        // info!("resolver: {resolver:?}");
        for mut sub_resolver in resolver.iter_all_cols::<IfThen2<_>>() {
            sub_resolver
                .if_then(|l1, l2| {
                    if !l1.is_enabled && l2.is_solo {
                        panic!("contradiction at {l1:?} {l2:?}");
                    }
                    None
                })
                .if_then(|l1, l2| {
                    if l1.is_enabled_not_solo() && l2.is_solo {
                        Some(UpdateCellIndex {
                            index: l1.index,
                            op: UpdateCellIndexOperation::Solo,
                        })
                    } else {
                        None
                    }
                })
                .if_then(|l1, l2| {
                    if l1.is_enabled_not_solo() && !l2.is_enabled {
                        Some(UpdateCellIndex {
                            index: l1.index,
                            op: UpdateCellIndexOperation::Clear,
                        })
                    } else {
                        None
                    }
                });
            // .when2(|p, l1, l2| p.is_enabled_not_solo(l1) && p.is_solo(l2))
            // .then2(|index| )
            // .when2(|p, l1, l2| p.is_enabled_not_solo(l1) && !p.is_enabled(l2))
            // .then2(|index| );
            for ev in sub_resolver.iter_perm_2s() {
                return Some(ev);
            }
            // info!("  sub_resolver: {sub_resolver:?}");
            // for ev in sub_resolver
            //     .when_solo()
            //     .then_solo()
            //     .chain(sub_resolver.when_disabled().then_disable())
            // {
            //     return Some(ev);
            // }
            // for loc in sub_resolver.when_enabled().then(|loc| loc) {
            //     info!("!! contradiction !! @{loc:?}");
            // }
        }
        None
    }

    fn display(&self) {
        todo!()
    }

    fn spawn_into<'s, 'p: 's>(
        &'s self,
        puzzle: &'p Puzzle,
    ) -> Box<dyn FnOnce(&mut ChildBuilder) + 's> {
        Box::new(|builder| {
            let sprite_size = Vec2::new(32., 32.);
            let size_sprite = |mut sprite: Sprite| {
                sprite.custom_size = Some(sprite_size);
                sprite
            };
            let (sprite1, color1) = puzzle.cell_answer_display(self.loc);
            builder
                .spawn((
                    Sprite::from_color(color1, sprite_size),
                    Transform::from_xyz(0., -32., 0.),
                ))
                .with_child((
                    size_sprite(sprite1),
                    Transform::from_xyz(0., 0., 1.),
                    PickingBehavior {
                        should_block_lower: false,
                        is_hoverable: false,
                    },
                ));
            let (sprite2, color2) = puzzle.cell_answer_display(self.loc2());
            builder
                .spawn((
                    Sprite::from_color(color2, sprite_size),
                    Transform::from_xyz(0., 0., 0.),
                ))
                .with_child((
                    size_sprite(sprite2),
                    Transform::from_xyz(0., 0., 1.),
                    PickingBehavior {
                        should_block_lower: false,
                        is_hoverable: false,
                    },
                ));
            if let Some(loc3) = self.loc3() {
                let (sprite3, color3) = puzzle.cell_answer_display(loc3);
                builder
                    .spawn((
                        Sprite::from_color(color3, sprite_size),
                        Transform::from_xyz(0., 32., 0.),
                    ))
                    .with_child((
                        size_sprite(sprite3),
                        Transform::from_xyz(0., 0., 1.),
                        PickingBehavior {
                            should_block_lower: false,
                            is_hoverable: false,
                        },
                    ));
            }
        })
    }
}

#[derive(Debug, Component, Clone, Reflect)]
pub struct AdjacentColumnClue {
    loc1: CellLoc,
    loc2: CellLoc,
}

impl AdjacentColumnClue {
    pub fn new_random<R: Rng>(rng: &mut R, puzzle: &Puzzle) -> Option<Self> {
        let n_rows = puzzle.rows.len();
        let [col1, col2] = rand::seq::index::sample_array(rng, puzzle.max_column)?;
        Some(AdjacentColumnClue {
            loc1: CellLoc {
                row_nr: rng.random_range(0..n_rows),
                cell_nr: col1 as isize,
            },
            loc2: CellLoc {
                row_nr: rng.random_range(0..n_rows),
                cell_nr: col2 as isize,
            },
        })
    }
}

impl PuzzleClue for AdjacentColumnClue {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance {
        let mut resolver = ImplicationResolver::new_unit(puzzle);
        resolver.add_loc(self.loc1);
        resolver.add_loc(self.loc2);
        info!("adjacent resolver: {resolver:#?}");
        for mut sub_resolver in resolver.iter_all_cols::<IfThen3<_>>() {
            info!("adjacent sub resolver: {sub_resolver:#?}");
            sub_resolver
                // .when2(|p, l1, l2| !p.is_enabled(l1) && p.is_solo(l2))
                // .then(|index| panic!("contradiction at {index:?}"))
                .if_then(|l1, l2, l3| {
                    info!("checking adjacent solo\n  l1={l1:?}\n  l2={l2:?}  \n  l3={l3:?}");
                    return None;
                    if l2.is_enabled_not_solo() && (l1.is_solo || l3.is_solo) {
                        Some(UpdateCellIndex {
                            index: l2.index,
                            op: UpdateCellIndexOperation::Solo,
                        })
                    } else {
                        None
                    }
                })
                .if_then(|l1, l2, l3| {
                    info!("checking adjacent enabled\n  l1={l1:?}\n  l2={l2:?}  \n  l3={l3:?}");
                    if l2.is_enabled_not_solo() && !l1.is_enabled && !l3.is_enabled {
                        Some(UpdateCellIndex {
                            index: l2.index,
                            op: UpdateCellIndexOperation::Clear,
                        })
                    } else {
                        None
                    }
                });
            // .then3(|index| );
            for ev in sub_resolver.iter_reflected_2s() {
                return Some(ev);
            }
            // info!("  sub_resolver: {sub_resolver:?}");
            // for ev in sub_resolver
            //     .when_solo()
            //     .then_solo()
            //     .chain(sub_resolver.when_disabled().then_disable())
            // {
            //     return Some(ev);
            // }
            // for loc in sub_resolver.when_enabled().then(|loc| loc) {
            //     info!("!! contradiction !! @{loc:?}");
            // }
        }
        // let mut resolver = ImplicationResolver::new(puzzle);
        // resolver.add_loc(self.loc1);
        // resolver.add_loc(self.loc2);
        // // info!("resolver: {resolver:?}");
        // for sub_resolver in resolver.iter_cols() {
        //     // info!("  sub_resolver: {sub_resolver:?}");
        //     // for ev in sub_resolver
        //     //     .when(|p, l| !p.cell_selection(l.loc).is_enabled(l.index))
        //     //     .then(|index| UpdateCellIndex {
        //     //         index,
        //     //         op: UpdateCellIndexOperation::Clear,
        //     //     })
        //     // {
        //     //     return Some(ev);
        //     // }
        //     // for ev in sub_resolver
        //     //     .when(|p, l| !p.cell_selection(l.loc).is_solo(l.index))
        //     //     .then(|index| UpdateCellIndex {
        //     //         index,
        //     //         op: UpdateCellIndexOperation::Solo,
        //     //     })
        //     // {
        //     //     return Some(ev);
        //     // }
        // }
        None
    }

    fn display(&self) {
        todo!()
    }

    fn spawn_into<'s, 'p: 's>(
        &'s self,
        puzzle: &'p Puzzle,
    ) -> Box<dyn FnOnce(&mut ChildBuilder) + 's> {
        Box::new(|builder| {
            let sprite_size = Vec2::new(32., 32.);
            let size_sprite = |mut sprite: Sprite| {
                sprite.custom_size = Some(sprite_size);
                sprite
            };
            let colspan = self
                .loc1
                .cell_nr
                .abs_diff(self.loc2.cell_nr)
                .saturating_sub(1);
            builder.spawn(Text2d::new(format!("{colspan}")));
            let (sprite1, color1) = puzzle.cell_answer_display(self.loc1);
            builder
                .spawn((
                    Sprite::from_color(color1, sprite_size),
                    Transform::from_xyz(-25., 0., 0.),
                ))
                .with_child((
                    size_sprite(sprite1),
                    Transform::from_xyz(0., 0., 1.),
                    PickingBehavior {
                        should_block_lower: false,
                        is_hoverable: false,
                    },
                ));
            let (sprite2, color2) = puzzle.cell_answer_display(self.loc2);
            builder
                .spawn((
                    Sprite::from_color(color2, sprite_size),
                    Transform::from_xyz(25., 0., 0.),
                ))
                .with_child((
                    size_sprite(sprite2),
                    Transform::from_xyz(0., 0., 1.),
                    PickingBehavior {
                        should_block_lower: false,
                        is_hoverable: false,
                    },
                ));
        })
    }
}

#[derive(Debug, Component, Clone, Reflect)]
pub struct BetweenColumnsClue {
    loc1: CellLoc,
    loc2: CellLoc,
    loc3: CellLoc,
}

impl BetweenColumnsClue {
    pub fn new_random<R: Rng>(rng: &mut R, puzzle: &Puzzle) -> Option<Self> {
        let n_rows = puzzle.rows.len();
        let mut columns: [usize; 3] = rand::seq::index::sample_array(rng, puzzle.max_column)?;
        columns.sort();
        let [col1, col2, col3] = columns;
        Some(BetweenColumnsClue {
            loc1: CellLoc {
                row_nr: rng.random_range(0..n_rows),
                cell_nr: col1 as isize,
            },
            loc2: CellLoc {
                row_nr: rng.random_range(0..n_rows),
                cell_nr: col2 as isize,
            },
            loc3: CellLoc {
                row_nr: rng.random_range(0..n_rows),
                cell_nr: col3 as isize,
            },
        })
    }
}

impl PuzzleClue for BetweenColumnsClue {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance {
        let mut resolver = ImplicationResolver::new_unit(puzzle);
        resolver.add_loc(self.loc1);
        resolver.add_loc(self.loc2);
        resolver.add_loc(self.loc3);
        info!("between resolver: {resolver:?}");
        for mut sub_resolver in resolver.iter_all_cols::<IfThen5<_>>() {
            info!("between sub resolver: {sub_resolver:?}");
            sub_resolver.if_then(|l1, l3p, l2, l3, l1p| {
                info!(
                    "checking between\n l1={l1:?}\n l3p={l3p:?}\n l2={l2:?}\n l3={l3:?}\n \
                     l1p={l1p:?}"
                );
                if l2.is_enabled_not_solo()
                    && !((l1.is_enabled && l3.is_enabled) || (l1p.is_enabled && l3p.is_enabled))
                {
                    Some(UpdateCellIndex {
                        index: l2.index,
                        op: UpdateCellIndexOperation::Clear,
                    })
                } else {
                    None
                }
            });
            // .then3(|index| panic!("contradiction at {index:?}"));
            for ev in sub_resolver.iter_reflected_3s() {
                return Some(ev);
            }
            // info!("  sub_resolver: {sub_resolver:?}");
            // for ev in sub_resolver
            //     .when_solo()
            //     .then_solo()
            //     .chain(sub_resolver.when_disabled().then_disable())
            // {
            //     return Some(ev);
            // }
            // for loc in sub_resolver.when_enabled().then(|loc| loc) {
            //     info!("!! contradiction !! @{loc:?}");
            // }
        }
        None
    }

    fn display(&self) {
        todo!()
    }

    fn spawn_into<'s, 'p: 's>(
        &'s self,
        puzzle: &'p Puzzle,
    ) -> Box<dyn FnOnce(&mut ChildBuilder) + 's> {
        Box::new(|builder| {
            let sprite_size = Vec2::new(32., 32.);
            let size_sprite = |mut sprite: Sprite| {
                sprite.custom_size = Some(sprite_size);
                sprite
            };
            let (sprite1, color1) = puzzle.cell_answer_display(self.loc1);
            builder
                .spawn((
                    Sprite::from_color(color1, sprite_size),
                    Transform::from_xyz(-32., 0., 0.),
                ))
                .with_child((
                    size_sprite(sprite1),
                    Transform::from_xyz(0., 0., 1.),
                    PickingBehavior {
                        should_block_lower: false,
                        is_hoverable: false,
                    },
                ));
            let (sprite2, color2) = puzzle.cell_answer_display(self.loc2);
            builder
                .spawn((
                    Sprite::from_color(color2, sprite_size),
                    Transform::from_xyz(0., 0., -1.),
                ))
                .with_child((
                    size_sprite(sprite2),
                    Transform::from_xyz(0., 0., 1.),
                    PickingBehavior {
                        should_block_lower: false,
                        is_hoverable: false,
                    },
                ));
            let (sprite3, color3) = puzzle.cell_answer_display(self.loc3);
            builder
                .spawn((
                    Sprite::from_color(color3, sprite_size),
                    Transform::from_xyz(32., 0., 0.),
                ))
                .with_child((
                    size_sprite(sprite3),
                    Transform::from_xyz(0., 0., 1.),
                    PickingBehavior {
                        should_block_lower: false,
                        is_hoverable: false,
                    },
                ));
        })
    }
}
