use bevy::prelude::*;
use fixedbitset::FixedBitSet;
use rand::{seq::SliceRandom, Rng};

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellLoc {
    pub row_nr: usize,
    pub cell_nr: usize,
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellLocIndex {
    pub loc: CellLoc,
    pub index: usize,
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateCellIndexOperation {
    Clear,
    Set,
    Toggle,
    Solo,
}

#[derive(Debug, Clone, Reflect)]
pub struct PuzzleCellSelection {
    #[reflect(ignore)]
    pub enabled: FixedBitSet,
}

impl PuzzleCellSelection {
    pub fn new(enabled: FixedBitSet) -> Self {
        PuzzleCellSelection { enabled }
    }

    pub fn apply(&mut self, index: usize, op: UpdateCellIndexOperation) {
        use UpdateCellIndexOperation::*;
        match op {
            Clear => self.enabled.remove(index),
            Set => self.enabled.insert(index),
            Toggle => self.enabled.toggle(index),
            Solo => {
                self.enabled.remove_range(..);
                self.enabled.insert(index);
            }
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
}

impl Puzzle {
    pub fn add_row(&mut self, row: PuzzleRow) {
        self.max_column = self.max_column.max(row.len());
        self.rows.push(row);
    }

    pub fn cell_selection(&self, loc: CellLoc) -> &PuzzleCellSelection {
        &self.rows[loc.row_nr].cell_selection[loc.cell_nr]
    }

    pub fn cell_selection_mut(&mut self, loc: CellLoc) -> &mut PuzzleCellSelection {
        &mut self.rows[loc.row_nr].cell_selection[loc.cell_nr]
    }

    pub fn cell_display(&self, loc: CellLoc) -> (Sprite, Color) {
        let row = &self.rows[loc.row_nr];
        (
            row.display_sprite(loc.cell_nr),
            row.display_color(loc.cell_nr),
        )
    }

    pub fn cell_answer_display(&self, loc: CellLoc) -> (Sprite, Color) {
        let row = &self.rows[loc.row_nr];
        (
            row.answer_display_sprite(loc.cell_nr),
            row.answer_display_color(loc.cell_nr),
        )
    }

    pub fn cell_answer_index(&self, loc: CellLoc) -> usize {
        self.rows[loc.row_nr].cell_answers[loc.cell_nr]
    }
}
