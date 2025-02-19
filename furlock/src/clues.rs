use bevy::prelude::*;
use rand::Rng;

use crate::{
    puzzle::{CellLoc, CellLocIndex, Puzzle, UpdateCellIndexOperation},
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

struct ImplicationResolver<'p, W, R> {
    puzzle: &'p Puzzle,
    cells: Vec<CellLocIndex>,
    actions: Vec<ImplicationAction<W, R>>,
}

impl<'p, W, R> std::fmt::Debug for ImplicationResolver<'p, W, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImplicationResolver")
            .field("puzzle", &(self.puzzle as *const Puzzle as usize))
            .field("cells", &self.cells)
            .field("actions", &self.actions)
            .finish()
    }
}

#[derive(Copy, Clone)]
struct PuzzleProxy<'p>(&'p Puzzle);

impl<'p> PuzzleProxy<'p> {
    fn is_enabled(&self, index: CellLocIndex) -> bool {
        self.0.cell_selection(index.loc).is_enabled(index.index)
    }
    fn is_solo(&self, index: CellLocIndex) -> bool {
        self.0.cell_selection(index.loc).is_solo(index.index)
    }
    fn is_enabled_not_solo(&self, index: CellLocIndex) -> bool {
        let sel = self.0.cell_selection(index.loc);
        sel.is_enabled(index.index) && !sel.is_solo(index.index)
    }
}

type When2 = fn(PuzzleProxy, CellLocIndex, CellLocIndex) -> bool;
type When3 = fn(PuzzleProxy, CellLocIndex, CellLocIndex, CellLocIndex) -> bool;
type Then<R> = fn(CellLocIndex) -> R;

struct ImplicationAction<W, R> {
    when_fn: W,
    then_fn: Then<R>,
}

impl<W, R> std::fmt::Debug for ImplicationAction<W, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImplicationAction").finish()
    }
}

#[derive(Debug, Clone, Copy)]
struct ImplicationWidth {
    min: isize,
    max: isize,
    cellspan: usize,
}

impl<'p> ImplicationResolver<'p, (), ()> {
    fn new_unit(puzzle: &'p Puzzle) -> Self {
        ImplicationResolver {
            puzzle,
            cells: Vec::default(),
            actions: Vec::default(),
        }
    }
}

impl<'p, W, R> ImplicationResolver<'p, W, R> {
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

    fn iter_all_cols<U, S>(&self) -> impl Iterator<Item = ImplicationResolver<U, S>> {
        let width = self.width();
        (-(width.cellspan as isize)..self.puzzle.max_column as isize).map(move |shift| {
            let cells = self
                .cells
                .iter()
                .map(|&c| self.puzzle.shift_loc_index(c, shift))
                .collect();
            ImplicationResolver {
                cells,
                actions: Vec::default(),
                puzzle: self.puzzle,
            }
        })
    }

    fn iter_cols<U, S>(&self) -> impl Iterator<Item = ImplicationResolver<U, S>> {
        let width = self.width();
        (0..(self.puzzle.max_column - width.cellspan) as isize).map(move |offset| {
            let mut cells = self.cells.clone();
            for cell in &mut cells {
                cell.loc.cell_nr += offset - width.min;
            }
            ImplicationResolver {
                cells,
                actions: Vec::default(),
                puzzle: self.puzzle,
            }
        })
    }
}

impl<'p, R> ImplicationResolver<'p, When2, R> {
    fn when2(&mut self, when_fn: When2) -> ImplicationBuilder<'p, '_, When2, R> {
        ImplicationBuilder {
            resolver: self,
            when_fn,
        }
    }

    // fn when_enabled(&self) -> ImplicationBuilder<impl Fn(&Puzzle, CellLocIndex) -> bool> {
    //     self.when(|p, l| p.cell_selection(l.loc).is_enabled(l.index))
    // }

    // fn when_disabled(&self) -> ImplicationBuilder<impl Fn(&Puzzle, CellLocIndex) -> bool> {
    //     self.when(|p, l| !p.cell_selection(l.loc).is_enabled(l.index))
    // }

    // fn when_solo(&self) -> ImplicationBuilder<impl Fn(&Puzzle, CellLocIndex) -> bool> {
    //     self.when(|p, l| p.cell_selection(l.loc).is_solo(l.index))
    // }

    fn iter_perm_2s(&self) -> impl Iterator<Item = R> + use<'_, R> {
        use itertools::Itertools;
        let proxy = PuzzleProxy(self.puzzle);
        let actions_iter = self.actions.iter();
        self.cells.iter().permutations(2).flat_map(move |locs| {
            let &[&loc1, &loc2] = &locs[..] else {
                unreachable!();
            };
            actions_iter.clone().filter_map(move |a| {
                if (a.when_fn)(proxy, loc1, loc2) {
                    Some((a.then_fn)(loc1))
                } else {
                    None
                }
            })
        })
    }
}

impl<'p, R> ImplicationResolver<'p, When3, R> {
    fn when3(&mut self, when_fn: When3) -> ImplicationBuilder<'p, '_, When3, R> {
        ImplicationBuilder {
            resolver: self,
            when_fn,
        }
    }

    fn iter_reflected_2s(&self) -> impl Iterator<Item = R> + use<'_, R> {
        use itertools::Itertools;
        let proxy = PuzzleProxy(self.puzzle);
        let actions_iter = self.actions.iter();
        self.cells
            .iter()
            .permutations(2)
            .filter(|locs| !locs[0].loc.is_void())
            .flat_map(move |locs| {
                let &[&loc1, &loc2] = &locs[..] else {
                    unreachable!();
                };
                let loc2_p = self.puzzle.reflect_loc_index_about(loc2, loc1);
                actions_iter.clone().filter_map(move |a| {
                    if (a.when_fn)(proxy, loc2_p, loc1, loc2) {
                        Some((a.then_fn)(loc1))
                    } else {
                        None
                    }
                })
            })
    }

    fn iter_reflected_3s(&self) -> impl Iterator<Item = R> + use<'_, R> {
        let proxy = PuzzleProxy(self.puzzle);
        let actions_iter = self.actions.iter();
        let &[loc1, loc2, loc3] = &self.cells[..] else {
            unreachable!();
        };
        let mut loc1_p = self.puzzle.reflect_loc_index_about(loc1, loc2);
        let mut loc3_p = self.puzzle.reflect_loc_index_about(loc3, loc2);
        std::mem::swap(&mut loc1_p.loc.cell_nr, &mut loc3_p.loc.cell_nr);
        vec![(loc1, loc2, loc3), (loc1_p, loc2, loc3_p)]
            .into_iter()
            .flat_map(move |(loc1, loc2, loc3)| {
                actions_iter.clone().filter_map(move |a| {
                    if (a.when_fn)(proxy, loc1, loc2, loc3) {
                        Some((a.then_fn)(loc1))
                    } else {
                        None
                    }
                })
            })
    }
}

struct ImplicationBuilder<'p, 'r, W, R> {
    resolver: &'r mut ImplicationResolver<'p, W, R>,
    when_fn: W,
}

impl<'p, 'r, W, R> ImplicationBuilder<'p, 'r, W, R> {
    fn then(self, then_fn: Then<R>) -> &'r mut ImplicationResolver<'p, W, R> {
        let action = ImplicationAction {
            when_fn: self.when_fn,
            then_fn,
        };
        self.resolver.actions.push(action);
        self.resolver
    }

    // fn then_disable<'s: 'r>(&'s self) -> impl Iterator<Item = UpdateCellIndex> + 'r {
    //     self.then(|index| UpdateCellIndex {
    //         index,
    //         op: UpdateCellIndexOperation::Clear,
    //     })
    // }

    // fn then_solo<'s: 'r>(&'s self) -> impl Iterator<Item = UpdateCellIndex> + 'r {
    //     self.then(|index| UpdateCellIndex {
    //         index,
    //         op: UpdateCellIndexOperation::Solo,
    //     })
    // }
}

impl PuzzleClue for SameColumnClue {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance {
        let mut resolver = ImplicationResolver::new_unit(puzzle);
        resolver.add_loc(self.loc);
        resolver.add_loc(self.loc2());
        if let Some(loc3) = self.loc3() {
            resolver.add_loc(loc3);
        }
        // info!("resolver: {resolver:?}");
        for mut sub_resolver in resolver.iter_cols() {
            sub_resolver
                .when2(|p, l1, l2| !p.is_enabled(l1) && p.is_solo(l2))
                .then(|index| panic!("contradiction at {index:?}"))
                .when2(|p, l1, l2| p.is_enabled_not_solo(l1) && p.is_solo(l2))
                .then(|index| UpdateCellIndex {
                    index,
                    op: UpdateCellIndexOperation::Solo,
                })
                .when2(|p, l1, l2| p.is_enabled_not_solo(l1) && !p.is_enabled(l2))
                .then(|index| UpdateCellIndex {
                    index,
                    op: UpdateCellIndexOperation::Clear,
                });
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
        for mut sub_resolver in resolver.iter_all_cols() {
            info!("adjacent sub resolver: {sub_resolver:#?}");
            sub_resolver
                // .when2(|p, l1, l2| !p.is_enabled(l1) && p.is_solo(l2))
                // .then(|index| panic!("contradiction at {index:?}"))
                .when3(|p, l1, l2, l3| {
                    info!("checking adjacent solo\n  l1={l1:?}\n  l2={l2:?}  \n  l3={l3:?}");
                    p.is_enabled_not_solo(l2) && (p.is_solo(l1) || p.is_solo(l3))
                })
                .then(|index| UpdateCellIndex {
                    index,
                    op: UpdateCellIndexOperation::Solo,
                })
                .when3(|p, l1, l2, l3| {
                    info!("checking adjacent enabled\n  l1={l1:?}\n  l2={l2:?}  \n  l3={l3:?}");
                    p.is_enabled_not_solo(l2) && !p.is_enabled(l1) && !p.is_enabled(l3)
                })
                .then(|index| UpdateCellIndex {
                    index,
                    op: UpdateCellIndexOperation::Clear,
                });
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
        for mut sub_resolver in resolver.iter_all_cols() {
            info!("between sub resolver: {sub_resolver:?}");
            sub_resolver
                .when3(|p, l1, l2, l3| {
                    info!("checking between {l1:?} {l2:?} {l3:?}");
                    false
                })
                .then(|index| panic!("contradiction at {index:?}"));
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
