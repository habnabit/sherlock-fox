use std::time::Duration;

use bevy::{color::palettes::css, prelude::*};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use fixedbitset::FixedBitSet;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(WorldInspectorPlugin::new())
        .add_event::<AddRow>()
        .register_type::<Puzzle>()
        .register_type::<PuzzleRow>()
        .register_type::<PuzzleCell>()
        .register_type::<DisplayMatrix>()
        .register_type::<DisplayCell>()
        .add_systems(Startup, setup)
        .add_systems(Update, (spawn_row, add_row).chain())
        // .add_systems(Update, sprite_movement)
        .run();
}

#[derive(Debug, Clone, Reflect)]
// #[reflect(from_reflect = false)]
struct PuzzleCell {
    #[reflect(ignore)]
    enabled: FixedBitSet,
}

impl PuzzleCell {
    fn new(enabled: FixedBitSet) -> Self {
        PuzzleCell { enabled }
    }
}

#[derive(Debug, Clone, Reflect)]
struct PuzzleRow {
    cells: Vec<PuzzleCell>,
}

impl PuzzleRow {
    fn new(len: usize) -> Self {
        let mut bitset = FixedBitSet::with_capacity(len);
        bitset.set_range(.., true);
        let cells = vec![PuzzleCell::new(bitset); len];
        PuzzleRow { cells }
    }

    fn len(&self) -> usize {
        self.cells.len()
    }
}

#[derive(Debug, Component, Default, Reflect)]
struct Puzzle {
    rows: Vec<PuzzleRow>,
}

impl Puzzle {
    fn add_row(&mut self, row: PuzzleRow) {
        self.rows.push(row);
    }
}

#[derive(Reflect, Debug, Component)]
struct DisplayMatrix;

#[derive(Reflect, Debug, Component)]
struct DisplayCell {
    row_nr: usize,
    cell: usize,
}

#[derive(Resource)]
struct PuzzleSpawn {
    timer: Timer,
}

#[derive(Event, Debug)]
struct AddRow {
    len: usize,
}

fn spawn_row(
    mut writer: EventWriter<AddRow>,
    time: Res<Time>,
    mut config: ResMut<PuzzleSpawn>,
    puzzle: Single<&Puzzle>,
) {
    config.timer.tick(time.delta());
    if config.timer.finished() {
        if puzzle.rows.len() < 4 {
            writer.send(AddRow { len: 5 });
        }
    }
}

fn add_row(
    mut commands: Commands,
    mut reader: EventReader<AddRow>,
    mut puzzle: Single<&mut Puzzle>,
    mut matrix: Single<(Entity, &mut Node, &DisplayMatrix)>,
) {
    let (matrix, ref mut matrix_node, _) = *matrix;
    let mut readjust_rows = false;
    for ev in reader.read() {
        readjust_rows = true;
        let row_nr = puzzle.rows.len();
        puzzle.add_row(PuzzleRow::new(ev.len));
        commands.entity(matrix).with_children(|matrix| {
            matrix
                .spawn((
                    Node {
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::flex(ev.len as u16, 1.0),
                        padding: UiRect::all(Val::Px(5.)),
                        margin: UiRect::all(Val::Px(5.)),
                        border: UiRect::all(Val::Px(1.)),
                        ..Default::default()
                    },
                    BorderColor(css::REBECCA_PURPLE.into()),
                ))
                .with_children(|row| {
                    for cell in 0..ev.len {
                        row.spawn((
                            Node {
                                display: Display::Flex,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceEvenly,
                                padding: UiRect::all(Val::Px(5.)),
                                margin: UiRect::all(Val::Px(5.)),
                                border: UiRect::all(Val::Px(1.)),
                                ..Default::default()
                            },
                            BorderColor(css::STEEL_BLUE.into()),
                            DisplayCell { row_nr, cell },
                        ))
                        .with_children(|cell| {
                            for i in 0..ev.len {
                                cell.spawn((
                                    Node {
                                        padding: UiRect::all(Val::Px(5.)),
                                        margin: UiRect::all(Val::Px(5.)),
                                        border: UiRect::all(Val::Px(1.)),
                                        width: Val::Percent(100.),
                                        ..Default::default()
                                    },
                                    BorderColor(css::YELLOW_GREEN.into()),
                                    BackgroundColor(css::DARK_SLATE_GRAY.into()),
                                ))
                                .with_child(Text::new(format!("{i}")));
                            }
                        });
                    }
                });
        });
    }

    if readjust_rows {
        matrix_node.grid_template_rows = RepeatedGridTrack::flex(puzzle.rows.len() as u16, 1.0);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(<Puzzle as Default>::default());
    commands.insert_resource(PuzzleSpawn {
        timer: Timer::new(Duration::from_secs_f32(0.5), TimerMode::Repeating),
    });

    commands
        .spawn((Node {
            display: Display::Grid,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Default::default()
        },))
        .with_children(|root| {
            root.spawn((
                Node {
                    display: Display::Grid,
                    ..Default::default()
                },
                DisplayMatrix,
            ));
        });
}
