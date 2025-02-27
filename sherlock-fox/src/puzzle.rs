// © 2025 <_@habnab.it>
//
// SPDX-License-Identifier: EUPL-1.2

use bevy::{
    prelude::*,
    utils::{HashMap, HashSet},
};
use fixedbitset::FixedBitSet;
use rand::{seq::SliceRandom, Rng};

use crate::UpdateCellIndex;

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellLoc {
    pub row_nr: usize,
    pub cell_nr: isize,
}

impl CellLoc {
    pub fn shift(&self, shift: isize) -> CellLoc {
        let cell_nr = self.cell_nr + shift;
        CellLoc { cell_nr, ..*self }
    }

    pub fn reflect_about(&self, mirror: &CellLoc) -> CellLoc {
        let shift = (mirror.cell_nr as isize - self.cell_nr as isize) * 2;
        self.shift(shift)
    }

    pub fn colspan(&self, other: &CellLoc) -> usize {
        self.cell_nr.abs_diff(other.cell_nr).saturating_sub(1)
    }
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellLocIndex {
    pub loc: CellLoc,
    pub index: usize,
}

impl CellLocIndex {
    pub fn shift_loc(&self, shift: isize) -> CellLocIndex {
        CellLocIndex {
            loc: self.loc.shift(shift),
            ..*self
        }
    }

    pub fn reflect_loc_about(&self, mirror: &CellLocIndex) -> CellLocIndex {
        CellLocIndex {
            loc: self.loc.reflect_about(&mirror.loc),
            ..*self
        }
    }

    pub fn collapse_index(&self) -> CellLoc {
        CellLoc {
            row_nr: self.loc.row_nr,
            cell_nr: self.index as isize,
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
    Solo { width: usize, index: usize },
    Void,
}

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

    pub fn is_enabled(&self, index: usize) -> bool {
        use PuzzleCellSelection::*;
        match self {
            Enabled(s) => s.contains(index),
            &Solo { index: i, .. } => index == i,
            Void => false,
        }
    }

    pub fn is_solo(&self, index: usize) -> bool {
        use PuzzleCellSelection::*;
        match self {
            Enabled(s) => s.contains(index) && s.count_ones(..) == 1,
            &Solo { index: i, .. } => index == i,
            Void => false,
        }
    }

    pub fn is_any_solo(&self) -> Option<usize> {
        use PuzzleCellSelection::*;
        match self {
            Enabled(s) => {
                let mut ones = s.ones();
                let ret = ones.next();
                if ret.is_some() && ones.next().is_none() {
                    ret
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

    pub fn iter_ones(&self) -> Box<dyn Iterator<Item = usize> + '_> {
        use PuzzleCellSelection::*;
        match self {
            Enabled(s) => Box::new(s.ones()),
            &Solo { index, .. } => Box::new(std::iter::once(index)),
            Void => Box::new(std::iter::empty()),
        }
    }

    pub fn apply(&mut self, index: usize, op: UpdateCellIndexOperation) -> usize {
        use UpdateCellIndexOperation::*;
        if self.is_void() {
            warn!("logic error: tried to apply {op:?}@{index} to void selection");
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
                    let ret = if enabled.contains(index) { 1 } else { 0 };
                    enabled.remove(index);
                    ret
                }
                Set => {
                    if enabled.put(index) {
                        0
                    } else {
                        1
                    }
                }
                Toggle => {
                    enabled.toggle(index);
                    1
                }
                Solo => unreachable!(),
            },
            &mut PuzzleCellSelection::Solo { width, index: i } => {
                let mut enabled = FixedBitSet::with_capacity(width);
                enabled.insert(i);
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
    cell_selection: Vec<PuzzleCellSelection>,
    cell_display: Vec<PuzzleCellDisplay>,
    cell_answers: Vec<usize>,
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
        let mut cell_answers = (0..len).collect::<Vec<_>>();
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

    pub fn len(&self) -> usize {
        self.cell_selection.len()
    }

    pub fn display_atlas(&self, index: usize) -> TextureAtlas {
        TextureAtlas {
            layout: self.atlas_layout.clone(),
            index: self.cell_display[index].atlas_index,
        }
    }

    pub fn display_image_node(&self, index: usize) -> ImageNode {
        ImageNode::from_atlas_image(self.atlas.clone(), self.display_atlas(index))
    }

    pub fn display_sprite(&self, index: usize) -> Sprite {
        Sprite::from_atlas_image(self.atlas.clone(), self.display_atlas(index))
    }

    pub fn display_color(&self, index: usize) -> Color {
        self.cell_display[index].color
    }

    pub fn answer_display_sprite(&self, index: usize) -> Sprite {
        self.display_sprite(self.cell_answers[index])
    }

    pub fn answer_display_color(&self, index: usize) -> Color {
        self.display_color(self.cell_answers[index])
    }
}

#[derive(Debug, Clone, Component, Default, Reflect)]
pub struct Puzzle {
    pub rows: Vec<PuzzleRow>,
    pub max_column: usize,
    // happily, the default is Void
    void: PuzzleCellSelection,
}

impl Puzzle {
    pub fn add_row(&mut self, row: PuzzleRow) {
        self.max_column = self.max_column.max(row.len());
        self.rows.push(row);
    }

    pub fn cell_selection(&self, loc: CellLoc) -> &PuzzleCellSelection {
        let sel: Option<&PuzzleCellSelection> = try {
            self.rows[loc.row_nr]
                .cell_selection
                .get::<usize>(loc.cell_nr.try_into().ok()?)?
        };
        sel.unwrap_or(&self.void)
    }

    pub fn cell_selection_mut(&mut self, loc: CellLoc) -> &mut PuzzleCellSelection {
        let sel: Option<&mut PuzzleCellSelection> = try {
            self.rows[loc.row_nr]
                .cell_selection
                .get_mut::<usize>(loc.cell_nr.try_into().ok()?)?
        };
        sel.unwrap_or(&mut self.void)
    }

    // TODO: too many `as usize`
    pub fn cell_display(&self, loc: CellLoc) -> (Sprite, Color) {
        let row = &self.rows[loc.row_nr];
        (
            row.display_sprite(loc.cell_nr as usize),
            row.display_color(loc.cell_nr as usize),
        )
    }

    // TODO: too many `as usize`
    // TODO: also these names and return types aren't great
    pub fn cell_index_display(&self, index: CellLocIndex) -> (ImageNode, Color) {
        let row = &self.rows[index.loc.row_nr];
        (
            row.display_image_node(index.index),
            row.display_color(index.index),
        )
    }

    // TODO: too many `as usize`
    pub fn cell_answer_display(&self, loc: CellLoc) -> (Sprite, Color) {
        let row = &self.rows[loc.row_nr];
        (
            row.answer_display_sprite(loc.cell_nr as usize),
            row.answer_display_color(loc.cell_nr as usize),
        )
    }

    // TODO: too many `as usize`
    pub fn cell_answer_index(&self, loc: CellLoc) -> usize {
        self.rows[loc.row_nr].cell_answers[loc.cell_nr as usize]
    }

    pub fn answer_loc(&self, index: CellLocIndex) -> CellLoc {
        let cell_nr = self.rows[index.loc.row_nr].cell_answers[index.index] as isize;
        CellLoc {
            cell_nr,
            ..index.loc
        }
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

            fn drain_into(self, into: &mut HashMap<usize, HashSet<CellLocIndex>>) {
                if let SoloInf::One(index) = self {
                    into.entry(index.loc.row_nr).or_default().insert(index);
                }
            }
        }

        let rows = considering
            .drain()
            .map(|l| l.row_nr)
            .collect::<HashSet<_>>();
        let mut updates = 0;
        let mut did_update = |c: usize| {
            updates += c;
            c > 0
        };
        let mut solo_ops = HashMap::new();
        for row_nr in rows {
            let mut counts = HashMap::new();
            for cell_nr in 0..self.max_column as isize {
                let loc = CellLoc { row_nr, cell_nr };
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
        for (row_nr, solos) in solo_ops {
            for cell_nr in 0..self.max_column as isize {
                let loc = CellLoc { row_nr, cell_nr };
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
