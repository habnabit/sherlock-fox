use bevy::prelude::*;
use rand::Rng;

use crate::puzzle::{CellLoc, Puzzle};

pub trait PuzzleClue: std::fmt::Debug {
    fn advance_puzzle(&self, puzzle: &Puzzle);
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
}

impl PuzzleClue for SameColumnClue {
    fn advance_puzzle(&self, puzzle: &Puzzle) {
        todo!()
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
            let (sprite2, color2) = puzzle.cell_answer_display(CellLoc {
                row_nr: self.row2,
                ..self.loc
            });
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
            if let Some(row3) = self.row3 {
                let (sprite3, color3) = puzzle.cell_answer_display(CellLoc {
                    row_nr: row3,
                    ..self.loc
                });
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
    fn advance_puzzle(&self, puzzle: &Puzzle) {
        todo!()
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
    fn advance_puzzle(&self, puzzle: &Puzzle) {
        todo!()
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
