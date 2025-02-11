use std::time::Duration;

use bevy::{
    color::palettes::css, input::common_conditions::input_just_released, prelude::*,
    utils::hashbrown::HashMap, window::PrimaryWindow,
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use fixedbitset::FixedBitSet;
use rand::{distr::Distribution, seq::SliceRandom, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

fn main() {
    App::new()
        .init_resource::<SeededRng>()
        .add_plugins(DefaultPlugins)
        .add_plugins(WorldInspectorPlugin::new())
        .add_event::<AddRow>()
        .add_event::<StartButtonDrag>()
        .add_event::<UpdateCellDisplay>()
        .add_event::<UpdateCellIndex>()
        .register_type::<AssignRandomColor>()
        .register_type::<CellLoc>()
        .register_type::<CellLocIndex>()
        .register_type::<DisplayCell>()
        .register_type::<DisplayCellButton>()
        .register_type::<DisplayMatrix>()
        .register_type::<DisplayRow>()
        .register_type::<FitWithin>()
        .register_type::<Puzzle>()
        .register_type::<PuzzleCell>()
        .register_type::<PuzzleRow>()
        .register_type::<SeededRng>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                assign_random_color,
                (
                    fit_inside_window.run_if(any_with_component::<PrimaryWindow>),
                    fit_inside_matrix,
                    fit_inside_row,
                    fit_inside_cell,
                    fit_to_transform,
                    show_fit,
                )
                    .chain(),
                (
                    mouse_inside_window.run_if(any_with_component::<PrimaryWindow>),
                    interact_cell,
                )
                    .chain(),
                (
                    cell_start_drag,
                    cell_continue_drag,
                    cell_release_drag.run_if(input_just_released(MouseButton::Left)),
                )
                    .chain(),
                (interact_cell, cell_update, cell_update_display).chain(),
                (spawn_row, add_row).chain(),
            ),
        )
        // .add_systems(Update, sprite_movement)
        .run();
}

#[derive(Resource, Reflect)]
#[reflect(from_reflect = false)]
struct SeededRng(#[reflect(ignore)] ChaCha8Rng);

impl FromWorld for SeededRng {
    fn from_world(_world: &mut World) -> Self {
        SeededRng(ChaCha8Rng::from_os_rng())
    }
}

#[derive(Debug, Clone, Reflect)]
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
    colors: Vec<Color>,
}

impl PuzzleRow {
    fn new(colors: Vec<Color>) -> Self {
        let len = colors.len();
        let mut bitset = FixedBitSet::with_capacity(len);
        bitset.insert_range(..);
        let cells = vec![PuzzleCell::new(bitset); len];
        PuzzleRow { cells, colors }
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

#[derive(Reflect, Debug, Component, Default)]
struct FitWithin {
    rect: Rect,
    updating: bool,
}

impl FitWithin {
    fn new(rect: Rect) -> Self {
        FitWithin {
            rect,
            updating: true,
        }
    }

    fn set_rect(&mut self, new_rect: Rect) {
        if self.rect != new_rect {
            self.updating = true;
        }
        self.rect = new_rect;
    }
}

#[derive(Reflect, Debug, Component)]
enum FitInteraction {
    None,
    Hover,
}

#[derive(Bundle)]
struct FitWithinBundle {
    fit: FitWithin,
    interaction: FitInteraction,
    transform: Transform,
    visibility: InheritedVisibility,
}

impl FitWithinBundle {
    fn new() -> Self {
        FitWithinBundle {
            fit: FitWithin::default(),
            interaction: FitInteraction::None,
            transform: Transform::default(),
            visibility: InheritedVisibility::VISIBLE,
        }
    }
}

#[derive(Bundle)]
struct RandomColorSprite {
    sprite: Sprite,
    assign: AssignRandomColor,
}

impl RandomColorSprite {
    fn new() -> Self {
        RandomColorSprite {
            sprite: Sprite::from_color(css::ALICE_BLUE, Vec2::new(1., 1.)),
            assign: AssignRandomColor,
        }
    }
}

#[derive(Reflect, Debug, Component)]
struct AssignRandomColor;

#[derive(Reflect, Debug, Component)]
struct DisplayMatrix;

#[derive(Reflect, Debug, Component)]
struct DisplayRow {
    row_nr: usize,
}

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

#[derive(Reflect, Debug, Component, Clone)]
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
struct StartButtonDrag {
    entity: Entity,
}

#[derive(Reflect, Debug, Component, Default)]
struct DragUI;

#[derive(Reflect, Debug, Component, Default)]
struct DragTarget {
    start: Vec2,
    latest: Vec2,
}

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

fn assign_random_color(
    mut commands: Commands,
    mut rng: ResMut<SeededRng>,
    mut q_fit: Query<(Entity, &mut Sprite), With<AssignRandomColor>>,
) {
    let rng = &mut rng.0;
    let hue_dist = rand::distr::Uniform::new(0., 360.).unwrap();
    let saturation_dist = rand::distr::Uniform::new(0.5, 0.9).unwrap();
    let lightness_dist = rand::distr::Uniform::new(0.2, 0.6).unwrap();
    for (entity, mut sprite) in &mut q_fit {
        sprite.color = Color::hsla(
            hue_dist.sample(rng),
            saturation_dist.sample(rng),
            lightness_dist.sample(rng),
            0.2,
        );
        commands.entity(entity).remove::<AssignRandomColor>();
    }
}

fn random_colors<R: Rng>(n_colors: usize, rng: &mut R) -> Vec<Color> {
    let n_samples = n_colors * 3;
    let saturation_dist = rand::distr::Uniform::new(0.5, 0.9).unwrap();
    let lightness_dist = rand::distr::Uniform::new(0.2, 0.6).unwrap();
    let saturation = saturation_dist.sample(rng);
    let lightness = lightness_dist.sample(rng);
    let hue_width = 360. / n_samples as f32;
    let hue_shift = hue_width / 2. * rand::distr::Uniform::new(0., 1.).unwrap().sample(rng);
    let mut hues = (0..n_samples)
        .map(|i| hue_shift + hue_width * i as f32)
        .collect::<Vec<_>>();
    // info!(
    //     "saturation={saturation} lightntess={lightness} hue_width={hue_width} \
    //      hue_shift={hue_shift} hues={hues:?}"
    // );
    hues.as_mut_slice().shuffle(rng);
    // info!("shuffled? hues={hues:?}");
    hues.into_iter()
        .take(n_colors)
        .map(|hue| Color::hsl(hue, saturation, lightness))
        .collect()
}

fn add_row(
    mut commands: Commands,
    mut reader: EventReader<AddRow>,
    mut rng: ResMut<SeededRng>,
    mut puzzle: Single<&mut Puzzle>,
    matrix: Single<Entity, With<DisplayMatrix>>,
) {
    // let mut observer = Observer::new(cell_clicked_down);
    for ev in reader.read() {
        let row_nr = puzzle.rows.len();
        let colors = random_colors(ev.len, &mut rng.0);
        puzzle.add_row(PuzzleRow::new(colors));
        commands.entity(*matrix).with_children(|matrix_spawner| {
            matrix_spawner
                .spawn((
                    FitWithinBundle::new(),
                    // RandomColorSprite::new(),
                    DisplayRow { row_nr },
                ))
                .with_children(|row_spawner| {
                    for cell_nr in 0..ev.len {
                        let loc = CellLoc { row_nr, cell_nr };
                        row_spawner
                            .spawn((
                                FitWithinBundle::new(),
                                // RandomColorSprite::new(),
                                DisplayCell { loc },
                            ))
                            .with_children(|cell_spawner| {
                                for index in 0..ev.len {
                                    let cell = cell_spawner
                                        .spawn((
                                            Sprite::from_color(
                                                puzzle.rows[row_nr].colors[index],
                                                Vec2::new(25., 25.),
                                            ),
                                            FitWithinBundle::new(),
                                            DisplayCellButton {
                                                index: CellLocIndex { loc, index },
                                            },
                                        ))
                                        .observe(cell_clicked_down)
                                        // .observe(cell_clicked_up)
                                        .with_child((
                                            Text2d::new(format!("{index}")),
                                            Transform::from_xyz(0., 0., 1.),
                                        ))
                                        .id();
                                    // observer.watch_entity(cell);
                                }
                            });
                    }
                });
        });
    }
    // commands.spawn(observer);
}

fn fit_inside_window(
    q_primary_window: Single<&Window, With<PrimaryWindow>>,
    q_camera: Single<(&Camera, &GlobalTransform)>,
    mut q_fit_root: Query<&mut FitWithin, Without<Parent>>,
    // parent: Query<()>,
    // mut child: Query<()>,
) {
    let (camera, camera_transform) = *q_camera;
    let Some(logical_viewport) = camera.logical_viewport_rect() else {
        return;
    };
    for mut fit_within in &mut q_fit_root {
        fit_within.set_rect(logical_viewport.inflate(-50.));
    }
}

fn fit_inside_matrix(
    q_parent: Query<&FitWithin, (With<DisplayMatrix>, Without<DisplayRow>)>,
    mut q_child: Query<(&mut FitWithin, &Parent, &DisplayRow)>,
) {
    let mut row_map = HashMap::<Entity, _>::new();
    for (fit_within, parent, display_row) in &mut q_child {
        let Ok(within) = q_parent.get(**parent) else {
            unreachable!()
        };
        let (_, children) = row_map.entry(**parent).or_insert_with(|| (within, vec![]));
        children.push((display_row, fit_within));
    }
    for (within, mut children) in row_map.into_values() {
        children.sort_by_key(|(row, _)| row.row_nr);
        let fit = within.rect;
        let fit_height = fit.height();
        let row_height = fit_height / children.len() as f32;
        let mut current_y = fit.max.y;
        for (display_row, mut fit_within) in children {
            // dbg!(display_row);
            let new_y = current_y - row_height;
            let row_rect =
                Rect::from_corners(Vec2::new(fit.min.x, current_y), Vec2::new(fit.max.x, new_y));
            fit_within.set_rect(row_rect);
            current_y = new_y;
        }
    }
}

fn fit_inside_row(
    q_parent: Query<&FitWithin, (With<DisplayRow>, Without<DisplayCell>)>,
    mut q_child: Query<(&mut FitWithin, &Parent, &DisplayCell)>,
) {
    let mut cell_map = HashMap::<Entity, _>::new();
    for (fit_within, parent, display_row) in &mut q_child {
        let Ok(within) = q_parent.get(**parent) else {
            unreachable!()
        };
        let (_, children) = cell_map.entry(**parent).or_insert_with(|| (within, vec![]));
        children.push((display_row, fit_within));
    }
    for (within, mut children) in cell_map.into_values() {
        children.sort_by_key(|(cell, _)| cell.loc);
        let fit = within.rect;
        let fit_width = fit.width();
        let prospective_cell_width = fit_width / children.len() as f32;
        let cell_spacing = prospective_cell_width * 0.15;
        let total_cell_spacing = cell_spacing * (children.len() - 1) as f32;
        let cell_width = (fit_width - total_cell_spacing) / children.len() as f32;
        let mut current_x = fit.min.x;
        for (display_cell, mut fit_within) in children {
            let new_x = current_x + cell_width;
            let cell_rect =
                Rect::from_corners(Vec2::new(current_x, fit.min.y), Vec2::new(new_x, fit.max.y));
            fit_within.set_rect(cell_rect);
            current_x = new_x + cell_spacing;
        }
    }
}

fn fit_inside_cell(
    q_parent: Query<&FitWithin, (With<DisplayCell>, Without<DisplayCellButton>)>,
    mut q_child: Query<(&mut FitWithin, &Parent, &DisplayCellButton)>,
) {
    let mut cell_map = HashMap::<Entity, _>::new();
    for (fit_within, parent, display_row) in &mut q_child {
        let Ok(within) = q_parent.get(**parent) else {
            unreachable!()
        };
        let (_, children) = cell_map.entry(**parent).or_insert_with(|| (within, vec![]));
        children.push((display_row, fit_within));
    }
    for (within, mut children) in cell_map.into_values() {
        children.sort_by_key(|(cell, _)| cell.index);
        let fit = within.rect;
        let fit_width = fit.width();
        let cell_width = fit_width / children.len() as f32;
        let mut current_x = fit.min.x;
        for (display_cell, mut fit_within) in children {
            let new_x = current_x + cell_width;
            let cell_rect =
                Rect::from_corners(Vec2::new(current_x, fit.min.y), Vec2::new(new_x, fit.max.y));
            fit_within.set_rect(cell_rect);
            current_x = new_x;
        }
    }
}

fn fit_to_transform(mut q_fit: Query<(Entity, &FitWithin, Option<&Parent>, &mut Transform)>) {
    let updates = q_fit
        .iter()
        .filter_map(|(entity, fit, parent, transform)| {
            let Ok((_, parent_fit, _, _)) = q_fit.get(**(parent?)) else {
                return None;
            };
            Some((
                entity,
                // TODO: unsure why this needs to be Y-reflected
                (fit.rect.center() - parent_fit.rect.center()) * Vec2::new(1., -1.),
            ))
        })
        .collect::<Vec<_>>();
    for (entity, translate) in updates {
        let Ok((_, _, _, mut transform)) = q_fit.get_mut(entity) else {
            continue;
        };
        transform.translation.x = translate.x;
        transform.translation.y = translate.y;
    }
}

fn show_fit(mut q_fit: Query<(&mut FitWithin, &mut Sprite)>) {
    for (mut fit, mut sprite) in &mut q_fit {
        if fit.updating {
            // ("updating {:?}", fit);
            // sprite.custom_size = Some(fit.rect.size());
            fit.updating = false;
        }
        // transform.translation.x = -center.x;
        // transform.translation.y = -center.y;
        // *transform = Transform::from_xyz(-center.x, -center.y, 0.);
    }
}

fn mouse_inside_window(
    q_primary_window: Single<&Window, With<PrimaryWindow>>,
    q_camera: Single<(&Camera, &GlobalTransform)>,
    mut q_fit_root: Query<(&FitWithin, &mut FitInteraction)>,
    // parent: Query<()>,
    // mut child: Query<()>,
) {
    let Some(cursor) = q_primary_window.cursor_position() else {
        for (_, mut interaction) in &mut q_fit_root {
            *interaction = FitInteraction::None;
        }
        return;
    };
    for (fit_within, mut interaction) in &mut q_fit_root {
        *interaction = if fit_within.rect.contains(cursor) {
            FitInteraction::Hover
        } else {
            FitInteraction::None
        };
    }
}

fn interact_cell(
    mut interaction_query: Query<
        (&FitInteraction, &mut Transform),
        (Changed<FitInteraction>, With<DisplayCellButton>),
    >,
) {
    for (interaction, mut transform) in &mut interaction_query {
        let scale = match *interaction {
            FitInteraction::Hover => 1.25,
            FitInteraction::None => 1.,
        };
        transform.scale.x = scale;
        transform.scale.y = scale;
    }
}

fn cell_clicked_down(
    ev: Trigger<Pointer<Down>>,
    q_camera: Single<(&Camera, &GlobalTransform)>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_cell: Query<(
        Entity,
        &DisplayCellButton,
        &FitInteraction,
        &GlobalTransform,
        &Sprite,
    )>,
    // mut writer: EventWriter<StartButtonDrag>,
    mut commands: Commands,
) {
    let (camera, camera_transform) = *q_camera;
    let Some(logical_viewport) = camera.logical_viewport_rect() else {
        return;
    };
    let Some(window) = q_window.iter().next() else {
        return;
    };
    let Some(cursor_loc) = window.cursor_position() else {
        return;
    };
    let window_center = logical_viewport.center();
    for (entity, button, interaction, &transform, sprite) in &q_cell {
        if matches!(interaction, FitInteraction::Hover) {
            // info!("starting drag {:?} {:?}", entity, sprite.color);
            let translate = (cursor_loc - window_center) * Vec2::new(1., -1.);
            commands
                .spawn((
                    Sprite::from_color(sprite.color.with_alpha(0.5), Vec2::new(100., 100.)),
                    Transform::from_xyz(translate.x, translate.y, 15.),
                    DragTarget {
                        start: cursor_loc,
                        latest: cursor_loc,
                    },
                    button.clone(),
                ))
                .with_child((Text2d::new("drag"), Transform::from_xyz(0., 0., 1.)));
            let mut transform = transform.compute_transform();
            transform.translation.z += 10.;
            commands
                .spawn((
                    Sprite::from_color(Color::hsla(0., 0., 0.5, 0.8), Vec2::new(200., 200.)),
                    transform,
                    DragUI,
                ))
                .with_children(|actions_spawner| {
                    actions_spawner.spawn((Text2d::new("Clear"), Transform::from_xyz(50., 0., 1.)));
                    actions_spawner.spawn((Text2d::new("Set"), Transform::from_xyz(0., -50., 1.)));
                    actions_spawner
                        .spawn((Text2d::new("Toggle"), Transform::from_xyz(-50., 0., 1.)));
                    actions_spawner.spawn((Text2d::new("Solo"), Transform::from_xyz(0., 50., 1.)));
                });
            // info!(
            //     "down ev={:#?} button={:#?} int={:#?} transform={:#?} local={:#?} iso={:#?}",
            //     ev,
            //     button,
            //     (),
            //     // interaction,
            //     transform,
            //     transform.compute_transform(),
            //     transform.to_isometry()
            // );
            // let loc = &ev.event().pointer_location;
            // writer.send(dbg!(StartButtonDrag { entity }));
        }
    }
}

fn cell_start_drag(
    mut commands: Commands,
    mut cell_index_query: Query<&DisplayCellButton>,
    mut reader: EventReader<StartButtonDrag>,
) {
    // for &StartButtonDrag { entity } in reader.read() {
    //     // let () = cell_index_query.get(entity);
    // }
}

fn cell_continue_drag(
    q_camera: Single<(&Camera, &GlobalTransform)>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut q_transform: Query<(&mut Transform, &mut DragTarget)>,
) {
    let (camera, camera_transform) = *q_camera;
    let Some(logical_viewport) = camera.logical_viewport_rect() else {
        return;
    };
    let Some(window) = q_window.iter().next() else {
        return;
    };
    let Some(cursor_loc) = window.cursor_position() else {
        return;
    };
    let window_center = logical_viewport.center();
    for (mut transform, mut drag_target) in &mut q_transform {
        let translate = (cursor_loc - window_center) * Vec2::new(1., -1.);
        transform.translation.x = translate.x;
        transform.translation.y = translate.y;
        drag_target.latest = cursor_loc;
    }
}

fn cell_release_drag(
    mut commands: Commands,
    q_cell: Query<(Entity, &DisplayCellButton, &DragTarget)>,
    q_dragui: Query<Entity, With<DragUI>>,
    mut writer: EventWriter<UpdateCellIndex>,
) {
    for (entity, &DisplayCellButton { index }, drag_target) in &q_cell {
        let distance = drag_target.start.distance(drag_target.latest);
        let angle = (drag_target.start - drag_target.latest).to_angle() + std::f32::consts::PI;
        let sectors = 4;
        let frac_adjust = 1. / sectors as f32 / 2.;
        let pre_angle_frac = angle / std::f32::consts::TAU;
        let angle_frac = (pre_angle_frac + frac_adjust) % 1.;
        let sector = (angle_frac * sectors as f32).floor();
        // info!("drag release distance={distance} sector={sector}");
        if distance > 5. {
            let op = match sector as u8 {
                0 => UpdateCellIndexOperation::Clear,
                1 => UpdateCellIndexOperation::Set,
                2 => UpdateCellIndexOperation::Toggle,
                3 => UpdateCellIndexOperation::Solo,
                _ => unreachable!(),
            };
            writer.send(UpdateCellIndex { index, op });
        }
        commands.entity(entity).despawn_recursive();
    }
    for entity in &q_dragui {
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
    for (&DisplayCellButton { index }, interaction) in &cell_query {
        // info!("cell={cell:?} int={interaction:?}");
        if matches!(interaction, Interaction::Pressed) {
            writer.send(UpdateCellIndex {
                index,
                op: UpdateCellIndexOperation::Toggle,
            });
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
        //     "updating: index={index:?} op={op:?} entity={entity:?} state={:x?}",
        //     puzzle_cell.enabled.as_slice()
        // );
        writer.send(UpdateCellDisplay { loc: index.loc });
    }
}

fn cell_update_display(
    puzzle: Single<&Puzzle>,
    mut cell_index_query: Query<(&DisplayCellButton, &mut Sprite)>,
    mut reader: EventReader<UpdateCellDisplay>,
) {
    let mut entity_map = HashMap::<_, Vec<_>>::new();
    for (&DisplayCellButton { index }, sprite) in &mut cell_index_query {
        entity_map
            .entry(index.loc)
            .or_default()
            .push((index, sprite));
    }
    for &UpdateCellDisplay { loc } in reader.read() {
        let cell = puzzle.cell(loc);
        let Some(buttons) = entity_map.get_mut(&loc) else {
            unreachable!()
        };
        // info!("updating cell={cell:?}");
        buttons.sort_by_key(|(index, _)| *index);
        for (index, sprite) in buttons.iter_mut() {
            let alpha = if cell.enabled.contains(index.index) {
                1.
            } else {
                0.2
            };
            // info!("  sprite @{index:?} = a{alpha}");
            sprite.color.set_alpha(alpha);
        }
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(<Puzzle as Default>::default());
    commands.insert_resource(PuzzleSpawn {
        timer: Timer::new(Duration::from_secs_f32(0.1), TimerMode::Repeating),
    });

    commands.spawn((
        DisplayMatrix,
        FitWithinBundle::new(),
        // RandomColorSprite::new(),
    ));
}
