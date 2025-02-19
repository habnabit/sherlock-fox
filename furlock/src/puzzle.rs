use bevy::prelude::*;
use fixedbitset::FixedBitSet;
use rand::{seq::SliceRandom, Rng};

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

    pub fn reflect_about(&self, mirror: CellLoc) -> CellLoc {
        let shift = (mirror.cell_nr as isize - self.cell_nr as isize) * 2;
        self.shift(shift)
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

    pub fn reflect_loc_about(&self, mirror: CellLocIndex) -> CellLocIndex {
        CellLocIndex {
            loc: self.loc.reflect_about(mirror.loc),
            ..*self
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

    pub fn apply(&mut self, index: usize, op: UpdateCellIndexOperation) {
        use UpdateCellIndexOperation::*;
        if self.is_void() {
            warn!("logic error: tried to apply {op:?}@{index} to void selection");
            return;
        }
        if let Solo = op {
            let width = self.width();
            *self = PuzzleCellSelection::Solo { width, index };
            return;
        }
        match self {
            PuzzleCellSelection::Enabled(enabled) => match op {
                Clear => enabled.remove(index),
                Set => enabled.insert(index),
                Toggle => enabled.toggle(index),
                Solo => unreachable!(),
            },
            &mut PuzzleCellSelection::Solo { width, index: i } => {
                let mut enabled = FixedBitSet::with_capacity(width);
                enabled.insert(i);
                *self = PuzzleCellSelection::Enabled(enabled);
                self.apply(index, op);
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

    pub fn display_sprite(&self, index: usize) -> Sprite {
        Sprite::from_atlas_image(self.atlas.clone(), TextureAtlas {
            layout: self.atlas_layout.clone(),
            index: self.cell_display[index].atlas_index,
        })
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

#[derive(Debug, Component, Default, Reflect)]
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
}
