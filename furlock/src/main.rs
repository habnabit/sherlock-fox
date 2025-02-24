// © 2025 <_@habnab.it>
//
// SPDX-License-Identifier: EUPL-1.2

#![feature(try_blocks, cmp_minmax)]

mod clues;
mod puzzle;

use std::{any::TypeId, time::Duration};

use bevy::{
    animation::{
        animated_field, AnimationEntityMut, AnimationEvaluationError, AnimationTarget,
        AnimationTargetId,
    },
    color::palettes::css,
    input::common_conditions::{input_just_pressed, input_just_released},
    prelude::*,
    utils::hashbrown::{HashMap, HashSet},
    window::PrimaryWindow,
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use clues::{
    AdjacentColumnClue, BetweenColumnsClue, ClueExplanation, ClueExplanationResolvedChunk,
    DynPuzzleClue, PuzzleClues, SameColumnClue,
};
use petgraph::graph::NodeIndex;
use puzzle::{
    CellLoc, CellLocIndex, Puzzle, PuzzleCellDisplay, PuzzleCellSelection, PuzzleRow,
    UpdateCellIndexOperation,
};
use rand::{
    distr::Distribution,
    seq::{IndexedRandom, SliceRandom},
    Rng, SeedableRng,
};
use rand_chacha::ChaCha8Rng;
use uuid::Uuid;

const NO_PICK: PickingBehavior = PickingBehavior {
    should_block_lower: false,
    is_hoverable: false,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<Assets<DynPuzzleClue>>()
        .init_resource::<SeededRng>()
        .init_state::<ClueExplanationState>()
        .add_plugins(WorldInspectorPlugin::new())
        .add_event::<AddClue>()
        .add_event::<AddRow>()
        .add_event::<UpdateCellDisplay>()
        .add_event::<UpdateCellIndex>()
        .register_asset_reflect::<DynPuzzleClue>()
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
        .register_type::<DynPuzzleClue>()
        .register_type::<FitHover>()
        .register_type::<FitWithin>()
        .register_type::<HoverAlphaEdge>()
        .register_type::<HoverScaleEdge>()
        .register_type::<Puzzle>()
        .register_type::<PuzzleCellDisplay>()
        .register_type::<PuzzleCellSelection>()
        .register_type::<PuzzleClueComponent>()
        .register_type::<PuzzleClues>()
        .register_type::<PuzzleRow>()
        .register_type::<PuzzleSpawn>()
        .register_type::<FitTransformEdge>()
        .register_type::<SameColumnClue>()
        .register_type::<SeededRng>()
        .register_type::<UpdateCellIndexOperation>()
        .add_observer(cell_clicked_down)
        .add_observer(cell_continue_drag)
        .add_observer(clue_explanation_clicked)
        .add_observer(fit_inside_cell)
        .add_observer(fit_inside_clues)
        .add_observer(fit_inside_matrix)
        .add_observer(fit_inside_puzzle)
        .add_observer(fit_inside_row)
        .add_observer(fit_to_transform)
        .add_observer(fit_background_sprite)
        .add_observer(interact_cell_generic::<OnAdd>(1.25))
        .add_observer(interact_cell_generic::<OnRemove>(1.0))
        .add_observer(interact_drag_ui_move)
        .add_observer(mouse_out_fit)
        .add_observer(mouse_over_fit)
        .add_observer(show_clue_highlight)
        .add_observer(remove_clue_highlight)
        .add_observer(make_fit_background_sprite)
        .add_observer(show_dyn_clue)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                assign_random_color,
                show_clues.run_if(input_just_pressed(KeyCode::KeyC)),
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
                add_clue,
            ),
        )
        .add_systems(OnEnter(ClueExplanationState::Shown), show_clue_explanation)
        .add_systems(OnExit(ClueExplanationState::Shown), hide_clue_explanation)
        .run();
}

#[derive(Resource, Reflect)]
#[reflect(Resource)]
#[reflect(from_reflect = false)]
struct SeededRng(#[reflect(ignore)] ChaCha8Rng);

impl FromReflect for SeededRng {
    fn from_reflect(reflect: &dyn PartialReflect) -> Option<Self> {
        todo!()
    }
}

impl FromWorld for SeededRng {
    fn from_world(_world: &mut World) -> Self {
        SeededRng(ChaCha8Rng::from_os_rng())
    }
}

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
enum ClueExplanationState {
    #[default]
    NotShown,
    Shown,
}

#[derive(Debug, Component, Reflect)]
struct ExplanationHilight;

#[derive(Debug, Component, Reflect)]
struct ExplainClueComponent {
    clue: Handle<DynPuzzleClue>,
    update: UpdateCellIndex,
}

fn show_clue_explanation(
    mut commands: Commands,
    q_puzzle: Single<&Puzzle>,
    q_clue: Query<(Entity, &ExplainClueComponent)>,
    q_clues: Query<(Entity, &PuzzleClueComponent)>,
    q_cell: Query<(Entity, &DisplayCellButton)>,
    // clues: Res<Assets<DynPuzzleClue>>,
) {
    #[derive(Debug, Default)]
    struct TextTaker(Option<String>);
    impl TextTaker {
        fn insert_str(&mut self, input: &str) {
            self.0.get_or_insert_default().push_str(input);
        }
        fn insert_string(&mut self, input: String) {
            match &mut self.0 {
                Some(s) => s.push_str(&input),
                p @ None => *p = Some(input),
            }
        }
        fn drain_into(&mut self, parent: &mut ChildBuilder) {
            if let Some(text) = self.0.take() {
                parent.spawn((
                    Text::new(text),
                    BackgroundColor(Color::hsla(0., 0., 0.1, 0.8)),
                    NO_PICK,
                ));
            }
        }
    }
    let Ok((clue_display_entity, clue_component)) = q_clue.get_single() else {
        return;
    };
    let clue_id = clue_component.clue.id();
    // let Some(clue) = clues.get(clue_id) else {
    //     return;
    // };
    let Some(ref explanation) = clue_component.update.explanation else {
        warn!("couldn't show explanation on {clue_component:#?}");
        return;
    };
    let Some((clue_entity, _)) = q_clues.iter().find(|(_, c)| c.0.id() == clue_id) else {
        return;
    };
    commands.entity(clue_entity).insert(ExplanationHilight);
    let mut cell_highlight = HashSet::new();
    commands
        .entity(clue_display_entity)
        .insert((
            Node {
                width: Val::Vw(35.),
                height: Val::Vh(30.),
                margin: UiRect::all(Val::Auto),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..Default::default()
            },
            BackgroundColor(Color::hsla(0., 0., 0.3, 0.25)),
        ))
        .with_children(|parent| {
            use ClueExplanationResolvedChunk as Ch;
            let mut built_text = TextTaker::default();
            for c in explanation.resolved() {
                match c {
                    Ch::Text(s) => {
                        built_text.insert_str(s);
                    }
                    Ch::Accessed(_name, cell_display) => {
                        built_text.drain_into(parent);
                        cell_display.spawn_into(*q_puzzle, parent);
                        if let Some(&loc) = cell_display.loc_index() {
                            cell_highlight.insert(loc);
                        }
                        // parent.spawn(Text::new(format!("<{name}: {cell_display:p}>")));
                    }
                    Ch::Eval(_expr, result) => {
                        built_text.insert_string(result);
                    }
                }
            }
            built_text.drain_into(parent);
        });

    for (cell, button) in &q_cell {
        if cell_highlight.contains(&button.index) {
            commands.entity(cell).insert(ExplanationHilight);
        }
    }
}

fn hide_clue_explanation(
    mut commands: Commands,
    // q_puzzle: Single<&Puzzle>,
    q_explanation: Query<(Entity, &ExplainClueComponent)>,
    q_clues: Query<Entity, With<ExplanationHilight>>,
    mut writer: EventWriter<UpdateCellIndex>,
) {
    for (explanation_entity, explanation) in &q_explanation {
        commands.entity(explanation_entity).despawn_recursive();
        writer.send(explanation.update.clone());
    }
    for clue_entity in &q_clues {
        commands.entity(clue_entity).remove::<ExplanationHilight>();
    }
}

// ev: Trigger<OnInsert, FitWithin>,
// ) {
// let Ok((entity, fit, parent, mut transform)) = q_fit.get_mut(ev.entity()) else {
//     return;
// };
// let Ok(parent_fit) = q_just_fit.get(**parent) else {
//     return;
// };
// // info!("fit to transform before={fit:?}");
// // TODO: unsure why this needs to be Y-reflected
// let translate = (fit.rect.center() - parent_fit.rect.center()) * Vec2::new(1., -1.);

fn show_clue_highlight(
    ev: Trigger<OnInsert, ExplanationHilight>,
    mut q_transform: Query<&mut Transform>,
    mut q_animation: Query<(&AnimationTarget, &mut ExplanationBounceEdge)>,
    mut q_reader: Query<(&mut AnimationPlayer, &AnimationGraphHandle)>,
    mut animation_clips: ResMut<Assets<AnimationClip>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
) {
    let Ok(mut transform) = q_transform.get_mut(ev.entity()) else {
        return;
    };

    let animation_info = q_animation
        .get_mut(ev.entity())
        .ok()
        .and_then(|(target, row_edge)| {
            let (player, graph_handle) = q_reader.get_mut(target.player).ok()?;
            let graph = animation_graphs.get_mut(graph_handle.id())?;
            Some((target, row_edge, player, graph))
        });
    let Some((target, mut row_edge, mut player, graph)) = animation_info else {
        return;
    };
    // let translate = (translate, 0.).into();
    let scale = Vec3::new(1.25, 1.25, 1.);

    let mut clip = AnimationClip::default();
    clip.add_curve_to_target(
        target.id,
        AnimatableCurve::new(
            animated_field!(Transform::scale),
            EasingCurve::new(transform.scale, scale, EaseFunction::SineInOut)
                .reparametrize_linear(interval(0., 0.5).unwrap())
                .unwrap()
                .ping_pong()
                .unwrap(),
        ),
    );

    if let Some(prev_node) = row_edge.0 {
        graph.remove_edge(graph.root, prev_node);
    }
    let clip_handle = animation_clips.add(clip);
    let node_index = graph.add_clip(clip_handle, 1., graph.root);
    player.play(node_index).repeat();
    row_edge.0 = Some(node_index);

    transform.translation.z += 10.;
}

fn remove_clue_highlight(
    ev: Trigger<OnRemove, ExplanationHilight>,
    mut q_transform: Query<&mut Transform>,
    mut q_animation: Query<(&AnimationTarget, &mut ExplanationBounceEdge)>,
    mut q_reader: Query<(&mut AnimationPlayer, &AnimationGraphHandle)>,
    mut animation_clips: ResMut<Assets<AnimationClip>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
) {
    let Ok(mut transform) = q_transform.get_mut(ev.entity()) else {
        return;
    };

    let animation_info = q_animation
        .get_mut(ev.entity())
        .ok()
        .and_then(|(target, row_edge)| {
            let (player, graph_handle) = q_reader.get_mut(target.player).ok()?;
            let graph = animation_graphs.get_mut(graph_handle.id())?;
            Some((target, row_edge, player, graph))
        });
    let Some((target, mut row_edge, mut player, graph)) = animation_info else {
        return;
    };
    let scale = Vec3::new(1., 1., 1.);

    let mut clip = AnimationClip::default();
    clip.add_curve_to_target(
        target.id,
        AnimatableCurve::new(
            animated_field!(Transform::scale),
            EasingCurve::new(transform.scale, scale, EaseFunction::SineOut)
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

    transform.translation.z -= 10.;
}

#[derive(Debug, Component, Reflect)]
struct PuzzleClueComponent(Handle<DynPuzzleClue>);

fn show_dyn_clue(
    ev: Trigger<OnInsert, PuzzleClueComponent>,
    q_clue: Query<&PuzzleClueComponent>,
    q_puzzle: Single<&Puzzle>,
    clues: Res<Assets<DynPuzzleClue>>,
    mut commands: Commands,
) {
    let puzzle = *q_puzzle;
    let Ok(clue) = q_clue.get(ev.entity()) else {
        return;
    };
    let Some(clue) = clues.get(clue.0.id()) else {
        return;
    };
    info!("dyn clue ev={ev:?} clue={clue:?}");
    commands
        .entity(ev.entity())
        .with_children(clue.spawn_into(puzzle));
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

#[derive(Reflect, Debug, Component, Default)]
struct FitWithinBackground {
    index: usize,
    color: Color,
}

impl FitWithinBackground {
    fn new(index: usize) -> Self {
        FitWithinBackground {
            index,
            color: Color::hsla(0., 0., 1., 1.),
        }
    }

    fn new_colored(index: usize, color: Color) -> Self {
        FitWithinBackground { index, color }
    }
}

#[derive(Bundle)]
struct HoverAnimationBundle {
    target: AnimationTarget,
    scale_tracker: HoverScaleEdge,
    alpha_tracker: HoverAlphaEdge,
    explanation_tracker: ExplanationBounceEdge,
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
            explanation_tracker: Default::default(),
        }
    }
}

impl Default for HoverAnimationBundle {
    fn default() -> Self {
        HoverAnimationBundle::new(Entity::PLACEHOLDER)
    }
}

#[derive(Bundle)]
struct FitTransformAnimationBundle {
    target: AnimationTarget,
    translation_tracker: FitTransformEdge,
}

impl FitTransformAnimationBundle {
    fn new(player: Entity) -> Self {
        FitTransformAnimationBundle {
            target: AnimationTarget {
                id: AnimationTargetId(Uuid::new_v4()),
                player,
            },
            translation_tracker: Default::default(),
        }
    }
}

impl Default for FitTransformAnimationBundle {
    fn default() -> Self {
        FitTransformAnimationBundle::new(Entity::PLACEHOLDER)
    }
}

#[derive(Bundle)]
struct ExplanationBounceAnimationBundle {
    target: AnimationTarget,
    scale_tracker: ExplanationBounceEdge,
    translation_tracker: FitTransformEdge,
}

impl ExplanationBounceAnimationBundle {
    fn new(player: Entity) -> Self {
        ExplanationBounceAnimationBundle {
            target: AnimationTarget {
                id: AnimationTargetId(Uuid::new_v4()),
                player,
            },
            scale_tracker: Default::default(),
            translation_tracker: Default::default(),
        }
    }
}

impl Default for ExplanationBounceAnimationBundle {
    fn default() -> Self {
        ExplanationBounceAnimationBundle::new(Entity::PLACEHOLDER)
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
struct DisplayPuzzle;

#[derive(Reflect, Debug, Component)]
struct DisplayCluebox;

#[derive(Reflect, Debug, Component)]
struct DisplayClue;

#[derive(Reflect, Debug, Component)]
struct DisplayMatrix;

#[derive(Reflect, Debug, Component)]
struct DisplayRow {
    row_nr: usize,
}

#[derive(Reflect, Debug, Component)]
struct DisplayCell {
    loc: CellLoc,
}

#[derive(Reflect, Debug, Component, Clone)]
struct DisplayCellButton {
    index: CellLocIndex,
}

#[derive(Reflect, Debug, Component, Clone)]
struct DisplayCellButtonEnlarge;

#[derive(Reflect, Debug, Component, Clone, Default)]
struct HoverScaleEdge(Option<NodeIndex>);

#[derive(Reflect, Debug, Component, Clone, Default)]
struct HoverAlphaEdge(Option<NodeIndex>);

#[derive(Reflect, Debug, Component, Clone, Default)]
struct FitTransformEdge(Option<NodeIndex>);

#[derive(Reflect, Debug, Component, Clone, Default)]
struct ExplanationBounceEdge(Option<NodeIndex>);

#[derive(Resource, Reflect)]
#[reflect(Resource)]
struct PuzzleSpawn {
    tileset_pool: Vec<Tileset>,
    timer: Timer,
    show_clues: usize,
}

#[derive(Event, Debug)]
struct AddRow {
    row: PuzzleRow,
}

#[derive(Event, Debug)]
struct AddClue {
    clue: Handle<DynPuzzleClue>,
}

#[derive(Event, Debug, Reflect, Clone)]
struct UpdateCellIndex {
    index: CellLocIndex,
    op: UpdateCellIndexOperation,
    explanation: Option<ClueExplanation>,
}

impl UpdateCellIndex {
    fn with_explanation(mut self, explanation: impl Into<ClueExplanation>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }
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

fn spawn_row(
    mut new_row_writer: EventWriter<AddRow>,
    mut new_clue_writer: EventWriter<AddClue>,
    time: Res<Time>,
    mut config: ResMut<PuzzleSpawn>,
    puzzle: Single<&Puzzle>,
    mut rng: ResMut<SeededRng>,
    mut update_cell_writer: EventWriter<UpdateCellIndex>,
    mut clue_assets: ResMut<Assets<DynPuzzleClue>>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    static LENGTH_SAMPLE: &[usize] = &[4, 5, 5, 5, 5, 6, 6, 7];
    config.timer.tick(time.delta());
    if config.timer.finished() {
        if puzzle.rows.len() < 5 {
            // let len = LENGTH_SAMPLE.choose(&mut rng.0).cloned().unwrap();
            let len = 5;
            let tileset = config.tileset_pool.pop().unwrap();
            let image = asset_server.load(tileset.asset_path);
            let layout = TextureAtlasLayout::from_grid(
                UVec2::new(tileset.tile_size, tileset.tile_size),
                tileset.columns,
                tileset.rows,
                None,
                None,
            );
            let atlas_len = layout.len();
            let layout_handle = texture_atlas_layouts.add(layout);
            let row = PuzzleRow::new_shuffled(
                &mut rng.0,
                len,
                image.clone(),
                layout_handle.clone(),
                atlas_len,
                tileset.shuffle,
            );
            new_row_writer.send(AddRow { row });
        } else if config.show_clues > 0 {
            config.show_clues -= 1;
            if config.show_clues == 0 {
                let row_nr = rng.0.random_range(0..puzzle.rows.len());
                let cell_nr = rng.0.random_range(0..puzzle.max_column) as isize;
                let loc = CellLoc { row_nr, cell_nr };
                let index = puzzle.cell_answer_index(loc);
                update_cell_writer.send(UpdateCellIndex {
                    index: CellLocIndex { loc, index },
                    op: UpdateCellIndexOperation::Solo,
                    explanation: None,
                });
            }
            // let (cluebox, cluebox_fit) = q_cluebox.single();
            let Some(clue): Option<Handle<DynPuzzleClue>> = (try {
                match rng.0.random_range(0..3) {
                    0 => clue_assets.add(SameColumnClue::new_random(&mut rng.0, &puzzle)?),
                    _ => clue_assets.add(AdjacentColumnClue::new_random(&mut rng.0, &puzzle)?),
                    2 => clue_assets.add(BetweenColumnsClue::new_random(&mut rng.0, &puzzle)?),
                    _ => unreachable!(),
                }
            }) else {
                return;
            };
            new_clue_writer.send(AddClue { clue });
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
    let lightness_dist = rand::distr::Uniform::new(0.2, 0.4).unwrap();
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
    hues.shuffle(rng);
    // info!("shuffled? hues={hues:?}");
    hues.into_iter()
        .take(n_colors)
        .map(|hue| Color::hsl(hue, saturation, lightness))
        .collect()
}

#[derive(Debug, Clone, Reflect)]
struct Tileset {
    asset_path: &'static str,
    shuffle: bool,
    tile_size: u32,
    columns: u32,
    rows: u32,
}

static TILESETS: [Tileset; 6] = [
    Tileset {
        asset_path: "foods.png",
        shuffle: true,
        tile_size: 200,
        columns: 10,
        rows: 1,
    },
    Tileset {
        asset_path: "natures.png",
        shuffle: true,
        tile_size: 200,
        columns: 10,
        rows: 1,
    },
    Tileset {
        asset_path: "tiles.png",
        shuffle: true,
        tile_size: 200,
        columns: 6,
        rows: 1,
    },
    Tileset {
        asset_path: "weapons.png",
        shuffle: true,
        tile_size: 200,
        columns: 7,
        rows: 1,
    },
    Tileset {
        asset_path: "armor.png",
        shuffle: true,
        tile_size: 200,
        columns: 7,
        rows: 1,
    },
    Tileset {
        asset_path: "letters.png",
        shuffle: false,
        tile_size: 200,
        columns: 6,
        rows: 1,
    },
];

fn add_row(
    mut commands: Commands,
    mut reader: EventReader<AddRow>,
    mut puzzle: Single<&mut Puzzle>,
    matrix_entity: Single<(Entity, &FitWithin), With<DisplayMatrix>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
) {
    let (matrix, matrix_fit) = *matrix_entity;
    let mut spawned = false;
    for ev in reader.read() {
        let row_nr = puzzle.rows.len();
        puzzle.add_row(ev.row.clone());
        let puzzle_row = &puzzle.rows[row_nr];

        commands.entity(matrix).with_children(|matrix_spawner| {
            matrix_spawner
                .spawn((
                    FitWithinBundle::new(),
                    // RandomColorSprite::new(),
                    DisplayRow { row_nr },
                    FitTransformAnimationBundle::new(matrix),
                ))
                .with_children(|row_spawner| {
                    for cell_nr in 0..puzzle_row.len() as isize {
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
                                FitWithinBackground::new_colored(6, DEFAULT_CELL_BORDER_COLOR),
                                // RandomColorSprite::new(),
                                DisplayCell { loc },
                            ))
                            .with_children(|cell_spawner| {
                                let button_size = Vec2::new(32., 32.);
                                for index in 0..puzzle_row.len() {
                                    let mut sprite = puzzle_row.display_sprite(index);
                                    sprite.custom_size = Some(button_size - Vec2::new(5., 5.));
                                    sprite.color = Color::hsla(0., 0., 1., 1.);
                                    cell_spawner
                                        .spawn((
                                            Sprite::from_color(
                                                puzzle_row.display_color(index),
                                                button_size,
                                            ),
                                            FitWithinBundle::new(),
                                            DisplayCellButton {
                                                index: CellLocIndex { loc, index },
                                            },
                                            HoverAnimationBundle::new(cell_player),
                                        ))
                                        .with_child((
                                            sprite,
                                            Transform::from_xyz(0., 0., 1.),
                                            NO_PICK,
                                            DisplayCellButton {
                                                index: CellLocIndex { loc, index },
                                            },
                                            HoverAnimationBundle::new(cell_player),
                                            // AssignRandomColor,
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

fn add_clue(
    mut commands: Commands,
    mut reader: EventReader<AddClue>,
    mut q_puzzle: Single<(&Puzzle, &mut PuzzleClues)>,
    q_cluebox: Single<(Entity, &FitWithin), (With<DisplayCluebox>, With<AnimationPlayer>)>,
) {
    let (_puzzle, ref mut puzzle_clues) = *q_puzzle;
    let (cluebox, cluebox_fit) = *q_cluebox;
    let mut updated = false;
    for AddClue { clue } in reader.read() {
        puzzle_clues.clues.push(clue.clone());
        commands.entity(cluebox).with_child((
            PuzzleClueComponent(clue.clone_weak()),
            FitWithinBundle::new(),
            DisplayClue,
            ExplanationBounceAnimationBundle::new(cluebox),
        ));
        updated = true;
    }
    if updated {
        cluebox_fit.refresh_rect(&mut commands, cluebox);
    }
}

fn fit_inside_window(
    q_camera: Query<(Entity, &Camera)>,
    q_fit_root: Query<(Entity, &FitWithin), Without<Parent>>,
    mut commands: Commands,
) {
    let (_camera_entity, camera) = q_camera.single();
    let Some(logical_viewport) = camera.logical_viewport_rect() else {
        return;
    };
    let window_size = logical_viewport.inflate(-10.);
    // info!("ensuring window fit of window({:?}) {:?} {:?}", window_size, camera_entity, camera);
    for (entity, fit_within) in &q_fit_root {
        fit_within.set_rect(&mut commands, entity, window_size);
    }
}

fn fit_inside_puzzle(
    ev: Trigger<OnInsert, (FitWithin, DisplayPuzzle)>,
    q_about_target: Query<
        (&FitWithin, &Children),
        (
            With<DisplayPuzzle>,
            Without<DisplayMatrix>,
            Without<DisplayCluebox>,
        ),
    >,
    q_matrix: Query<(Entity, &FitWithin, &DisplayMatrix)>,
    q_clues: Query<(Entity, &FitWithin, &DisplayCluebox)>,
    mut commands: Commands,
) {
    // info!("testing matrix fit of {:?}", ev.entity());
    let Ok((within, children)) = q_about_target.get(ev.entity()) else {
        return;
    };
    let Some(matrix) = children.iter().filter_map(|e| q_matrix.get(*e).ok()).next() else {
        return;
    };
    let Some(clues) = children.iter().filter_map(|e| q_clues.get(*e).ok()).next() else {
        return;
    };
    let fit = within.rect;
    let cluebox_height = fit.height() / 4.;
    let cluebox_y = fit.max.y - cluebox_height;
    let matrix_rect = Rect::new(fit.min.x, fit.min.y, fit.max.x, cluebox_y);
    let cluebox_rect = Rect::new(fit.min.x, cluebox_y, fit.max.x, fit.max.y);
    matrix.1.set_rect(&mut commands, matrix.0, matrix_rect);
    clues.1.set_rect(&mut commands, clues.0, cluebox_rect);
}

fn fit_inside_clues(
    ev: Trigger<OnInsert, (FitWithin, DisplayCluebox)>,
    q_about_target: Query<(&FitWithin, &Children), (With<DisplayCluebox>, Without<DisplayClue>)>,
    q_children: Query<(Entity, &FitWithin, &DisplayClue)>,
    mut commands: Commands,
) {
    let Ok((within, children)) = q_about_target.get(ev.entity()) else {
        return;
    };
    let children = children
        .iter()
        .filter_map(|e| q_children.get(*e).ok())
        .collect::<Vec<_>>();
    let fit = within.rect;
    let fit_width = fit.width();
    let clue_width = fit_width / children.len() as f32;
    // let clue_width = 45.;
    let mut current_x = fit.min.x;
    for (entity, fit_within, _) in children {
        let new_x = current_x + clue_width;
        let clue_rect = Rect::new(current_x, fit.min.y, new_x, fit.max.y);
        fit_within.set_rect(&mut commands, entity, clue_rect);
        current_x = new_x;
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
        let row_rect = Rect::new(fit.min.x, current_y, fit.max.x, new_y).inflate(-5.);
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
        let cell_rect = Rect::new(current_x, fit.min.y, new_x, fit.max.y).inflate(-5.);
        fit_within.set_rect(&mut commands, entity, cell_rect);
        current_x = new_x + cell_spacing;
    }
}

fn fit_inside_cell(
    ev: Trigger<OnInsert, (FitWithin, DisplayCell)>,
    q_about_target: Query<(&FitWithin, &Children, &DisplayCell), Without<DisplayCellButton>>,
    q_children: Query<(Entity, &FitWithin, &DisplayCellButton)>,
    q_puzzle: Single<&Puzzle>,
    mut commands: Commands,
) {
    // info!("testing matrix cell fit of {:?}", ev.entity());
    let Ok((within, children, display)) = q_about_target.get(ev.entity()) else {
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
    let sel = q_puzzle.cell_selection(display.loc);
    let sel_solo = sel.is_any_solo();
    let fit = within.rect;
    let fit_width = fit.width();
    let button_width = fit_width / children.len() as f32;
    let mut current_x = fit.min.x;
    for (entity, fit_within, button) in children {
        let new_x = current_x + button_width;
        // TODO: update the parent rect to lay this out
        let button_rect = if sel_solo == Some(button.index.index) {
            Rect::from_center_size(Vec2::default(), Vec2::new(50., 50.))
        } else {
            Rect::new(current_x, fit.min.y, new_x, fit.max.y)
        };
        fit_within.set_rect(&mut commands, entity, button_rect);
        current_x = new_x;
    }
}

fn fit_to_transform(
    ev: Trigger<OnInsert, FitWithin>,
    mut q_fit: Query<(Entity, &FitWithin, &Parent, &mut Transform)>,
    q_just_fit: Query<&FitWithin>,
    mut q_animation: Query<(&AnimationTarget, &mut FitTransformEdge)>,
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
        let mut translation = transform.translation;
        translation.x = translate.x;
        translation.y = translate.y;

        let mut clip = AnimationClip::default();
        clip.add_curve_to_target(
            target.id,
            AnimatableCurve::new(
                animated_field!(Transform::translation),
                EasingCurve::new(transform.translation, translation, EaseFunction::CubicOut)
                    .reparametrize_linear(interval(0., 0.5).unwrap())
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

fn fit_background_sprite(
    ev: Trigger<OnInsert, FitWithin>,
    mut q_fit: Query<(&FitWithin, &mut Sprite), With<FitWithinBackground>>,
) {
    let Ok((fit, mut sprite)) = q_fit.get_mut(ev.entity()) else {
        return;
    };
    sprite.custom_size = Some(fit.rect.size());
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

fn show_clues(
    q_clues: Single<&PuzzleClues>,
    q_puzzle: Single<&Puzzle>,
    clues: Res<Assets<DynPuzzleClue>>,
    mut commands: Commands,
    mut clue_state: ResMut<NextState<ClueExplanationState>>,
) {
    let puzzle = *q_puzzle;
    let mut to_enact = None;
    for clue_handle in q_clues.clues.iter() {
        let Some(clue) = clues.get(clue_handle.id()) else {
            continue;
        };
        let next = clue.advance_puzzle(puzzle);
        info!("next from {clue:?} => {next:?}");
        if let Some(next) = next {
            to_enact = Some((clue_handle, next));
            break;
        }
    }
    if let Some((clue, update)) = to_enact {
        let clue = clue.clone();
        commands.spawn(ExplainClueComponent { clue, update });
        clue_state.set(ClueExplanationState::Shown);
        // writer.send(ev);
    }
}

fn clue_explanation_clicked(
    mut ev: Trigger<Pointer<Up>>,
    q_explanation: Query<(Entity, &ExplainClueComponent), With<FitHover>>,
    mut clue_state: ResMut<NextState<ClueExplanationState>>,
    // mut commands: Commands,
) {
    info!("clicked in ?");
    let Ok((explanation, ExplainClueComponent { update, .. })) = q_explanation.get_single() else {
        return;
    };
    info!("clicked next {update:#?}");
    clue_state.set(ClueExplanationState::NotShown);
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
            writer.send(UpdateCellIndex {
                index,
                op,
                explanation: None,
            });
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
    let mut to_update = HashSet::new();
    for &UpdateCellIndex { index, op, .. } in reader.read() {
        let puzzle_cell = puzzle.cell_selection_mut(index.loc);
        if puzzle_cell.apply(index.index, op) > 0 {
            to_update.insert(index.loc);
        }
    }
    puzzle.run_inference(&mut to_update);
    for loc in to_update {
        writer.send(UpdateCellDisplay { loc });
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

const DEFAULT_BORDER_COLOR: Color = Color::hsla(33., 1., 0.32, 1.);
const DEFAULT_CELL_BORDER_COLOR: Color = Color::hsla(33., 1., 0.26, 1.);
// const DEFAULT_CELL_BORDER_COLOR: Color = Color::hsla(0., 0., 0.8, 1.);
const INVALID_CELL_BORDER_COLOR: Color = Color::hsla(0., 1., 0.5, 1.);

fn cell_update_display(
    puzzle: Single<&Puzzle>,
    mut reader: EventReader<UpdateCellDisplay>,
    mut q_bg: Query<(&DisplayCell, &mut Sprite), Without<DisplayCellButton>>,
    mut q_cell: Query<
        (
            &DisplayCellButton,
            &mut Sprite,
            &mut AnimationTarget,
            &mut HoverAlphaEdge,
        ),
        Without<DisplayCell>,
    >,
    mut q_reader: Query<(&mut AnimationPlayer, &AnimationGraphHandle)>,
    mut animation_clips: ResMut<Assets<AnimationClip>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
) {
    let mut bg_map = HashMap::new();
    for (cell, sprite) in &mut q_bg {
        bg_map.insert(cell.loc, sprite);
    }
    let mut entity_map = HashMap::<_, Vec<_>>::new();
    for (&DisplayCellButton { index }, sprite, target, hover_edge) in &mut q_cell {
        entity_map
            .entry(index.loc)
            .or_default()
            .push((index, sprite, target, hover_edge));
    }
    for &UpdateCellDisplay { loc } in reader.read() {
        let sel = puzzle.cell_selection(loc);
        let Some(buttons) = entity_map.get_mut(&loc) else {
            unreachable!()
        };
        // info!("updating cell={cell:?}");
        buttons.sort_by_key(|t| t.0);
        let sel_solo = sel.is_any_solo();

        if let Some(sprite) = bg_map.get_mut(&loc) {
            let color = if !sel.is_enabled(puzzle.cell_answer_index(loc)) {
                INVALID_CELL_BORDER_COLOR
            } else {
                DEFAULT_CELL_BORDER_COLOR
            };
            sprite.color = color;
        }

        for (index, sprite, target, hover_edge) in buttons.iter_mut() {
            let Ok((mut player, graph_handle)) = q_reader.get_mut(target.player) else {
                continue;
            };
            let Some(graph) = animation_graphs.get_mut(graph_handle.id()) else {
                continue;
            };
            let alpha = if sel.is_enabled(index.index) {
                1.
            } else if sel_solo.is_some() {
                0.03
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

#[derive(Resource, Reflect)]
#[reflect(Resource)]
struct UIBorders {
    texture: Handle<Image>,
    atlas_layout: Handle<TextureAtlasLayout>,
    slicer: TextureSlicer,
}

impl UIBorders {
    fn make_sprite(&self, index: usize, color: Color) -> Sprite {
        let mut sprite = Sprite::from_atlas_image(self.texture.clone(), TextureAtlas {
            index,
            layout: self.atlas_layout.clone(),
        });
        sprite.color = color;
        sprite.image_mode = SpriteImageMode::Sliced(self.slicer.clone());
        sprite
    }
}

fn make_fit_background_sprite(
    ev: Trigger<OnInsert, FitWithinBackground>,
    borders: Res<UIBorders>,
    mut q_target: Query<(&FitWithinBackground, &mut Transform)>,
    mut commands: Commands,
) {
    let Ok((background, mut transform)) = q_target.get_mut(ev.entity()) else {
        return;
    };
    transform.translation.z -= 5.;
    // info!("transform: {transform:?}");
    commands.entity(ev.entity()).insert((
        borders.make_sprite(background.index, background.color),
        NO_PICK,
    ));
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut rng: ResMut<SeededRng>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
) {
    commands.spawn(Camera2d);
    commands.spawn((Puzzle::default(), PuzzleClues::default()));

    let mut tileset_pool = TILESETS.iter().cloned().collect::<Vec<_>>();
    tileset_pool.shuffle(&mut rng.0);
    commands.insert_resource(PuzzleSpawn {
        timer: Timer::new(Duration::from_secs_f32(0.05), TimerMode::Repeating),
        show_clues: 10,
        tileset_pool,
    });

    commands.insert_resource({
        let texture = asset_server.load("fantasy_ui_border_sheet.png");
        let atlas_layout =
            TextureAtlasLayout::from_grid(UVec2::new(50, 50), 6, 6, Some(UVec2::splat(2)), None);
        let atlas_layout = texture_atlases.add(atlas_layout);
        let slicer = TextureSlicer {
            border: BorderRect::square(24.0),
            center_scale_mode: SliceScaleMode::Stretch,
            sides_scale_mode: SliceScaleMode::Stretch,
            max_corner_scale: 1.0,
        };
        UIBorders {
            texture,
            atlas_layout,
            slicer,
        }
    });

    commands
        .spawn((DisplayPuzzle, FitWithinBundle::new()))
        .with_children(|puzzle| {
            puzzle.spawn((
                DisplayMatrix,
                FitWithinBundle::new(),
                FitWithinBackground::new_colored(19, DEFAULT_BORDER_COLOR),
                AnimationPlayer::default(),
                AnimationGraphHandle(animation_graphs.add(AnimationGraph::new())),
            ));
            puzzle.spawn((
                DisplayCluebox,
                FitWithinBundle::new(),
                FitWithinBackground::new_colored(24, DEFAULT_BORDER_COLOR),
                AnimationPlayer::default(),
                AnimationGraphHandle(animation_graphs.add(AnimationGraph::new())),
            ));
        });
}
