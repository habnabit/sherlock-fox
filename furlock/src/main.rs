use std::time::Duration;

use bevy::{color::palettes::css, input::common_conditions::input_just_released, prelude::*, utils::hashbrown::HashMap, window::PrimaryWindow};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use fixedbitset::FixedBitSet;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(WorldInspectorPlugin::new())
        .add_event::<AddRow>()
        .add_event::<UpdateCellIndex>()
        .add_event::<UpdateCellDisplay>()
        .add_event::<StartCellDrag>()
        .register_type::<Puzzle>()
        .register_type::<PuzzleRow>()
        .register_type::<PuzzleCell>()
        .register_type::<DisplayMatrix>()
        .register_type::<DisplayCell>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                (cell_start_drag, cell_continue_drag, cell_release_drag.run_if(input_just_released(MouseButton::Left))).chain(),
                (interact_cell, cell_update, cell_update_display).chain(),
                (spawn_row, add_row).chain(),
            ),
        )
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

    fn apply(&mut self, index: usize, op: UpdateCellIndexOperation) {
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
struct PuzzleRow {
    cells: Vec<PuzzleCell>,
}

impl PuzzleRow {
    fn new(len: usize) -> Self {
        let mut bitset = FixedBitSet::with_capacity(len);
        bitset.insert_range(..);
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

    fn cell(&self, loc: CellLoc) -> &PuzzleCell {
        &self.rows[loc.row_nr].cells[loc.cell_nr]
    }

    fn cell_mut(&mut self, loc: CellLoc) -> &mut PuzzleCell {
        &mut self.rows[loc.row_nr].cells[loc.cell_nr]
    }
}

#[derive(Reflect, Debug, Component)]
struct NodeRoot;

#[derive(Reflect, Debug, Component)]
struct DisplayMatrix;

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CellLoc {
    row_nr: usize,
    cell_nr: usize,
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CellLocIndex {
    loc: CellLoc,
    index: usize,
}

#[derive(Reflect, Debug, Component)]
struct DisplayCell {
    loc: CellLoc,
}

#[derive(Reflect, Debug, Component)]
struct DisplayCellButton {
    index: CellLocIndex,
}

#[derive(Resource)]
struct PuzzleSpawn {
    timer: Timer,
}

#[derive(Event, Debug)]
struct AddRow {
    len: usize,
}

#[derive(Event, Debug)]
struct UpdateCellIndex {
    index: CellLocIndex,
    op: UpdateCellIndexOperation,
}

#[derive(Event, Debug)]
struct UpdateCellDisplay {
    loc: CellLoc,
}

#[derive(Event, Debug)]
struct StartCellDrag {
    entity: Entity,
    x: f32,
    y: f32,
}

#[derive(Reflect, Debug, Component)]
struct DragTarget;

#[derive(Debug, Clone, Copy)]
enum UpdateCellIndexOperation {
    Clear,
    Set,
    Toggle,
    Solo,
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
    mut matrix: Single<(Entity, &mut Node), With<DisplayMatrix>>,
) {
    let (matrix, ref mut matrix_node) = *matrix;
    let mut readjust_rows = false;
    for ev in reader.read() {
        readjust_rows = true;
        let row_nr = puzzle.rows.len();
        puzzle.add_row(PuzzleRow::new(ev.len));
        commands.entity(matrix).with_children(|matrix_spawner| {
            matrix_spawner
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
                .with_children(|row_spawner| {
                    for cell_nr in 0..ev.len {
                        let loc = CellLoc { row_nr, cell_nr };
                        row_spawner
                            .spawn((
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
                                DisplayCell { loc },
                            ))
                            .with_children(|cell_spawner| {
                                for index in 0..ev.len {
                                    cell_spawner
                                        .spawn((
                                            Node {
                                                padding: UiRect::all(Val::Px(5.)),
                                                margin: UiRect::all(Val::Px(5.)),
                                                border: UiRect::all(Val::Px(1.)),
                                                width: Val::Percent(100.),
                                                ..Default::default()
                                            },
                                            BorderColor(css::YELLOW_GREEN.into()),
                                            BackgroundColor(css::DARK_SLATE_GRAY.into()),
                                            Button,
                                            DisplayCellButton {
                                                index: CellLocIndex { loc, index },
                                            },
                                        ))
                                        .observe(cell_clicked_down)
                                        .observe(cell_clicked_up)
                                        .with_child(Text::new(format!("{index}")));
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

fn interact_cell(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>, With<DisplayCellButton>),
    >,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(css::HOT_PINK.into());
            }
            Interaction::Hovered => {
                *color = BackgroundColor(css::DEEP_PINK.into());
            }
            Interaction::None => {
                *color = BackgroundColor(css::DARK_SLATE_GRAY.into());
            }
        }
    }
}

fn cell_clicked_down(
    // mut commands: Commands,
    ev: Trigger<Pointer<Down>>,
    cell_query: Query<(Entity, &DisplayCellButton, &Interaction, &GlobalTransform)>,
    mut writer: EventWriter<StartCellDrag>,
) {
    for (entity, button, interaction, &transform) in &cell_query {
        if matches!(interaction, Interaction::Hovered) {
            info!("down ev={:#?} button={:#?} int={:#?} transform={:#?} local={:#?} iso={:#?}", ev, button, interaction, transform, transform.compute_transform(), transform.to_isometry());
            let loc = &ev.event().pointer_location;
            writer.send(dbg!(StartCellDrag { entity, x: loc.position.x, y: loc.position.y }));
        }
    }
}

fn cell_start_drag(
    mut commands: Commands,
    mut cell_index_query: Query<&DisplayCellButton>,
    mut reader: EventReader<StartCellDrag>,
    root: Single<Entity, With<NodeRoot>>,
) {
    for &StartCellDrag { entity, x, y } in reader.read() {
        commands.entity(*root).with_child((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y),
                ..Default::default()
            },
            Text::new("drag"),
            DragTarget,
        ));
        // let () = cell_index_query.get(entity);
    }
}

fn cell_continue_drag(
    mut cell: Query<&mut Node, With<DragTarget>>,
    // mut cursor_world_pos: ResMut<CursorWorldPos>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    // q_camera: Single<(&Camera, &GlobalTransform)>,
) {
    // let (main_camera, main_camera_transform) = *q_camera;
    // Get the cursor position in the world
    let Some(loc) = primary_window.iter().next().and_then(|w| w.cursor_position()) else { return };
    // dbg!(loc);
    for mut node in &mut cell {
        node.left = Val::Px(loc.x);
        node.top = Val::Px(loc.y);
    }
}

fn cell_release_drag(
    mut commands: Commands,
    mut cell: Query<Entity, With<DragTarget>>,
) {
    for entity in &cell {
        commands.entity(entity).despawn_recursive();
    }
}

fn cell_clicked_up(
    ev: Trigger<Pointer<Up>>,
    cell_query: Query<(&DisplayCellButton, &Interaction)>,
    mut puzzle: Single<&mut Puzzle>,
    mut writer: EventWriter<UpdateCellIndex>,
) {
    // info!("click ev={ev:?}");
    for (
        &DisplayCellButton {
            index,
        },
        interaction,
    ) in &cell_query
    {
        // info!("cell={cell:?} int={interaction:?}");
        if matches!(interaction, Interaction::Pressed) {
            writer.send(UpdateCellIndex { index, op: UpdateCellIndexOperation::Toggle });
        }
    }
}

fn cell_update(
    cell_query: Query<(Entity, &DisplayCell)>,
    mut puzzle: Single<&mut Puzzle>,
    mut reader: EventReader<UpdateCellIndex>,
    mut writer: EventWriter<UpdateCellDisplay>,
) {
    let entity_map = cell_query
        .iter()
        .map(|(entity, cell)| (cell.loc, entity))
        .collect::<HashMap<_, _>>();
    for &UpdateCellIndex { index, op } in reader.read() {
        let entity = entity_map.get(&index.loc);
        let puzzle_cell = puzzle.cell_mut(index.loc);
        puzzle_cell.apply(index.index, op);
        // info!(
        //     "updating: index={index:?} entity={entity:?} state={:x?}",
        //     puzzle_cell.enabled.as_slice()
        // );
        writer.send(UpdateCellDisplay { loc: index.loc });
    }
}

fn cell_update_display(
    puzzle: Single<&Puzzle>,
    mut cell_index_query: Query<(Entity, &DisplayCellButton, &mut BorderColor)>,
    mut reader: EventReader<UpdateCellDisplay>,
) {
    let mut entity_map = {
        let mut map = HashMap::new();
        for (entity, &DisplayCellButton { index }, border_color) in &mut cell_index_query {
            let v = map.entry(index.loc).or_insert_with(|| {
                let n_cells = puzzle.rows[index.loc.row_nr].len();
                (0..n_cells).map(|_| None).collect::<Vec<_>>()
            });
            v[index.index] = Some(border_color);
        }
        map
    };
    for &UpdateCellDisplay { loc } in reader.read() {
        let cell = puzzle.cell(loc);
        let Some(buttons) = entity_map.get_mut(&loc) else { unreachable!() };
        for (e, button) in buttons.iter_mut().enumerate() {
            let Some(ref mut border_color) = button else { unreachable!() };
            if cell.enabled.contains(e) {
                **border_color = BorderColor(css::YELLOW_GREEN.into());
            } else {
                **border_color = BorderColor(css::ORANGE_RED.into());
            }
        }
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(<Puzzle as Default>::default());
    commands.insert_resource(PuzzleSpawn {
        timer: Timer::new(Duration::from_secs_f32(0.1), TimerMode::Repeating),
    });

    commands
        .spawn((Node {
            display: Display::Grid,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Default::default()
        },NodeRoot))
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
