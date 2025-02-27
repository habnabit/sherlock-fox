// © 2025 <_@habnab.it>
//
// SPDX-License-Identifier: EUPL-1.2

use std::ops::{Range, RangeInclusive};

use bevy::{
    prelude::*,
    utils::{HashMap, HashSet},
};
use fixedbitset::FixedBitSet;
use itertools::MinMaxResult;
use rand::{seq::SliceRandom, Rng};

use crate::UpdateCellIndex;

#[derive(Reflect, Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LRow(pub usize);

#[derive(Reflect, Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LCol(pub isize);

impl LCol {
    pub fn shift(&self, shift: isize) -> Self {
        LCol(self.0 + shift)
    }

    pub fn delta(&self, other: &LCol) -> isize {
        self.0 - other.0
    }

    pub fn columns_between(&self, other: &LCol) -> usize {
        self.0.abs_diff(other.0).saturating_sub(1)
    }
}

#[derive(Reflect, Debug, Default, Clone)]
pub struct LColspan {
    pub min: LCol,
    pub max: LCol,
    pub abs_diff: usize,
}

impl From<MinMaxResult<LCol>> for LColspan {
    fn from(value: MinMaxResult<LCol>) -> Self {
        use itertools::MinMaxResult::*;
        let (min, max) = match value {
            OneElement(c) => (c, c),
            MinMax(a, b) => (a, b),
            NoElements => panic!("needed at least one column"),
        };
        Self {
            min,
            max,
            abs_diff: max.0.abs_diff(min.0),
        }
    }
}

#[derive(Reflect, Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LInd(pub usize);

#[derive(Reflect, Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LAns(pub usize);

impl LAns {
    pub fn decay_to_ind(&self) -> LInd {
        LInd(self.0)
    }
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellLoc {
    pub row: LRow,
    pub col: LCol,
}

impl CellLoc {
    pub fn shift_column(&self, shift: isize) -> CellLoc {
        let col = self.col.shift(shift);
        CellLoc { col, ..*self }
    }

    pub fn reflect_about(&self, mirror: &CellLoc) -> CellLoc {
        let shift = mirror.col.delta(&self.col) * 2;
        self.shift_column(shift)
    }

    pub fn columns_between(&self, other: &CellLoc) -> usize {
        self.col.columns_between(&other.col)
    }
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellLocIndexed<I> {
    pub loc: CellLoc,
    pub index: I,
}

pub type CellLocIndex = CellLocIndexed<LInd>;
pub type CellLocAnswer = CellLocIndexed<LAns>;

impl CellLocIndex {
    pub fn shift_column(&self, shift: isize) -> CellLocIndex {
        CellLocIndex {
            loc: self.loc.shift_column(shift),
            ..*self
        }
    }

    pub fn reflect_loc_about(&self, mirror: &CellLocIndex) -> CellLocIndex {
        CellLocIndex {
            loc: self.loc.reflect_about(&mirror.loc),
            ..*self
        }
    }

    fn as_update(&self, op: UpdateCellIndexOperation) -> UpdateCellIndex {
        UpdateCellIndex {
            index: *self,
            op,
            explanation: None,
        }
    }

    pub fn as_clear(&self) -> UpdateCellIndex {
        self.as_update(UpdateCellIndexOperation::Clear)
    }

    pub fn as_solo(&self) -> UpdateCellIndex {
        self.as_update(UpdateCellIndexOperation::Solo)
    }
}

impl CellLocAnswer {
    pub fn decay_to_ind(&self) -> CellLocIndex {
        CellLocIndex {
            loc: self.loc,
            index: self.index.decay_to_ind(),
        }
    }
}

impl<I: Copy> CellLocIndexed<I> {
    pub fn decay_column(&self) -> RowIndexed<I> {
        RowIndexed {
            row: self.loc.row,
            index: self.index,
        }
    }
}

#[derive(Reflect, Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowIndexed<I> {
    row: LRow,
    index: I,
}

pub type RowIndex = RowIndexed<LInd>;
pub type RowAnswer = RowIndexed<LAns>;

impl RowIndex {
    pub fn upgrade_to_answer(&self) -> RowAnswer {
        RowAnswer {
            row: self.row,
            index: LAns(self.index.0),
        }
    }
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateCellIndexOperation {
    Clear,
    Set,
    Toggle,
    Solo,
}

#[derive(Debug, Clone, Reflect)]
pub enum PuzzleCellSelection {
    Enabled(#[reflect(ignore)] FixedBitSet),
    Solo { width: usize, index: LInd },
    Void,
}

pub static VOID: PuzzleCellSelection = PuzzleCellSelection::Void;

impl Default for PuzzleCellSelection {
    fn default() -> Self {
        PuzzleCellSelection::Void
    }
}

impl PuzzleCellSelection {
    pub fn new(enabled: FixedBitSet) -> Self {
        PuzzleCellSelection::Enabled(enabled)
    }

    pub fn is_void(&self) -> bool {
        matches!(self, PuzzleCellSelection::Void)
    }

    pub fn is_enabled(&self, index: LInd) -> bool {
        use PuzzleCellSelection::*;
        match self {
            Enabled(s) => s.contains(index.0),
            &Solo { index: i, .. } => index == i,
            Void => false,
        }
    }

    pub fn is_solo(&self, index: LInd) -> bool {
        use PuzzleCellSelection::*;
        match self {
            Enabled(s) => s.contains(index.0) && s.count_ones(..) == 1,
            &Solo { index: i, .. } => index == i,
            Void => false,
        }
    }

    pub fn is_any_solo(&self) -> Option<LInd> {
        use PuzzleCellSelection::*;
        match self {
            Enabled(s) => {
                let mut ones = s.ones();
                let ret = ones.next();
                if ret.is_some() && ones.next().is_none() {
                    ret.map(LInd)
                } else {
                    None
                }
            }
            &Solo { index, .. } => Some(index),
            Void => None,
        }
    }

    pub fn width(&self) -> usize {
        use PuzzleCellSelection::*;
        match self {
            Enabled(s) => s.len(),
            &Solo { width, .. } => width,
            Void => 0,
        }
    }

    pub fn count_ones(&self) -> usize {
        use PuzzleCellSelection::*;
        match self {
            Enabled(s) => s.count_ones(..),
            Solo { .. } => 1,
            Void => 0,
        }
    }

    pub fn iter_ones(&self) -> Box<dyn Iterator<Item = LInd> + '_> {
        use PuzzleCellSelection::*;
        match self {
            Enabled(s) => Box::new(s.ones().map(LInd)),
            &Solo { index, .. } => Box::new(std::iter::once(index)),
            Void => Box::new(std::iter::empty()),
        }
    }

    pub fn apply(&mut self, index: LInd, op: UpdateCellIndexOperation) -> usize {
        use UpdateCellIndexOperation::*;
        if self.is_void() {
            warn!("logic error: tried to apply {op:?}@{index:?} to void selection");
            return 0;
        }
        if let Solo = op {
            let width = self.width();
            let ret = self
                .count_ones()
                .saturating_add_signed(if self.is_enabled(index) { -1 } else { 1 });
            *self = PuzzleCellSelection::Solo { width, index };
            return ret;
        }
        match self {
            PuzzleCellSelection::Enabled(enabled) => match op {
                Clear => {
                    let ret = if enabled.contains(index.0) { 1 } else { 0 };
                    enabled.remove(index.0);
                    ret
                }
                Set => {
                    if enabled.put(index.0) {
                        0
                    } else {
                        1
                    }
                }
                Toggle => {
                    enabled.toggle(index.0);
                    1
                }
                Solo => unreachable!(),
            },
            &mut PuzzleCellSelection::Solo { width, index: i } => {
                let mut enabled = FixedBitSet::with_capacity(width);
                enabled.insert(i.0);
                *self = PuzzleCellSelection::Enabled(enabled);
                return self.apply(index, op);
            }
            PuzzleCellSelection::Void => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Reflect)]
pub struct PuzzleCellDisplay {
    atlas_index: usize,
    color: Color,
}

#[derive(Debug, Clone, Reflect)]
pub struct PuzzleRow {
    // LCol -> [LInd]
    cell_selection: Vec<PuzzleCellSelection>,
    // LInd -> Display
    cell_display: Vec<PuzzleCellDisplay>,
    // LCol -> LAns
    cell_answers: Vec<LAns>,
    atlas: Handle<Image>,
    atlas_layout: Handle<TextureAtlasLayout>,
}

impl PuzzleRow {
    pub fn new_shuffled<R: Rng>(
        rng: &mut R,
        len: usize,
        atlas: Handle<Image>,
        atlas_layout: Handle<TextureAtlasLayout>,
        atlas_len: usize,
        shuffle_atlas: bool,
    ) -> Self {
        let colors = crate::random_colors(len, rng);
        let mut cell_answers = (0..len).map(LAns).collect::<Vec<_>>();
        cell_answers.shuffle(rng);
        let mut bitset = FixedBitSet::with_capacity(len);
        bitset.insert_range(..);
        let mut atlas_index_map = (0..atlas_len).collect::<Vec<_>>();
        if shuffle_atlas {
            atlas_index_map.shuffle(rng);
        }
        let cell_display = atlas_index_map
            .into_iter()
            .take(len)
            .zip(colors)
            .map(|(atlas_index, color)| PuzzleCellDisplay { atlas_index, color })
            .collect();
        let cell_selection = (0..len)
            .map(|_| PuzzleCellSelection::new(bitset.clone()))
            .collect();
        PuzzleRow {
            cell_selection,
            cell_display,
            cell_answers,
            atlas,
            atlas_layout,
        }
    }

    // pub fn len(&self) -> usize {
    //     self.cell_selection.len()
    // }

    pub fn iter_cols(&self) -> impl Iterator<Item = LCol> {
        (0..self.cell_selection.len() as isize).map(LCol)
    }

    pub fn iter_indices(&self) -> impl Iterator<Item = LInd> {
        (0..self.cell_selection.len()).map(LInd)
    }

    pub fn max_column(&self) -> LCol {
        LCol(self.cell_selection.len().saturating_sub(1) as isize)
    }

    pub fn selection_at(&self, col: LCol) -> Option<&PuzzleCellSelection> {
        let col: usize = col.0.try_into().ok()?;
        self.cell_selection.get(col)
    }

    pub fn selection_mut_at(&mut self, col: LCol) -> Option<&mut PuzzleCellSelection> {
        let col: usize = col.0.try_into().ok()?;
        self.cell_selection.get_mut(col)
    }

    pub fn answer_at(&self, col: LCol) -> LAns {
        self.cell_answers[col.0 as usize]
    }

    pub fn display_atlas(&self, index: LInd) -> TextureAtlas {
        TextureAtlas {
            layout: self.atlas_layout.clone(),
            index: self.cell_display[index.0].atlas_index,
        }
    }

    pub fn display_image_node(&self, index: LInd) -> ImageNode {
        ImageNode::from_atlas_image(self.atlas.clone(), self.display_atlas(index))
    }

    pub fn display_sprite(&self, index: LInd) -> Sprite {
        Sprite::from_atlas_image(self.atlas.clone(), self.display_atlas(index))
    }

    pub fn display_color(&self, LInd(index): LInd) -> Color {
        self.cell_display[index].color
    }

    // TODO: should be less safe? or should LInd/LAns be combined?
    fn answer_as_index(&self, col: LCol) -> LInd {
        LInd(self.answer_at(col).0)
    }

    pub fn answer_display_sprite(&self, col: LCol) -> Sprite {
        self.display_sprite(self.answer_as_index(col))
    }

    pub fn answer_display_color(&self, col: LCol) -> Color {
        self.display_color(self.answer_as_index(col))
    }
}

#[derive(Debug, Clone, Component, Default, Reflect)]
pub struct Puzzle {
    rows: Vec<PuzzleRow>,
    max_column: LCol,
}

impl Puzzle {
    pub fn add_row(&mut self, row: PuzzleRow) -> LRow {
        let ret = LRow(self.rows.len());
        self.max_column = self.max_column.max(row.max_column());
        self.rows.push(row);
        ret
    }

    pub fn n_rows(&self) -> usize {
        self.rows.len()
    }

    fn row_range(&self) -> Range<usize> {
        0..self.rows.len()
    }

    pub fn iter_rows(&self) -> impl Iterator<Item = LRow> {
        self.row_range().map(LRow)
    }

    pub fn shuffled_rows<R: Rng>(&self, rng: &mut R) -> Vec<LRow> {
        let mut all_rows = self.iter_rows().collect::<Vec<_>>();
        all_rows.shuffle(rng);
        all_rows
    }

    pub fn random_row<R: Rng>(&self, rng: &mut R) -> LRow {
        LRow(rng.random_range(self.row_range()))
    }

    fn col_range(&self) -> RangeInclusive<isize> {
        0..=self.max_column.0
    }

    pub fn iter_cols(&self) -> impl Iterator<Item = LCol> {
        self.col_range().map(LCol)
    }

    pub fn iter_col_shift(&self, from_span: LColspan) -> impl Iterator<Item = isize> {
        (-(from_span.abs_diff as isize)..0)
            .chain(self.col_range())
            .map(move |shift| shift - from_span.min.0)
    }

    pub fn shuffled_cols<R: Rng>(&self, rng: &mut R) -> Vec<LCol> {
        let mut all_cols = self.iter_cols().collect::<Vec<_>>();
        all_cols.shuffle(rng);
        all_cols
    }

    pub fn random_column<R: Rng>(&self, rng: &mut R) -> LCol {
        LCol(rng.random_range(0..=self.max_column.0 as usize) as isize)
    }

    pub fn row_at(&self, row: LRow) -> &PuzzleRow {
        &self.rows[row.0]
    }

    pub fn row_mut_at(&mut self, row: LRow) -> &mut PuzzleRow {
        &mut self.rows[row.0]
    }

    pub fn cell_selection(&self, loc: CellLoc) -> &PuzzleCellSelection {
        self.row_at(loc.row).selection_at(loc.col).unwrap_or(&VOID)
    }

    pub fn cell_selection_mut(&mut self, loc: CellLoc) -> &mut PuzzleCellSelection {
        self.row_mut_at(loc.row)
            .selection_mut_at(loc.col)
            .unwrap_or_else(|| todo!())
    }

    // TODO: too many `as usize`
    // pub fn cell_display(&self, loc: CellLoc) -> (Sprite, Color) {
    //     let row = self.row_at(loc.row);
    //     (
    //         row.display_sprite(loc.cell_nr as usize),
    //         row.display_color(loc.cell_nr as usize),
    //     )
    // }

    // TODO: too many `as usize`
    // TODO: also these names and return types aren't great
    pub fn cell_index_display(&self, index: CellLocIndex) -> (ImageNode, Color) {
        let row = self.row_at(index.loc.row);
        (
            row.display_image_node(index.index),
            row.display_color(index.index),
        )
    }

    pub fn cell_answer_display(&self, loc: CellLoc) -> (Sprite, Color) {
        let row = self.row_at(loc.row);
        (
            row.answer_display_sprite(loc.col),
            row.answer_display_color(loc.col),
        )
    }

    pub fn answer_at(&self, loc: CellLoc) -> CellLocAnswer {
        let index = self.row_at(loc.row).answer_at(loc.col);
        CellLocAnswer { loc, index }
    }

    fn one_inference_step(
        &mut self,
        to_update: &mut HashSet<CellLoc>,
        considering: &mut HashSet<CellLoc>,
    ) -> usize {
        #[derive(Debug)]
        enum SoloInf {
            None,
            One(CellLocIndex),
            Many,
        }

        impl SoloInf {
            fn insert(&mut self, index: CellLocIndex) {
                *self = match self {
                    SoloInf::None => SoloInf::One(index),
                    _ => SoloInf::Many,
                };
            }

            fn drain_into(self, into: &mut HashMap<LRow, HashSet<CellLocIndex>>) {
                if let SoloInf::One(index) = self {
                    into.entry(index.loc.row).or_default().insert(index);
                }
            }
        }

        let rows = considering.drain().map(|l| l.row).collect::<HashSet<_>>();
        let mut updates = 0;
        let mut did_update = |c: usize| {
            updates += c;
            c > 0
        };
        let mut solo_ops = HashMap::new();
        for row in rows {
            let mut counts = HashMap::new();
            for col in self.iter_cols() {
                let loc = CellLoc { row, col };
                let mut cell_inf = SoloInf::None;
                for index in self.cell_selection(loc).iter_ones() {
                    let cell_index = CellLocIndex { loc, index };
                    counts
                        .entry(index)
                        .or_insert(SoloInf::None)
                        .insert(cell_index);
                    cell_inf.insert(cell_index);
                }
                // info!("cell_inf @r{row_nr}xc{cell_nr}: {cell_inf:#?}");
                cell_inf.drain_into(&mut solo_ops);
            }
            // info!("counts @r{row_nr}: {counts:#?}");
            for inf in counts.into_values() {
                inf.drain_into(&mut solo_ops);
            }
        }
        // info!("solo ops: {solo_ops:#?}");
        for (row, solos) in solo_ops {
            for col in self.iter_cols() {
                let loc = CellLoc { row, col };
                let sel = self.cell_selection_mut(loc);
                for solo_index in &solos {
                    let op = if loc == solo_index.loc {
                        UpdateCellIndexOperation::Solo
                    } else {
                        UpdateCellIndexOperation::Clear
                    };
                    if did_update(sel.apply(solo_index.index, op)) {
                        considering.insert(loc);
                        to_update.insert(loc);
                    }
                }
            }
        }
        // info!("updates: {updates}");
        updates
    }

    pub fn run_inference(&mut self, to_update: &mut HashSet<CellLoc>) -> usize {
        let mut considering = to_update.clone();
        let mut updates = 0;
        let mut steps = 0;
        while !considering.is_empty() {
            info!(
                "running inference to_update hwm {} considering hwm {}",
                to_update.len(),
                considering.len()
            );
            updates += self.one_inference_step(to_update, &mut considering);
            steps += 1;
            info!("ran inference step {steps}, {updates} updates");
        }
        updates
    }
}
