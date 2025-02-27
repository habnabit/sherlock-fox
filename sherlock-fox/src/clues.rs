// © 2025 <_@habnab.it>
//
// SPDX-License-Identifier: EUPL-1.2

use bevy::{prelude::*, utils::HashMap};
use rand::{seq::SliceRandom, Rng};
use typemap::ShareCloneMap;

use crate::{
    puzzle::{
        CellLoc, CellLocAnswer, CellLocIndex, LAns, LCol, LColspan, LRow, Puzzle, RowAnswer,
        RowIndexed,
    },
    UpdateCellIndex, NO_PICK,
};

pub type PuzzleAdvance = Option<UpdateCellIndex>;

#[repr(transparent)]
struct StoredItem<T>(T);

#[derive(Clone)]
pub struct ClueExplanationPayload {
    // #[reflect(ignore)]
    stored: typemap::ShareCloneMap,
}

impl Default for ClueExplanationPayload {
    fn default() -> Self {
        Self {
            stored: ShareCloneMap::custom(),
        }
    }
}

impl std::fmt::Debug for ClueExplanationPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClueExplanationPayload")
            .field("stored", &self.stored.len())
            .finish()
    }
}

impl ClueExplanationPayload {
    pub fn lookup<T: PayloadType>(&self) -> Option<&T> {
        self.stored.get::<StoredItem<T>>()
    }
}

pub trait PayloadType: Send + Sync + Clone + 'static {}

impl<T: PayloadType> typemap::Key for StoredItem<T> {
    type Value = T;
}

#[derive(Debug, Reflect, Clone)]
pub struct ClueExplanation {
    #[reflect(ignore)]
    chunks: &'static [ClueExplanationChunk],
    #[reflect(ignore)]
    payload: ClueExplanationPayload,
}

impl ClueExplanation {
    pub fn resolved(&self) -> impl Iterator<Item = ClueExplanationResolvedChunk> {
        use ClueExplanationChunk as Ch;
        use ClueExplanationResolvedChunk as ResCh;
        self.chunks.iter().map(|c| match c {
            &Ch::Text(s) => ResCh::Text(s),
            &Ch::Accessor(name, f) => {
                ResCh::Accessed(name, f(&self.payload).unwrap_or(&FailedToAccess))
            }
            &Ch::Eval(expr, f) => ResCh::Eval(
                expr,
                f(&self.payload).unwrap_or_else(|| FailedToAccess.as_cell_display_string()),
            ),
        })
    }
}

macro_rules! impl_clue_explanation {
    ( $( $t:ty , )* ) => {
        $(

            impl PayloadType for $t {}
            impl From<(& $t, &'static [ClueExplanationChunk])> for ClueExplanation {
                fn from((loc, chunks): (& $t, &'static [ClueExplanationChunk])) -> Self {
                    let mut payload = ClueExplanationPayload::default();
                    payload.stored.insert::<StoredItem<$t>>(loc.clone());
                    ClueExplanation { chunks, payload }
                }
            }

        )*
    };
}

impl_clue_explanation! {
    Loc2, Loc2Mirrored, Loc3,
}

// impl From<(&Loc2, &'static [ClueExplanationChunk])> for ClueExplanation {
//     fn from((loc, chunks): (&Loc2, &'static [ClueExplanationChunk])) -> Self {
//         let mut payload = ClueExplanationPayload::default();
//         payload.stored.insert::<StoredItem<Loc2>>(loc.clone());
//         ClueExplanation { chunks, payload }
//     }
// }

pub trait CellDisplay: std::fmt::Debug {
    fn as_cell_display_string(&self) -> String;
    fn spawn_into(&self, puzzle: &Puzzle, parent: &mut ChildBuilder);
    fn loc_index(&self) -> Option<&CellLocIndex> {
        None
    }
}

#[derive(Debug, Reflect, Clone, Copy)]
pub struct FailedToAccess;

impl CellDisplay for FailedToAccess {
    fn as_cell_display_string(&self) -> String {
        "<<<?>>>".into()
    }

    fn spawn_into(&self, _puzzle: &Puzzle, parent: &mut ChildBuilder) {
        parent.spawn((
            Node {
                width: Val::Px(32.),
                height: Val::Px(32.),
                margin: UiRect::horizontal(Val::Px(5.)),
                ..Default::default()
            },
            BackgroundColor(Color::hsla(0., 0., 0., 1.)),
        ));
    }
}

#[derive(Debug, Reflect, Clone, Copy)]
pub enum ClueExplanationChunk {
    Text(&'static str),
    Accessor(
        &'static str,
        fn(&ClueExplanationPayload) -> Option<&dyn CellDisplay>,
    ),
    Eval(&'static str, fn(&ClueExplanationPayload) -> Option<String>),
}

#[derive(Debug, Reflect, Clone)]
pub enum ClueExplanationResolvedChunk<'d> {
    Text(&'static str),
    Accessed(&'static str, &'d dyn CellDisplay),
    Eval(&'static str, String),
}

pub trait PuzzleClue: std::fmt::Debug {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance;
    fn spawn_into(
        &self,
        parent: &mut ChildBuilder,
        puzzle: &Puzzle,
        cells: &mut HashMap<RowAnswer, Entity>,
    );
}

#[derive(Reflect, Asset, Debug)]
#[reflect(from_reflect = false)]
pub struct DynPuzzleClue(#[reflect(ignore)] Box<(dyn PuzzleClue + Sync + Send + 'static)>);

impl FromReflect for DynPuzzleClue {
    fn from_reflect(_reflect: &dyn PartialReflect) -> Option<Self> {
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
    row2: LRow,
    row3: Option<LRow>,
}

impl SameColumnClue {
    pub fn new_random<R: Rng>(rng: &mut R, puzzle: &Puzzle) -> Option<Self> {
        let mut rows = puzzle.shuffled_rows(rng).into_iter();
        let first_row = rows.next()?;
        let loc = CellLoc {
            row: first_row,
            col: puzzle.random_column(rng),
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
            row: self.row2,
            ..self.loc
        }
    }

    fn loc3(&self) -> Option<CellLoc> {
        try {
            CellLoc {
                row: self.row3?,
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

impl CellDisplay for SelectionProxy {
    fn as_cell_display_string(&self) -> String {
        format!(
            "Cell[{:?} {:?} {:?}]",
            self.loc.row, self.loc.col, self.index
        )
    }

    fn spawn_into(&self, puzzle: &Puzzle, parent: &mut ChildBuilder) {
        let (mut image_node, color) = puzzle.cell_index_display(self.index_);
        // let button_size = Vec2::new(32., 32.);
        // sprite.custom_size = Some(button_size - Vec2::new(5., 5.));
        image_node.color = Color::hsla(0., 0., 1., 1.);
        parent
            .spawn((
                Node {
                    width: Val::Px(42.),
                    height: Val::Px(42.),
                    margin: UiRect::horizontal(Val::Px(5.)),
                    padding: UiRect::all(Val::Px(5.)),
                    ..Default::default()
                },
                BackgroundColor(color),
                NO_PICK,
            ))
            .with_child((Node::default(), image_node, NO_PICK));
    }

    fn loc_index(&self) -> Option<&CellLocIndex> {
        Some(&self.index_)
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

impl Loc2Mirrored {
    pub fn colspan(&self) -> usize {
        self.loc1.loc.columns_between(&self.loc2.loc)
    }
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

    fn eval_as_3s<R>(&self, eval: fn(&Loc3) -> Option<R>) -> Option<R> {
        let my_3s = self.as_3s();
        eval(&my_3s.0).or_else(|| eval(&my_3s.1))
    }
}

type IfThen<L, R> = fn(&L) -> Option<R>;

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
    fn add_answer(&mut self, loc: CellLoc) {
        self.cells.push(self.puzzle.answer_at(loc).decay_to_ind());
    }

    fn colspan(&self) -> LColspan {
        use itertools::Itertools;
        self.cells.iter().map(|i| i.loc.col).minmax().into()
    }

    fn iter_all_cols<IT2>(&self) -> impl Iterator<Item = ImplicationResolver<IT2>> {
        let colspan = self.colspan();
        self.puzzle.iter_col_shift(colspan).map(move |shift| {
            let cells = self.cells.iter().map(|&c| c.shift_column(shift)).collect();
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
            // TODO: permutations is too many.. we don't need [1, 2, 3] and [1, 3, 2]
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

// fn f() {
//     ClueExplanationChunk::Accessor(|p| &p.lookup::<Loc2>().loc1);
// }

macro_rules! explanation {
    ( $typ:ty : [] , $( $accum:tt )* ) => {
        &[ $($accum)* ]
    };
    ( $typ:ty : [* {|$p:pat_param| $e:expr } , $( $rest:tt )*] , $( $accum:tt )* ) => {
        explanation!(
            $typ: [$($rest)*] ,
            $($accum)*
            ClueExplanationChunk::Eval(
                stringify!($e),
                |p| p.lookup::<$typ>().map(|$p| $e),
            ),
        )
    };
    ( $typ:ty : [% { $name:ident } , $( $rest:tt )*] , $( $accum:tt )* ) => {
        explanation!(
            $typ: [$($rest)*] ,
            $($accum)*
            ClueExplanationChunk::Accessor(
                stringify!($name),
                |p| p.lookup::<$typ>().map(|x| &x.$name as &dyn CellDisplay),
            ),
        )
    };
    ( $typ:ty : [$text:expr , $( $rest:tt )*] , $( $accum:tt )* ) => {
        explanation!(
            $typ: [$($rest)*] ,
            $($accum)*
            ClueExplanationChunk::Text($text),
        )
    };
    ( $typ:ty : $( $rest:tt )* ) => {
        explanation!(
            $typ: [$($rest)*] ,
        )
    };
}

static SAME_COLUMN_SOLO: &[ClueExplanationChunk] = explanation![
    Loc2:
    %{loc1}, "must be selected, because in the same column",
    %{loc2}, "is selected.",
    // %{loc2}, "is selected, therefore", %{loc1}, "must be selected in the same column.",
];

static SAME_COLUMN_CLEAR: &[ClueExplanationChunk] = explanation![
    Loc2:
    // %{loc2}, "is not possible, therefore", %{loc1}, "must be impossible in the same column.",
    %{loc1}, "must be impossible, because in the same column",
    %{loc2}, "is not possible.",
];

impl PuzzleClue for SameColumnClue {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance {
        let mut resolver = ImplicationResolver::new_unit(puzzle);
        resolver.add_answer(self.loc);
        resolver.add_answer(self.loc2());
        if let Some(loc3) = self.loc3() {
            resolver.add_answer(loc3);
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

    fn spawn_into(
        &self,
        parent: &mut ChildBuilder,
        puzzle: &Puzzle,
        cells: &mut HashMap<RowAnswer, Entity>,
    ) {
        let sprite_size = Vec2::new(32., 32.);
        let size_sprite = |mut sprite: Sprite| {
            sprite.custom_size = Some(sprite_size);
            sprite
        };
        let (sprite1, color1) = puzzle.cell_answer_display(self.loc);
        let id1 = parent
            .spawn((
                Sprite::from_color(color1, sprite_size),
                Transform::from_xyz(0., -32., 0.),
            ))
            .with_child((
                size_sprite(sprite1),
                Transform::from_xyz(0., 0., 1.),
                NO_PICK,
            ))
            .id();
        cells.insert(puzzle.answer_at(self.loc).decay_column(), id1);
        let loc2 = self.loc2();
        let (sprite2, color2) = puzzle.cell_answer_display(loc2);
        let id2 = parent
            .spawn((
                Sprite::from_color(color2, sprite_size),
                Transform::from_xyz(0., 0., 0.),
            ))
            .with_child((
                size_sprite(sprite2),
                Transform::from_xyz(0., 0., 1.),
                NO_PICK,
            ))
            .id();
        cells.insert(puzzle.answer_at(loc2).decay_column(), id2);
        if let Some(loc3) = self.loc3() {
            let (sprite3, color3) = puzzle.cell_answer_display(loc3);
            let id3 = parent
                .spawn((
                    Sprite::from_color(color3, sprite_size),
                    Transform::from_xyz(0., 32., 0.),
                ))
                .with_child((
                    size_sprite(sprite3),
                    Transform::from_xyz(0., 0., 1.),
                    NO_PICK,
                ))
                .id();
            cells.insert(puzzle.answer_at(loc3).decay_column(), id3);
        }
    }
}

#[derive(Debug, Component, Clone, Reflect)]
pub struct AdjacentColumnClue {
    loc1: CellLoc,
    loc2: CellLoc,
}

impl AdjacentColumnClue {
    pub fn new_random<R: Rng>(rng: &mut R, puzzle: &Puzzle) -> Option<Self> {
        let cols = puzzle.shuffled_cols(rng);
        Some(AdjacentColumnClue {
            loc1: CellLoc {
                row: puzzle.random_row(rng),
                col: cols[0],
            },
            loc2: CellLoc {
                row: puzzle.random_row(rng),
                col: cols[1],
            },
        })
    }

    pub fn colspan(&self) -> usize {
        self.loc1.columns_between(&self.loc2)
    }
}

// static ADJACENT_COLUMN_SOLO: &[ClueExplanationChunk] = explanation![
//     Loc2Mirrored:
//     %{loc2}, "is selected, therefore", %{loc1}, "must be selected.",
// ];

static ADJACENT_COLUMN_CLEAR: &[ClueExplanationChunk] = explanation![
    Loc2Mirrored:
    // "Neither", %{loc2}, "nor", %{loc2_p}, *{|l| format!("are possible {} columns removed from", l.colspan())},
    // %{loc1}, "therefore it is also impossible.",
    %{loc1}, "must be impossible, because",
    %{loc2}, "or", %{loc2_p},
    *{|l| format!("must be possible at {} columns removed.", l.colspan())},
];

impl PuzzleClue for AdjacentColumnClue {
    fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance {
        let mut resolver = ImplicationResolver::new_unit(puzzle);
        resolver.add_answer(self.loc1);
        resolver.add_answer(self.loc2);
        // info!("adjacent resolver: {resolver:#?}");
        for mut sub_resolver in resolver.iter_all_cols::<IfThen<_, _>>() {
            // info!("adjacent sub resolver: {sub_resolver:#?}");
            sub_resolver
                // .if_then(
                //     |Loc2Mirrored {
                //          loc1: l1,
                //          loc2: l2,
                //          loc2_p: l2p,
                //      }| {
                //         // info!("checking adjacent solo\n  l1={l1:?}\n  l2={l2:?}  \n  l3={l2p:?}");
                //         return None;
                //         if l1.is_enabled_not_solo() && (l2.is_solo || l2p.is_solo) {
                //             Some(l1.as_solo())
                //         } else {
                //             None
                //         }
                //     },
                // )
                .if_then(
                    |l @ Loc2Mirrored {
                         loc1: l1,
                         loc2: l2,
                         loc2_p: l2p,
                     }| {
                        // info!(
                        //     "checking adjacent enabled\n  l1={l1:?}\n  l2={l2:?}  \n  l3={l2p:?}"
                        // );
                        if l1.is_enabled_not_solo() && !l2.is_enabled && !l2p.is_enabled {
                            Some(l1.as_clear().with_explanation((l, ADJACENT_COLUMN_CLEAR)))
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

    fn spawn_into(
        &self,
        parent: &mut ChildBuilder,
        puzzle: &Puzzle,
        cells: &mut HashMap<RowAnswer, Entity>,
    ) {
        let sprite_size = Vec2::new(32., 32.);
        let size_sprite = |mut sprite: Sprite| {
            sprite.custom_size = Some(sprite_size);
            sprite
        };
        parent.spawn(Text2d::new(format!("{}", self.colspan())));
        let (sprite1, color1) = puzzle.cell_answer_display(self.loc1);
        let id1 = parent
            .spawn((
                Sprite::from_color(color1, sprite_size),
                Transform::from_xyz(-25., 0., 0.),
            ))
            .with_child((
                size_sprite(sprite1),
                Transform::from_xyz(0., 0., 1.),
                NO_PICK,
            ))
            .id();
        cells.insert(puzzle.answer_at(self.loc1).decay_column(), id1);
        let (sprite2, color2) = puzzle.cell_answer_display(self.loc2);
        let id2 = parent
            .spawn((
                Sprite::from_color(color2, sprite_size),
                Transform::from_xyz(25., 0., 0.),
            ))
            .with_child((
                size_sprite(sprite2),
                Transform::from_xyz(0., 0., 1.),
                NO_PICK,
            ))
            .id();
        cells.insert(puzzle.answer_at(self.loc2).decay_column(), id2);
    }
}

// #[derive(Debug, Component, Clone, Reflect)]
// pub struct BetweenColumnsClue {
//     loc1: CellLoc,
//     loc2: CellLoc,
//     loc3: CellLoc,
//     flip_on_display: bool,
// }

// impl BetweenColumnsClue {
//     pub fn new_random<R: Rng>(rng: &mut R, puzzle: &Puzzle) -> Option<Self> {
//         let n_rows = puzzle.rows.len();
//         let mut columns: [usize; 3] = rand::seq::index::sample_array(rng, puzzle.max_column)?;
//         columns.sort();
//         let [col1, col2, col3] = columns;
//         Some(BetweenColumnsClue {
//             loc1: CellLoc {
//                 row_nr: rng.random_range(0..n_rows),
//                 cell_nr: col1 as isize,
//             },
//             loc2: CellLoc {
//                 row_nr: rng.random_range(0..n_rows),
//                 cell_nr: col2 as isize,
//             },
//             loc3: CellLoc {
//                 row_nr: rng.random_range(0..n_rows),
//                 cell_nr: col3 as isize,
//             },
//             flip_on_display: rng.random(),
//         })
//     }
// }

// static BETWEEN_COLUMN_CLEAR: &[ClueExplanationChunk] = explanation![
//     Loc3:
//     %{loc1}, "must be impossible because it requires", %{loc2}, "and", %{loc3},
// ];

// impl PuzzleClue for BetweenColumnsClue {
//     fn advance_puzzle(&self, puzzle: &Puzzle) -> PuzzleAdvance {
//         let mut resolver = ImplicationResolver::new_unit(puzzle);
//         resolver.add_answer(self.loc1);
//         resolver.add_answer(self.loc2);
//         resolver.add_answer(self.loc3);
//         // info!("between resolver: {resolver:?}");
//         for mut sub_resolver in resolver.iter_all_cols::<IfThen<_, _>>() {
//             // info!("between sub resolver: {sub_resolver:?}");
//             sub_resolver.if_then(|l: &Loc3Mirrored| {
//                 if !l.loc1.is_enabled_not_solo() {
//                     return None;
//                 }
//                 l.eval_as_3s(|sl| {
//                     if !sl.loc2.is_enabled || !sl.loc3.is_enabled {
//                         Some(
//                             sl.loc1
//                                 .as_clear()
//                                 .with_explanation((sl, BETWEEN_COLUMN_CLEAR)),
//                         )
//                     } else {
//                         None
//                     }
//                 })
//             });
//             for ev in sub_resolver.iter_reflected_3s() {
//                 return Some(ev);
//             }
//         }
//         None
//     }

//     fn spawn_into(
//         &self,
//         parent: &mut ChildBuilder,
//         puzzle: &Puzzle,
//         cells: &mut HashMap<CellLoc, Entity>,
//     ) {
//         let sprite_size = Vec2::new(32., 32.);
//         let size_sprite = |mut sprite: Sprite| {
//             sprite.custom_size = Some(sprite_size);
//             sprite
//         };
//         let (loc1, loc3) = if self.flip_on_display {
//             (self.loc3, self.loc1)
//         } else {
//             (self.loc1, self.loc3)
//         };
//         let (sprite1, color1) = puzzle.cell_answer_display(loc1);
//         parent
//             .spawn((
//                 Sprite::from_color(color1, sprite_size),
//                 Transform::from_xyz(-32., 0., 0.),
//             ))
//             .with_child((
//                 size_sprite(sprite1),
//                 Transform::from_xyz(0., 0., 1.),
//                 NO_PICK,
//             ));
//         let (sprite2, color2) = puzzle.cell_answer_display(self.loc2);
//         parent
//             .spawn((
//                 Sprite::from_color(color2, sprite_size),
//                 Transform::from_xyz(0., 0., -1.),
//             ))
//             .with_child((
//                 size_sprite(sprite2),
//                 Transform::from_xyz(0., 0., 1.),
//                 NO_PICK,
//             ));
//         let (sprite3, color3) = puzzle.cell_answer_display(loc3);
//         parent
//             .spawn((
//                 Sprite::from_color(color3, sprite_size),
//                 Transform::from_xyz(32., 0., 0.),
//             ))
//             .with_child((
//                 size_sprite(sprite3),
//                 Transform::from_xyz(0., 0., 1.),
//                 NO_PICK,
//             ));
//     }
// }
