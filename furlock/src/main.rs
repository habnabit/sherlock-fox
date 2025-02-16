// © 2025 <_@habnab.it>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{any::TypeId, time::Duration};

use bevy::{
    animation::{
        animated_field, AnimationEntityMut, AnimationEvaluationError, AnimationTarget,
        AnimationTargetId,
    },
    color::palettes::css,
    input::common_conditions::input_just_released,
    prelude::*,
    utils::hashbrown::HashMap,
    window::PrimaryWindow,
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use fixedbitset::FixedBitSet;
use petgraph::graph::NodeIndex;
use rand::{distr::Distribution, seq::SliceRandom, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use uuid::Uuid;

fn main() {
    App::new()
        .init_resource::<SeededRng>()
        .add_plugins(DefaultPlugins)
        .add_plugins(WorldInspectorPlugin::new())
        .add_event::<AddRow>()
        .add_event::<UpdateCellDisplay>()
        .add_event::<UpdateCellIndex>()
        .register_type::<AssignRandomColor>()
        .register_type::<CellLoc>()
        .register_type::<CellLocIndex>()
        .register_type::<DisplayCell>()
        .register_type::<DisplayCellButton>()
        .register_type::<DisplayMatrix>()
        .register_type::<DisplayRow>()
        .register_type::<DragTarget>()
        .register_type::<DragUI>()
        .register_type::<DragUITarget>()
        .register_type::<FitHover>()
        .register_type::<FitWithin>()
        .register_type::<HoverAlphaEdge>()
        .register_type::<HoverScaleEdge>()
        .register_type::<Puzzle>()
        .register_type::<PuzzleCell>()
        .register_type::<PuzzleRow>()
        .register_type::<SeededRng>()
        .register_type::<UpdateCellIndexOperation>()
        .add_observer(cell_clicked_down)
        .add_observer(cell_continue_drag)
        .add_observer(fit_inside_cell)
        .add_observer(fit_inside_matrix)
        .add_observer(fit_inside_row)
        .add_observer(fit_to_transform)
        .add_observer(interact_cell_generic::<OnAdd>(1.25))
        .add_observer(interact_cell_generic::<OnRemove>(1.0))
        .add_observer(interact_drag_ui_move)
        .add_observer(mouse_out_fit)
        .add_observer(mouse_over_fit)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                assign_random_color,
                // (
                fit_inside_window.run_if(any_with_component::<PrimaryWindow>),
                // fit_inside_row,
                // fit_inside_cell,
                // )
                //     .chain(),
                // (
                //     mouse_inside_window.run_if(any_with_component::<PrimaryWindow>),
                //     interact_cell,
                // )
                //     .chain(),
                cell_release_drag.run_if(input_just_released(MouseButton::Left)),
                (cell_update, cell_update_display).chain(),
                (spawn_row, add_row).chain(),
            ),
        )
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
}

impl FitWithin {
    fn new(rect: Rect) -> Self {
        FitWithin { rect }
    }

    fn refresh_rect(&self, commands: &mut Commands, me: Entity) {
        // info!("refresh_rect: me={me:?} >{:?}", self.rect);
        commands.entity(me).insert(FitWithin { rect: self.rect });
    }

    fn set_rect(&self, commands: &mut Commands, me: Entity, new_rect: Rect) {
        if self.rect != new_rect {
            // info!("set_rect: me={me:?} {:?} -> {:?}", self.rect, new_rect);
            commands.entity(me).insert(FitWithin { rect: new_rect });
        }
    }
}

#[derive(Reflect, Debug, Component)]
struct FitHover;

#[derive(Bundle)]
struct FitWithinBundle {
    fit: FitWithin,
    transform: Transform,
    visibility: InheritedVisibility,
}

impl FitWithinBundle {
    fn new() -> Self {
        FitWithinBundle {
            fit: FitWithin::default(),
            transform: Transform::default(),
            visibility: InheritedVisibility::VISIBLE,
        }
    }
}

#[derive(Bundle)]
struct HoverAnimationBundle {
    target: AnimationTarget,
    scale_tracker: HoverScaleEdge,
    alpha_tracker: HoverAlphaEdge,
}

impl HoverAnimationBundle {
    fn new(player: Entity) -> Self {
        HoverAnimationBundle {
            target: AnimationTarget {
                id: AnimationTargetId(Uuid::new_v4()),
                player,
            },
            scale_tracker: Default::default(),
            alpha_tracker: Default::default(),
        }
    }
}

impl Default for HoverAnimationBundle {
    fn default() -> Self {
        HoverAnimationBundle::new(Entity::PLACEHOLDER)
    }
}

#[derive(Bundle)]
struct RowAnimationBundle {
    target: AnimationTarget,
    translation_tracker: RowMoveEdge,
}

impl RowAnimationBundle {
    fn new(player: Entity) -> Self {
        RowAnimationBundle {
            target: AnimationTarget {
                id: AnimationTargetId(Uuid::new_v4()),
                player,
            },
            translation_tracker: Default::default(),
        }
    }
}

impl Default for RowAnimationBundle {
    fn default() -> Self {
        RowAnimationBundle::new(Entity::PLACEHOLDER)
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

#[derive(Reflect, Debug, Component, Clone, Default)]
struct HoverScaleEdge(Option<NodeIndex>);

#[derive(Reflect, Debug, Component, Clone, Default)]
struct HoverAlphaEdge(Option<NodeIndex>);

#[derive(Reflect, Debug, Component, Clone, Default)]
struct RowMoveEdge(Option<NodeIndex>);

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

#[derive(Reflect, Debug, Component, Default)]
struct DragUI;

#[derive(Reflect, Debug, Component)]
struct DragUITarget(UpdateCellIndexOperation);

#[derive(Reflect, Debug, Component, Default)]
struct DragTarget {
    start: Vec2,
    latest: Vec2,
    op: Option<UpdateCellIndexOperation>,
}

impl DragTarget {
    fn new(start: Vec2) -> Self {
        DragTarget {
            latest: start,
            start,
            op: None,
        }
    }
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
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
    matrix_entity: Single<(Entity, &FitWithin), With<DisplayMatrix>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
) {
    let (matrix, matrix_fit) = *matrix_entity;
    let mut spawned = false;
    for ev in reader.read() {
        let row_nr = puzzle.rows.len();
        let colors = random_colors(ev.len, &mut rng.0);
        // info!("spawning row {:?}", colors);
        puzzle.add_row(PuzzleRow::new(colors));

        commands.entity(matrix).with_children(|matrix_spawner| {
            matrix_spawner
                .spawn((
                    FitWithinBundle::new(),
                    // RandomColorSprite::new(),
                    DisplayRow { row_nr },
                    RowAnimationBundle::new(matrix),
                ))
                .with_children(|row_spawner| {
                    for cell_nr in 0..ev.len {
                        let loc = CellLoc { row_nr, cell_nr };
                        let graph = AnimationGraph::new();
                        let cell_player = row_spawner
                            .spawn((
                                AnimationPlayer::default(),
                                AnimationGraphHandle(animation_graphs.add(graph)),
                            ))
                            .id();
                        row_spawner
                            .spawn((
                                FitWithinBundle::new(),
                                // RandomColorSprite::new(),
                                DisplayCell { loc },
                            ))
                            .with_children(|cell_spawner| {
                                for index in 0..ev.len {
                                    cell_spawner
                                        .spawn((
                                            Sprite::from_color(
                                                puzzle.rows[row_nr].colors[index],
                                                Vec2::new(25., 25.),
                                            ),
                                            FitWithinBundle::new(),
                                            DisplayCellButton {
                                                index: CellLocIndex { loc, index },
                                            },
                                            HoverAnimationBundle::new(cell_player),
                                        ))
                                        .with_child((
                                            Text2d::new(format!("{index}")),
                                            Transform::from_xyz(0., 0., 1.),
                                        ));
                                }
                            });
                    }
                });
        });

        spawned = true;
    }

    if spawned {
        matrix_fit.refresh_rect(&mut commands, matrix);
    }
}

fn fit_inside_window(
    q_camera: Query<(Entity, &Camera)>,
    q_fit_root: Query<(Entity, &FitWithin), Without<Parent>>,
    mut commands: Commands,
) {
    let (camera_entity, camera) = q_camera.single();
    let Some(logical_viewport) = camera.logical_viewport_rect() else {
        return;
    };
    let window_size = logical_viewport.inflate(-50.);
    // info!("ensuring window fit of window({:?}) {:?} {:?}", window_size, camera_entity, camera);
    for (entity, fit_within) in &q_fit_root {
        fit_within.set_rect(&mut commands, entity, window_size);
    }
}

fn fit_inside_matrix(
    ev: Trigger<OnInsert, (FitWithin, DisplayMatrix)>,
    q_about_target: Query<(&FitWithin, &Children), (With<DisplayMatrix>, Without<DisplayRow>)>,
    q_children: Query<(Entity, &FitWithin, &DisplayRow)>,
    mut commands: Commands,
) {
    // info!("testing matrix fit of {:?}", ev.entity());
    let Ok((within, children)) = q_about_target.get(ev.entity()) else {
        return;
    };
    // info!(
    //     " + fitting row inside matrix {:?} {:?}",
    //     within,
    //     children.len()
    // );
    let children = {
        let mut children = children
            .iter()
            .filter_map(|e| q_children.get(*e).ok())
            .collect::<Vec<_>>();
        children.sort_by_key(|(_, _, row)| row.row_nr);
        children
    };
    let fit = within.rect;
    let fit_height = fit.height();
    let row_height = fit_height / children.len() as f32;
    let mut current_y = fit.max.y;
    for (entity, fit_within, _) in children {
        let new_y = current_y - row_height;
        let row_rect =
            Rect::from_corners(Vec2::new(fit.min.x, current_y), Vec2::new(fit.max.x, new_y));
        fit_within.set_rect(&mut commands, entity, row_rect);
        current_y = new_y;
    }
}

fn fit_inside_row(
    ev: Trigger<OnInsert, (FitWithin, DisplayRow)>,
    q_about_target: Query<(&FitWithin, &Children), (With<DisplayRow>, Without<DisplayCell>)>,
    q_children: Query<(Entity, &FitWithin, &DisplayCell)>,
    mut commands: Commands,
) {
    // info!("testing matrix row fit of {:?}", ev.entity());
    let Ok((within, children)) = q_about_target.get(ev.entity()) else {
        return;
    };
    // info!(
    //     " + fitting row inside matrix {:?} {:?}",
    //     within,
    //     children.len()
    // );
    let children = {
        let mut children = children
            .iter()
            .filter_map(|e| q_children.get(*e).ok())
            .collect::<Vec<_>>();
        children.sort_by_key(|(_, _, cell)| cell.loc);
        children
    };
    let fit = within.rect;
    let fit_width = fit.width();
    let prospective_cell_width = fit_width / children.len() as f32;
    let cell_spacing = prospective_cell_width * 0.15;
    let total_cell_spacing = cell_spacing * (children.len() - 1) as f32;
    let cell_width = (fit_width - total_cell_spacing) / children.len() as f32;
    let mut current_x = fit.min.x;
    for (entity, fit_within, _) in children {
        let new_x = current_x + cell_width;
        let cell_rect =
            Rect::from_corners(Vec2::new(current_x, fit.min.y), Vec2::new(new_x, fit.max.y));
        fit_within.set_rect(&mut commands, entity, cell_rect);
        current_x = new_x + cell_spacing;
    }
}

fn fit_inside_cell(
    ev: Trigger<OnInsert, (FitWithin, DisplayCell)>,
    q_about_target: Query<(&FitWithin, &Children), (With<DisplayCell>, Without<DisplayCellButton>)>,
    q_children: Query<(Entity, &FitWithin, &DisplayCellButton)>,
    mut commands: Commands,
) {
    // info!("testing matrix cell fit of {:?}", ev.entity());
    let Ok((within, children)) = q_about_target.get(ev.entity()) else {
        return;
    };
    // info!(
    //     " + fitting button inside cell {:?} {:?}",
    //     within,
    //     children.len()
    // );
    let children = {
        let mut children = children
            .iter()
            .filter_map(|e| q_children.get(*e).ok())
            .collect::<Vec<_>>();
        children.sort_by_key(|(_, _, button)| button.index);
        children
    };
    let fit = within.rect;
    let fit_width = fit.width();
    let button_width = fit_width / children.len() as f32;
    let mut current_x = fit.min.x;
    for (entity, fit_within, _) in children {
        let new_x = current_x + button_width;
        let button_rect =
            Rect::from_corners(Vec2::new(current_x, fit.min.y), Vec2::new(new_x, fit.max.y));
        fit_within.set_rect(&mut commands, entity, button_rect);
        current_x = new_x;
    }
}

fn fit_to_transform(
    ev: Trigger<OnInsert, FitWithin>,
    mut q_fit: Query<(Entity, &FitWithin, &Parent, &mut Transform)>,
    q_just_fit: Query<&FitWithin>,
    mut q_animation: Query<(&AnimationTarget, &mut RowMoveEdge)>,
    mut q_reader: Query<(&mut AnimationPlayer, &AnimationGraphHandle)>,
    mut animation_clips: ResMut<Assets<AnimationClip>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
) {
    let Ok((entity, fit, parent, mut transform)) = q_fit.get_mut(ev.entity()) else {
        return;
    };
    let Ok(parent_fit) = q_just_fit.get(**parent) else {
        return;
    };
    // info!("fit to transform before={fit:?}");
    // TODO: unsure why this needs to be Y-reflected
    let translate = (fit.rect.center() - parent_fit.rect.center()) * Vec2::new(1., -1.);
    let animation_info = q_animation
        .get_mut(entity)
        .ok()
        .and_then(|(target, row_edge)| {
            let (player, graph_handle) = q_reader.get_mut(target.player).ok()?;
            let graph = animation_graphs.get_mut(graph_handle.id())?;
            Some((target, row_edge, player, graph))
        });
    if let Some((target, mut row_edge, mut player, graph)) = animation_info {
        let translate = (translate, 0.).into();

        let mut clip = AnimationClip::default();
        clip.add_curve_to_target(
            target.id,
            AnimatableCurve::new(
                animated_field!(Transform::translation),
                EasingCurve::new(transform.translation, translate, EaseFunction::CubicOut)
                    .reparametrize_linear(interval(0., 0.25).unwrap())
                    .unwrap(),
            ),
        );

        if let Some(prev_node) = row_edge.0 {
            graph.remove_edge(graph.root, prev_node);
        }
        let clip_handle = animation_clips.add(clip);
        let node_index = graph.add_clip(clip_handle, 1., graph.root);
        player.play(node_index);
        row_edge.0 = Some(node_index);
    } else {
        transform.translation.x = translate.x;
        transform.translation.y = translate.y;
    }
}

fn mouse_over_fit(ev: Trigger<Pointer<Over>>, mut commands: Commands) {
    // info!("mouse over fit {ev:?}");
    let Some(mut cmd) = commands.get_entity(ev.target) else {
        return;
    };
    cmd.insert(FitHover);
}

fn mouse_out_fit(ev: Trigger<Pointer<Out>>, mut commands: Commands) {
    // info!("mouse out fit {ev:?}");
    let Some(mut cmd) = commands.get_entity(ev.target) else {
        return;
    };
    cmd.remove::<FitHover>();
}

fn interact_cell_generic<T>(
    target_scale_xy: f32,
) -> impl Fn(
    Trigger<T, FitHover>,
    Query<(&Transform, &AnimationTarget, &mut HoverScaleEdge), With<DisplayCellButton>>,
    Query<(&mut AnimationPlayer, &AnimationGraphHandle)>,
    ResMut<Assets<AnimationClip>>,
    ResMut<Assets<AnimationGraph>>,
) {
    move |ev, mut q_target, mut q_player, mut animation_clips, mut animation_graphs| {
        let Ok((transform, target, mut hover_edge)) = q_target.get_mut(ev.entity()) else {
            return;
        };
        let Ok((mut player, graph_handle)) = q_player.get_mut(target.player) else {
            return;
        };
        let Some(graph) = animation_graphs.get_mut(graph_handle.id()) else {
            return;
        };

        let mut clip = AnimationClip::default();
        clip.add_curve_to_target(
            target.id,
            AnimatableCurve::new(
                animated_field!(Transform::scale),
                EasingCurve::new(
                    transform.scale,
                    Vec3::new(target_scale_xy, target_scale_xy, 1.0),
                    EaseFunction::CubicOut,
                )
                .reparametrize_linear(interval(0., 0.25).unwrap())
                .unwrap(),
            ),
        );
        let clip_handle = animation_clips.add(clip);
        if let Some(prev_node) = hover_edge.0 {
            graph.remove_edge(graph.root, prev_node);
        }
        let node_index = graph.add_clip(clip_handle, 1., graph.root);
        player.play(node_index);
        hover_edge.0 = Some(node_index);
    }
}

fn interact_drag_ui_move(
    _ev: Trigger<Pointer<Move>>,
    q_target: Query<&DragTarget>,
    mut q_transform: Query<(&mut Transform, &DragUITarget)>,
) {
    let Some(drag_target) = q_target.iter().next() else {
        return;
    };
    for (mut transform, ui_target) in &mut q_transform {
        let scale = if drag_target.op == Some(ui_target.0) {
            1.25
        } else {
            1.
        };
        transform.scale.x = scale;
        transform.scale.y = scale;
    }
}

fn cell_clicked_down(
    mut ev: Trigger<Pointer<Down>>,
    q_camera: Single<&Camera>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_cell: Query<(&DisplayCellButton, &GlobalTransform, &Sprite), With<FitHover>>,
    // q_ui: Query<Entity, With<DragUI>>,
    mut commands: Commands,
) {
    let Some(logical_viewport) = q_camera.logical_viewport_rect() else {
        return;
    };
    let Some(window) = q_window.iter().next() else {
        return;
    };
    let Some(cursor_loc) = window.cursor_position() else {
        return;
    };
    let window_center = logical_viewport.center();
    let mut dragged = false;
    for (button, &transform, sprite) in &q_cell {
        let translate = (cursor_loc - window_center) * Vec2::new(1., -1.);
        commands.spawn((
            Sprite::from_color(sprite.color.with_alpha(0.5), Vec2::new(100., 100.)),
            Transform::from_xyz(translate.x, translate.y, 15.),
            DragTarget::new(cursor_loc),
            button.clone(),
        ));
        let mut transform = transform.compute_transform();
        transform.translation.z += 10.;
        commands
            .spawn((
                Sprite::from_color(Color::hsla(0., 0., 0.5, 0.8), Vec2::new(200., 200.)),
                transform,
                DragUI,
            ))
            .with_children(|actions_spawner| {
                actions_spawner.spawn((
                    Text2d::new("Clear"),
                    Transform::from_xyz(50., 0., 1.),
                    DragUITarget(UpdateCellIndexOperation::Clear),
                ));
                actions_spawner.spawn((
                    Text2d::new("Set"),
                    Transform::from_xyz(0., -50., 1.),
                    DragUITarget(UpdateCellIndexOperation::Set),
                ));
                actions_spawner.spawn((
                    Text2d::new("Toggle"),
                    Transform::from_xyz(-50., 0., 1.),
                    DragUITarget(UpdateCellIndexOperation::Toggle),
                ));
                actions_spawner.spawn((
                    Text2d::new("Solo"),
                    Transform::from_xyz(0., 50., 1.),
                    DragUITarget(UpdateCellIndexOperation::Solo),
                ));
            });
        dragged = true;
    }
    if dragged {
        ev.propagate(false);
    }
}

fn cell_continue_drag(
    ev: Trigger<Pointer<Move>>,
    q_camera: Single<&Camera>,
    mut q_transform: Query<(&mut Transform, &mut DragTarget)>,
) {
    let Some(logical_viewport) = q_camera.logical_viewport_rect() else {
        return;
    };
    let cursor_loc = ev.pointer_location.position;
    let window_center = logical_viewport.center();
    let translate = (cursor_loc - window_center) * Vec2::new(1., -1.);
    for (mut transform, mut drag_target) in &mut q_transform {
        transform.translation.x = translate.x;
        transform.translation.y = translate.y;
        drag_target.latest = cursor_loc;
        let distance = drag_target.start.distance(drag_target.latest);
        let angle = (drag_target.start - drag_target.latest).to_angle() + std::f32::consts::PI;
        let sectors = 4;
        let frac_adjust = 1. / sectors as f32 / 2.;
        let pre_angle_frac = angle / std::f32::consts::TAU;
        let angle_frac = (pre_angle_frac + frac_adjust) % 1.;
        let sector = (angle_frac * sectors as f32).floor();
        // info!("drag release distance={distance} sector={sector}");
        drag_target.op = if distance > 10. && distance < 125. {
            match sector as u8 {
                0 => Some(UpdateCellIndexOperation::Clear),
                1 => Some(UpdateCellIndexOperation::Set),
                2 => Some(UpdateCellIndexOperation::Toggle),
                3 => Some(UpdateCellIndexOperation::Solo),
                _ => None,
            }
        } else {
            None
        };
    }
}

fn cell_release_drag(
    mut commands: Commands,
    q_cell: Query<(Entity, &DisplayCellButton, &DragTarget)>,
    q_dragui: Query<Entity, With<DragUI>>,
    mut writer: EventWriter<UpdateCellIndex>,
) {
    for (entity, &DisplayCellButton { index }, drag_target) in &q_cell {
        if let Some(op) = drag_target.op {
            writer.send(UpdateCellIndex { index, op });
        }
        commands.entity(entity).despawn_recursive();
    }
    for entity in &q_dragui {
        commands.entity(entity).despawn_recursive();
    }
}

fn cell_update(
    mut puzzle: Single<&mut Puzzle>,
    mut reader: EventReader<UpdateCellIndex>,
    mut writer: EventWriter<UpdateCellDisplay>,
) {
    for &UpdateCellIndex { index, op } in reader.read() {
        let puzzle_cell = puzzle.cell_mut(index.loc);
        puzzle_cell.apply(index.index, op);
        writer.send(UpdateCellDisplay { loc: index.loc });
    }
}

#[derive(Debug, Clone, Copy)]
struct ButtonOpacityAnimation;

impl AnimatableProperty for ButtonOpacityAnimation {
    type Property = f32;

    fn evaluator_id(&self) -> EvaluatorId {
        EvaluatorId::Type(TypeId::of::<Self>())
    }

    fn get_mut<'a>(
        &self,
        entity: &'a mut AnimationEntityMut,
    ) -> Result<&'a mut Self::Property, AnimationEvaluationError> {
        let sprite = entity
            .get_mut::<Sprite>()
            .ok_or(AnimationEvaluationError::ComponentNotPresent(TypeId::of::<
                Sprite,
            >(
            )))?
            .into_inner();
        match &mut sprite.color {
            Color::Hsla(color) => Ok(&mut color.alpha),
            _ => Err(AnimationEvaluationError::PropertyNotPresent(TypeId::of::<
                Color,
            >(
            ))),
        }
    }
}

fn cell_update_display(
    puzzle: Single<&Puzzle>,
    mut reader: EventReader<UpdateCellDisplay>,
    mut q_cell: Query<(
        &DisplayCellButton,
        &mut Sprite,
        &mut AnimationTarget,
        &mut HoverAlphaEdge,
    )>,
    mut q_reader: Query<(&mut AnimationPlayer, &AnimationGraphHandle)>,
    mut animation_clips: ResMut<Assets<AnimationClip>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
) {
    let mut entity_map = HashMap::<_, Vec<_>>::new();
    for (&DisplayCellButton { index }, sprite, target, hover_edge) in &mut q_cell {
        entity_map
            .entry(index.loc)
            .or_default()
            .push((index, sprite, target, hover_edge));
    }
    for &UpdateCellDisplay { loc } in reader.read() {
        let cell = puzzle.cell(loc);
        let Some(buttons) = entity_map.get_mut(&loc) else {
            unreachable!()
        };
        // info!("updating cell={cell:?}");
        buttons.sort_by_key(|t| t.0);

        for (index, sprite, target, hover_edge) in buttons.iter_mut() {
            let Ok((mut player, graph_handle)) = q_reader.get_mut(target.player) else {
                continue;
            };
            let Some(graph) = animation_graphs.get_mut(graph_handle.id()) else {
                continue;
            };
            let alpha = if cell.enabled.contains(index.index) {
                1.
            } else {
                0.2
            };

            let mut clip = AnimationClip::default();
            clip.add_curve_to_target(
                target.id,
                AnimatableCurve::new(
                    ButtonOpacityAnimation,
                    EasingCurve::new(sprite.color.alpha(), alpha, EaseFunction::CubicOut)
                        .reparametrize_linear(interval(0., 0.25).unwrap())
                        .unwrap(),
                ),
            );

            if let Some(prev_node) = hover_edge.0 {
                graph.remove_edge(graph.root, prev_node);
            }
            let clip_handle = animation_clips.add(clip);
            let node_index = graph.add_clip(clip_handle, 1., graph.root);
            player.play(node_index);
            hover_edge.0 = Some(node_index);
        }
    }
}

fn setup(mut commands: Commands, mut animation_graphs: ResMut<Assets<AnimationGraph>>) {
    commands.spawn(Camera2d);
    commands.spawn(<Puzzle as Default>::default());
    commands.insert_resource(PuzzleSpawn {
        timer: Timer::new(Duration::from_secs_f32(0.25), TimerMode::Repeating),
    });
    commands.spawn((
        DisplayMatrix,
        FitWithinBundle::new(),
        AnimationPlayer::default(),
        AnimationGraphHandle(animation_graphs.add(AnimationGraph::new())),
        // RandomColorSprite::new(),
    ));
}
