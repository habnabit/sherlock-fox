use bevy::{prelude::*, utils::HashMap};
use rand::Rng;

use crate::{
    puzzle::{CellLoc, CellLocIndex, Puzzle, UpdateCellIndexOperation},
    UpdateCellIndex,
};

pub type PuzzleAdvance = Option<UpdateCellIndex>;

#[derive(Debug, Reflect, Clone)]
pub struct ClueExplanationPayload {}

impl ClueExplanationPayload {
    pub fn lookup<T>(&self) -> &T {
        todo!()
    }
}

#[derive(Debug, Reflect, Clone)]
pub struct ClueExplanation {
    #[reflect(ignore)]
    chunks: &'static [ClueExplanationChunk],
    payload: ClueExplanationPayload,
}

impl From<(&Loc2, &'static [ClueExplanationChunk])> for ClueExplanation {
    fn from((loc, chunks): (&Loc2, &'static [ClueExplanationChunk])) -> Self {
        let mut named_cells = HashMap::new();
        named_cells.insert("loc1", loc.loc1.clone());
        named_cells.insert("loc2", loc.loc2.clone());
        ClueExplanation {
            chunks,
            payload: ClueExplanationPayload {},
        }
    }
}

pub trait CellDisplay {}

#[derive(Debug, Reflect, Clone, Copy)]
pub enum ClueExplanationChunk {
    Text(&'static str),
    NamedCell(&'static str),
    Accessor(fn(&ClueExplanationPayload) -> &dyn CellDisplay),
}

pub trait PuzzleClue: std::fmt::Debug {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance;
    fn spawn_into<'s, 'p: 's>(
        &'s self,
        puzzle: &'p Puzzle,
    ) -> Box<dyn FnOnce(&mut ChildBuilder) + 's>;
}

#[derive(Reflect, Asset, Debug)]
#[reflect(from_reflect = false)]
pub struct DynPuzzleClue(#[reflect(ignore)] Box<(dyn PuzzleClue + Sync + Send + 'static)>);

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

#[derive(Clone, Reflect, Debug)]
struct SelectionProxy {
    index_: CellLocIndex,
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
            index_: index,
            is_enabled,
            is_solo,
            is_void,
        }
    }

    fn is_enabled_not_solo(&self) -> bool {
        self.is_enabled && !self.is_solo
    }
}

impl std::ops::Deref for SelectionProxy {
    type Target = CellLocIndex;

    fn deref(&self) -> &Self::Target {
        &self.index_
    }
}

#[derive(Debug, Clone)]
struct Loc2 {
    loc1: SelectionProxy,
    loc2: SelectionProxy,
}

#[derive(Debug, Clone)]
struct Loc2Mirrored {
    loc1: SelectionProxy,
    loc2: SelectionProxy,
    loc2_p: SelectionProxy,
}

#[derive(Debug, Clone)]
struct Loc3 {
    loc1: SelectionProxy,
    loc2: SelectionProxy,
    loc3: SelectionProxy,
}

#[derive(Debug, Clone)]
struct Loc3Mirrored {
    loc1: SelectionProxy,
    loc2: SelectionProxy,
    loc2_p: SelectionProxy,
    loc3: SelectionProxy,
    loc3_p: SelectionProxy,
}

impl Loc3Mirrored {
    fn as_3s(&self) -> (Loc3, Loc3) {
        (
            Loc3 {
                loc1: self.loc1.clone(),
                loc2: self.loc2.clone(),
                loc3: self.loc3.clone(),
            },
            (Loc3 {
                loc1: self.loc1.clone(),
                loc2: self.loc2_p.clone(),
                loc3: self.loc3_p.clone(),
            }),
        )
    }

    fn both_3s(&self, predicate: fn(&Loc3) -> bool) -> bool {
        let my_3s = self.as_3s();
        predicate(&my_3s.0) && predicate(&my_3s.1)
    }

    fn eval_as_3s<R>(&self, predicate: fn(&Loc3) -> Option<R>) -> Option<R> {
        todo!()
    }
}

type IfThen<L, R> = fn(&L) -> Option<R>;

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

impl<'p, R> ImplicationResolver<'p, IfThen<Loc2, R>> {
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
                let (Some(loc2), Some(loc1)) = (sels.pop(), sels.pop()) else {
                    unreachable!()
                };
                let loc = Loc2 { loc1, loc2 };
                // info!("incoming perm 2:\n  loc={loc:?}");
                actions_iter.clone().filter_map(move |a| (a)(&loc))
            })
    }
}

impl<'p, R> ImplicationResolver<'p, IfThen<Loc2Mirrored, R>> {
    fn iter_reflected_2s(&self) -> impl Iterator<Item = R> + use<'_, R> {
        use itertools::Itertools;
        let proxy = |c| SelectionProxy::from_puzzle_and_index(self.puzzle, c);
        let proxies = self
            .cells
            .iter()
            .map(|c| proxy(*c))
            .permutations(2)
            .map(move |mut locs| {
                let (Some(loc2), Some(loc1)) = (locs.pop(), locs.pop()) else {
                    unreachable!()
                };
                let loc2_p = proxy(loc2.reflect_loc_about(&loc1));
                Loc2Mirrored { loc1, loc2, loc2_p }
            })
            // .inspect(|l| info!("incoming reflected 2:\n  loc={l:?}",))
            .filter(|l| !l.loc1.is_void)
            .collect::<Vec<_>>();
        let actions_iter = self.actions.iter();
        proxies
            .into_iter()
            .flat_map(move |loc| actions_iter.clone().filter_map(move |a| (a)(&loc)))
    }
}

impl<'p, R> ImplicationResolver<'p, IfThen<Loc3Mirrored, R>> {
    fn iter_reflected_3s(&self) -> impl Iterator<Item = R> + use<'_, R> {
        use itertools::Itertools;
        let proxy = |c| SelectionProxy::from_puzzle_and_index(self.puzzle, c);
        let proxies = self
            .cells
            .iter()
            .map(|c| proxy(*c))
            .permutations(3)
            .map(move |mut locs| {
                let (Some(loc3), Some(loc2), Some(loc1)) = (locs.pop(), locs.pop(), locs.pop())
                else {
                    unreachable!()
                };
                let loc2_p = proxy(loc2.reflect_loc_about(&loc1));
                let loc3_p = proxy(loc3.reflect_loc_about(&loc1));
                Loc3Mirrored {
                    loc1,
                    loc2,
                    loc2_p,
                    loc3,
                    loc3_p,
                }
            })
            // .inspect(|l| info!("incoming reflected 3:\n  loc={l:#?}",))
            .filter(|l| !l.loc1.is_void)
            .collect::<Vec<_>>();

        let actions_iter = self.actions.iter();
        proxies
            .into_iter()
            .flat_map(move |loc| actions_iter.clone().filter_map(move |a| (a)(&loc)))
    }
}

macro_rules! explanation {
    ( [] , $( $accum:tt )* ) => {
        &[ $($accum)* ]
    };
    ( [% { $name:ident } , $( $rest:tt )*] , $( $accum:tt )* ) => {
        explanation!([$($rest)*] , $($accum)* ClueExplanationChunk::NamedCell(stringify!($name)), )
    };
    ( [$text:expr , $( $rest:tt )*] , $( $accum:tt )* ) => {
        explanation!([$($rest)*] , $($accum)* ClueExplanationChunk::Text($text), )
    };
    ( $( $rest:tt )* ) => {
        explanation!( [$($rest)*] , )
    };
}

static SAME_COLUMN_SOLO: &[ClueExplanationChunk] = explanation![
    %{loc2}, " is selected, therefore ", %{loc1}, " must be selected.",
];

static SAME_COLUMN_CLEAR: &[ClueExplanationChunk] = explanation![
    %{loc2}, " is not possible, therefore ", %{loc1}, " must be impossible.",
];

// trace_macros!(false);

// static SAME_COLUMN_CLEAR: &[ClueExplanationChunk] = &[
//     ClueExplanationChunk::Text("same column clear"),
// ];

impl PuzzleClue for SameColumnClue {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance {
        let mut resolver = ImplicationResolver::new_unit(puzzle);
        resolver.add_loc(self.loc);
        resolver.add_loc(self.loc2());
        if let Some(loc3) = self.loc3() {
            resolver.add_loc(loc3);
        }
        for mut sub_resolver in resolver.iter_all_cols::<IfThen<_, _>>() {
            sub_resolver
                .if_then(|Loc2 { loc1: l1, loc2: l2 }| {
                    if !l1.is_enabled && l2.is_solo {
                        panic!("contradiction at {l1:?} {l2:?}");
                    }
                    None
                })
                .if_then(|l: &Loc2| {
                    if l.loc1.is_enabled_not_solo() && l.loc2.is_solo {
                        Some(l.loc1.as_solo().with_explanation((l, SAME_COLUMN_SOLO)))
                    } else {
                        None
                    }
                })
                .if_then(|l: &Loc2| {
                    if l.loc1.is_enabled_not_solo() && !l.loc2.is_enabled {
                        Some(l.loc1.as_clear().with_explanation((l, SAME_COLUMN_CLEAR)))
                    } else {
                        None
                    }
                });
            for ev in sub_resolver.iter_perm_2s() {
                return Some(ev);
            }
        }
        None
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
        // info!("adjacent resolver: {resolver:#?}");
        for mut sub_resolver in resolver.iter_all_cols::<IfThen<_, _>>() {
            // info!("adjacent sub resolver: {sub_resolver:#?}");
            sub_resolver
                .if_then(
                    |Loc2Mirrored {
                         loc1: l1,
                         loc2: l2,
                         loc2_p: l2p,
                     }| {
                        // info!("checking adjacent solo\n  l1={l1:?}\n  l2={l2:?}  \n  l3={l2p:?}");
                        return None;
                        if l1.is_enabled_not_solo() && (l2.is_solo || l2p.is_solo) {
                            Some(l1.as_solo())
                        } else {
                            None
                        }
                    },
                )
                .if_then(
                    |Loc2Mirrored {
                         loc1: l1,
                         loc2: l2,
                         loc2_p: l2p,
                     }| {
                        // info!(
                        //     "checking adjacent enabled\n  l1={l1:?}\n  l2={l2:?}  \n  l3={l2p:?}"
                        // );
                        if l1.is_enabled_not_solo() && !l2.is_enabled && !l2p.is_enabled {
                            Some(l1.as_clear())
                        } else {
                            None
                        }
                    },
                );
            for ev in sub_resolver.iter_reflected_2s() {
                return Some(ev);
            }
        }
        None
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
        // info!("between resolver: {resolver:?}");
        for mut sub_resolver in resolver.iter_all_cols::<IfThen<_, _>>() {
            // info!("between sub resolver: {sub_resolver:?}");
            sub_resolver.if_then(|l: &Loc3Mirrored| {
                // info!("checking between\n l={l:#?}");
                if l.loc1.is_enabled_not_solo()
                    && l.both_3s(|sl| !sl.loc2.is_enabled || !sl.loc3.is_enabled)
                {
                    Some(l.loc1.as_clear())
                } else {
                    None
                }
            });
            for ev in sub_resolver.iter_reflected_3s() {
                return Some(ev);
            }
        }
        None
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
