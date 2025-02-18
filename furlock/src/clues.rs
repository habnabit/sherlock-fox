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
        let cell_nr = rng.random_range(0..puzzle.max_column);
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

#[derive(Clone)]
struct ImplicationResolver<'p> {
    puzzle: &'p Puzzle,
    cells: Vec<CellLocIndex>,
}

impl<'p> std::fmt::Debug for ImplicationResolver<'p> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImplicationResolver")
            .field("puzzle", &(self.puzzle as *const Puzzle as usize))
            .field("cells", &self.cells)
            .finish()
    }
}

struct ImplicationWidth {
    min: usize,
    max: usize,
    cellspan: usize,
}

impl<'p> ImplicationResolver<'p> {
    fn new(puzzle: &'p Puzzle) -> Self {
        ImplicationResolver {
            puzzle,
            cells: Vec::default(),
        }
    }

    fn add_loc(&mut self, loc: CellLoc) {
        let index = self.puzzle.cell_answer_index(loc);
        self.cells.push(CellLocIndex { loc, index });
    }

    fn width(&self) -> ImplicationWidth {
        use itertools::{Itertools, MinMaxResult::*};
        let (min, max) = match self
            .cells
            .iter()
            .map(
                |CellLocIndex {
                     loc: CellLoc { cell_nr, .. },
                     ..
                 }| *cell_nr,
            )
            .minmax()
        {
            OneElement(c) => (c, c),
            MinMax(a, b) => (a, b),
            NoElements => unreachable!(),
        };
        ImplicationWidth {
            min,
            max,
            cellspan: max - min,
        }
    }

    fn iter_cols(&self) -> impl Iterator<Item = ImplicationResolver> {
        let width = self.width();
        (0..self.puzzle.max_column - width.cellspan).map(move |offset| {
            let mut cells = self.cells.clone();
            for cell in &mut cells {
                cell.loc.cell_nr = cell.loc.cell_nr + offset - width.min;
            }
            ImplicationResolver { cells, ..*self }
        })
    }

    fn when<'r, W: Fn(&Puzzle, CellLocIndex) -> bool>(
        &'r self,
        when_fn: W,
    ) -> ImplicationBuilder<'r, W> {
        ImplicationBuilder {
            resolver: self,
            when_fn,
        }
    }
}

struct ImplicationBuilder<'r, W> {
    resolver: &'r ImplicationResolver<'r>,
    when_fn: W,
}

impl<'r, W> ImplicationBuilder<'r, W>
where
    W: Fn(&Puzzle, CellLocIndex) -> bool,
{
    fn then<'s: 'r, R, T: Fn(CellLocIndex) -> R + 'r>(
        &'s self,
        then_fn: T,
    ) -> impl Iterator<Item = R> + 'r {
        use itertools::Itertools;
        self.resolver
            .cells
            .iter()
            .permutations(2)
            .filter_map(move |locs| {
                let &[&loc1, &loc2] = &locs[..] else {
                    return None;
                };
                if !(self.when_fn)(self.resolver.puzzle, loc1)
                    && (self.when_fn)(self.resolver.puzzle, loc2)
                {
                    return Some(then_fn(loc1));
                }
                None
            })
    }
}

impl PuzzleClue for SameColumnClue {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance {
        let mut resolver = ImplicationResolver::new(puzzle);
        resolver.add_loc(self.loc);
        resolver.add_loc(self.loc2());
        if let Some(loc3) = self.loc3() {
            resolver.add_loc(loc3);
        }
        // info!("resolver: {resolver:?}");
        for sub_resolver in resolver.iter_cols() {
            // info!("  sub_resolver: {sub_resolver:?}");
            for ev in sub_resolver
                .when(|p, l| !p.cell_selection(l.loc).enabled.contains(l.index))
                .then(|index| UpdateCellIndex {
                    index,
                    op: UpdateCellIndexOperation::Clear,
                })
            {
                return Some(ev);
            }
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
                cell_nr: col1,
            },
            loc2: CellLoc {
                row_nr: rng.random_range(0..n_rows),
                cell_nr: col2,
            },
        })
    }
}

impl PuzzleClue for AdjacentColumnClue {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance {
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
                cell_nr: col1,
            },
            loc2: CellLoc {
                row_nr: rng.random_range(0..n_rows),
                cell_nr: col2,
            },
            loc3: CellLoc {
                row_nr: rng.random_range(0..n_rows),
                cell_nr: col3,
            },
        })
    }
}

impl PuzzleClue for BetweenColumnsClue {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance {
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
