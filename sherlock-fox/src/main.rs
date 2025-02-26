// © 2025 <_@habnab.it>
//
// SPDX-License-Identifier: EUPL-1.2

#![feature(try_blocks, cmp_minmax)]

mod animation;
mod clues;
mod fit;
mod puzzle;
mod undo;

use std::{any::TypeId, time::Duration};

use animation::{AnimatorPlugin, SavedAnimationNode};
use bevy::{
    animation::{
        animated_field, AnimationEntityMut, AnimationEvaluationError, AnimationTarget,
        AnimationTargetId, RepeatAnimation,
    },
    color::palettes::css,
    prelude::*,
    utils::hashbrown::{HashMap, HashSet},
    window::PrimaryWindow,
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use clues::{
    AdjacentColumnClue, BetweenColumnsClue, ClueExplanation, ClueExplanationResolvedChunk,
    DynPuzzleClue, PuzzleClues, SameColumnClue,
};
use fit::{
    FitClicked, FitClickedEvent, FitHover, FitManip, FitMouse, FitTransformAnimationBundle,
    FitTransformEdge, FitWithin, FitWithinBackground, FitWithinBundle,
};
use petgraph::graph::NodeIndex;
use puzzle::{
    CellLoc, CellLocIndex, Puzzle, PuzzleCellDisplay, PuzzleCellSelection, PuzzleRow,
    UpdateCellIndexOperation,
};
use rand::{distr::Distribution, seq::SliceRandom, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use undo::{Action, PushNewAction, UndoTree, UndoTreeLocation};
use uuid::Uuid;

const NO_PICK: PickingBehavior = PickingBehavior {
    should_block_lower: false,
    is_hoverable: false,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(fit::FitPlugin)
        .add_plugins(fit::FitMouseInteractionPlugin::<DisplayTopButton>::default())
        .add_plugins(undo::UndoPlugin)
        .init_resource::<Assets<DynPuzzleClue>>()
        .init_resource::<SeededRng>()
        .init_state::<ClueExplanationState>()
        .add_plugins(WorldInspectorPlugin::new())
        .add_event::<AddClue>()
        .add_event::<AddRow>()
        .add_event::<PushNewAction>()
        .add_event::<UpdateCellDisplay>()
        .add_event::<UpdateCellIndex>()
        .register_asset_reflect::<DynPuzzleClue>()
        .register_type::<Action>()
        .register_type::<AssignRandomColor>()
        .register_type::<CellLoc>()
        .register_type::<CellLocIndex>()
        .register_type::<DisplayButtonbox>()
        .register_type::<DisplayCell>()
        .register_type::<DisplayCellButton>()
        .register_type::<DisplayMatrix>()
        .register_type::<DisplayRow>()
        .register_type::<DisplayTopButton>()
        .register_type::<DragTarget>()
        .register_type::<DragUI>()
        .register_type::<DragUITarget>()
        .register_type::<DynPuzzleClue>()
        .register_type::<ExplainClueComponent>()
        .register_type::<ExplanationBounceEdge>()
        .register_type::<ExplanationHilight>()
        .register_type::<FitHover>()
        .register_type::<FitTransformEdge>()
        .register_type::<FitWithin>()
        .register_type::<FitWithinBackground>()
        .register_type::<HoverAlphaEdge>()
        .register_type::<HoverScaleEdge>()
        .register_type::<PushNewAction>()
        .register_type::<Puzzle>()
        .register_type::<PuzzleCellDisplay>()
        .register_type::<PuzzleCellSelection>()
        .register_type::<PuzzleClueComponent>()
        .register_type::<PuzzleClues>()
        .register_type::<PuzzleRow>()
        .register_type::<PuzzleSpawn>()
        .register_type::<SameColumnClue>()
        .register_type::<SeededRng>()
        .register_type::<UndoTree>()
        .register_type::<UndoTreeLocation>()
        .register_type::<UpdateCellIndexOperation>()
        .add_observer(cell_clicked_down)
        .add_observer(cell_continue_drag)
        .add_observer(cell_release_drag)
        .add_observer(clue_explanation_clicked)
        .add_observer(interact_cell_generic::<OnAdd>(1.25))
        .add_observer(interact_cell_generic::<OnRemove>(1.0))
        .add_observer(interact_drag_ui_move)
        .add_observer(remove_clue_highlight)
        .add_observer(show_clue_highlight)
        .add_observer(show_dyn_clue)
        .add_observer(spawn_top_buttons)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                assign_random_color,
                show_clues,
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

impl SavedAnimationNode for ExplanationBounceEdge {
    type AnimatedFrom = Transform;

    fn node_mut(&mut self) -> &mut Option<NodeIndex> {
        &mut self.0
    }
}

fn show_clue_highlight(
    ev: Trigger<OnInsert, ExplanationHilight>,
    q_can_animate: Query<&AnimationTarget, With<ExplanationBounceEdge>>,
    mut commands: Commands,
) {
    let Ok(_) = q_can_animate.get(ev.entity()) else {
        return;
    };
    let scale = Vec3::new(1.25, 1.25, 1.);
    AnimatorPlugin::<ExplanationBounceEdge>::start_animation_system(
        &mut commands,
        ev.entity(),
        RepeatAnimation::Forever,
        move |transform, target| {
            let mut clip = AnimationClip::default();
            clip.add_curve_to_target(
                target,
                AnimatableCurve::new(
                    animated_field!(Transform::scale),
                    EasingCurve::new(transform.scale, scale, EaseFunction::SineInOut)
                        .reparametrize_linear(interval(0., 0.5).unwrap())
                        .unwrap()
                        .ping_pong()
                        .unwrap(),
                ),
            );
            clip
        },
    );
}

fn remove_clue_highlight(
    ev: Trigger<OnRemove, ExplanationHilight>,
    q_can_animate: Query<&AnimationTarget, With<ExplanationBounceEdge>>,
    mut commands: Commands,
) {
    let Ok(_) = q_can_animate.get(ev.entity()) else {
        return;
    };
    let scale = Vec3::new(1., 1., 1.);
    AnimatorPlugin::<ExplanationBounceEdge>::start_animation_system(
        &mut commands,
        ev.entity(),
        RepeatAnimation::Never,
        move |transform, target| {
            let mut clip = AnimationClip::default();
            clip.add_curve_to_target(
                target,
                AnimatableCurve::new(
                    animated_field!(Transform::scale),
                    EasingCurve::new(transform.scale, scale, EaseFunction::SineOut)
                        .reparametrize_linear(interval(0., 0.25).unwrap())
                        .unwrap(),
                ),
            );
            clip
        },
    );
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
struct ExplanationBounceAnimationBundle {
    target: AnimationTarget,
    scale_tracker: ExplanationBounceEdge,
    translation_tracker: fit::FitTransformEdge,
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
struct DisplayButtonbox;

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

#[derive(Reflect, Debug, Component, Clone)]
struct DisplayTopButton(TopButtonAction);

#[derive(Reflect, Debug, Clone, Copy)]
enum TopButtonAction {
    Undo,
    Redo,
    Clue,
}

#[derive(Reflect, Debug, Component, Clone, Default)]
struct HoverScaleEdge(Option<NodeIndex>);

#[derive(Reflect, Debug, Component, Clone, Default)]
struct HoverAlphaEdge(Option<NodeIndex>);

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

fn spawn_top_buttons(ev: Trigger<OnAdd, DisplayButtonbox>, mut commands: Commands) {
    commands.entity(ev.entity()).with_children(|parent| {
        use TopButtonAction as B;
        for action in [B::Undo, B::Redo, B::Clue] {
            parent
                .spawn((
                    DisplayTopButton(action),
                    FitWithinBundle::new(),
                    FitWithinBackground::new(14)
                        .colored(DEFAULT_BUTTON_BORDER_COLOR)
                        .with_interaction(true),
                ))
                .with_child(Text2d::new(format!("{:?}", action)));
        }
    });
}

fn spawn_row(
    mut commands: Commands,
    mut new_row_tx: EventWriter<AddRow>,
    mut new_clue_tx: EventWriter<AddClue>,
    time: Res<Time>,
    mut config: ResMut<PuzzleSpawn>,
    puzzle: Single<&Puzzle>,
    mut rng: ResMut<SeededRng>,
    mut update_cell_tx: EventWriter<UpdateCellIndex>,
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
            new_row_tx.send(AddRow { row });
        } else if config.show_clues > 0 {
            config.show_clues -= 1;
            if config.show_clues == 0 {
                let mut tree = petgraph::Graph::new();
                let root = tree.add_node((*puzzle).clone());
                commands.spawn(UndoTree { tree, root });
                commands.spawn(UndoTreeLocation { current: root });

                let row_nr = rng.0.random_range(0..puzzle.rows.len());
                let cell_nr = rng.0.random_range(0..puzzle.max_column) as isize;
                let loc = CellLoc { row_nr, cell_nr };
                let index = puzzle.cell_answer_index(loc);
                update_cell_tx.send(UpdateCellIndex {
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
            new_clue_tx.send(AddClue { clue });
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
    q_matrix: Query<(Entity, &FitWithin), With<DisplayMatrix>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
) {
    let Ok(matrix_e_fit) = q_matrix.get_single() else {
        return;
    };
    let mut spawned = false;
    for ev in reader.read() {
        let row_nr = puzzle.rows.len();
        puzzle.add_row(ev.row.clone());
        let puzzle_row = &puzzle.rows[row_nr];

        commands
            .entity(matrix_e_fit.0)
            .with_children(|matrix_spawner| {
                matrix_spawner
                    .spawn((
                        FitWithinBundle::new(),
                        // RandomColorSprite::new(),
                        DisplayRow { row_nr },
                        FitTransformAnimationBundle::new(matrix_e_fit.0),
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
                                    FitWithinBackground::new(6).colored(DEFAULT_CELL_BORDER_COLOR),
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
        matrix_e_fit.refresh_rect(&mut commands);
    }
}

fn add_clue(
    mut commands: Commands,
    mut reader: EventReader<AddClue>,
    mut q_puzzle: Single<(&Puzzle, &mut PuzzleClues)>,
    q_cluebox: Single<(Entity, &FitWithin), (With<DisplayCluebox>, With<AnimationPlayer>)>,
) {
    let (_puzzle, ref mut puzzle_clues) = *q_puzzle;
    let cluebox_e_fit = *q_cluebox;
    let mut updated = false;
    for AddClue { clue } in reader.read() {
        puzzle_clues.clues.push(clue.clone());
        commands.entity(cluebox_e_fit.0).with_child((
            PuzzleClueComponent(clue.clone_weak()),
            FitWithinBundle::new(),
            DisplayClue,
            ExplanationBounceAnimationBundle::new(cluebox_e_fit.0),
        ));
        updated = true;
    }
    if updated {
        cluebox_e_fit.refresh_rect(&mut commands);
    }
}

impl SavedAnimationNode for HoverScaleEdge {
    type AnimatedFrom = Transform;

    fn node_mut(&mut self) -> &mut Option<NodeIndex> {
        &mut self.0
    }
}

fn interact_cell_generic<T>(
    target_scale_xy: f32,
) -> impl Fn(
    Trigger<T, FitHover>,
    Query<&AnimationTarget, (With<HoverScaleEdge>, With<DisplayCellButton>)>,
    Commands,
) {
    let target_scale = Vec3::new(target_scale_xy, target_scale_xy, 1.0);
    move |ev, q_can_animate, mut commands| {
        let Ok(_) = q_can_animate.get(ev.entity()) else {
            return;
        };
        AnimatorPlugin::<HoverScaleEdge>::start_animation_system(
            &mut commands,
            ev.entity(),
            RepeatAnimation::Never,
            move |transform, target| {
                let mut clip = AnimationClip::default();
                clip.add_curve_to_target(
                    target,
                    AnimatableCurve::new(
                        animated_field!(Transform::scale),
                        EasingCurve::new(transform.scale, target_scale, EaseFunction::CubicOut)
                            .reparametrize_linear(interval(0., 0.25).unwrap())
                            .unwrap(),
                    ),
                );
                clip
            },
        );
    }
}

impl FitMouse for DisplayTopButton {
    const HOVER: Color = HOVER_BUTTON_BORDER_COLOR;
    const CLICKED: Color = CLICKED_BUTTON_BORDER_COLOR;
    const NEUTRAL: Color = DEFAULT_BUTTON_BORDER_COLOR;

    type OnClick = TopButtonAction;
    fn clicked(&self) -> Self::OnClick {
        self.0
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
    mut ev_rx: EventReader<FitClickedEvent<TopButtonAction>>,
    q_clues: Single<&PuzzleClues>,
    q_puzzle: Single<&Puzzle>,
    clues: Res<Assets<DynPuzzleClue>>,
    mut commands: Commands,
    mut clue_state: ResMut<NextState<ClueExplanationState>>,
) {
    let show_clue = {
        let mut seen = false;
        for &FitClickedEvent(action) in ev_rx.read() {
            if let TopButtonAction::Clue = action {
                seen = true;
            }
        }
        seen
    };
    if !show_clue {
        return;
    }

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
    _ev: Trigger<Pointer<Up>>,
    q_explanation: Query<(Entity, &ExplainClueComponent), With<FitClicked>>,
    mut clue_state: ResMut<NextState<ClueExplanationState>>,
) {
    // info!("clicked in ?");
    let Ok((explanation, ExplainClueComponent { update, .. })) = q_explanation.get_single() else {
        return;
    };
    info!("clicked next {update:#?}");
    clue_state.set(ClueExplanationState::NotShown);
}

fn cell_clicked_down(
    ev: Trigger<OnInsert, FitClicked>,
    q_camera: Single<&Camera>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_cell: Query<(&DisplayCellButton, &GlobalTransform, &Sprite), With<FitClicked>>,
    // q_ui: Query<Entity, With<DragUI>>,
    mut commands: Commands,
) {
    let Ok((button, &transform, sprite)) = q_cell.get(ev.entity()) else {
        return;
    };
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
        .with_children(|parent| {
            parent.spawn((
                Text2d::new("Clear"),
                Transform::from_xyz(50., 0., 1.),
                DragUITarget(UpdateCellIndexOperation::Clear),
            ));
            parent.spawn((
                Text2d::new("Set"),
                Transform::from_xyz(0., -50., 1.),
                DragUITarget(UpdateCellIndexOperation::Set),
            ));
            parent.spawn((
                Text2d::new("Toggle"),
                Transform::from_xyz(-50., 0., 1.),
                DragUITarget(UpdateCellIndexOperation::Toggle),
            ));
            parent.spawn((
                Text2d::new("Solo"),
                Transform::from_xyz(0., 50., 1.),
                DragUITarget(UpdateCellIndexOperation::Solo),
            ));
        });
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
    ev: Trigger<OnRemove, FitClicked>,
    q_orig: Query<Entity, (With<FitClicked>, With<DisplayCellButton>)>,
    mut commands: Commands,
    q_cell: Query<(Entity, &DisplayCellButton, &DragTarget)>,
    q_dragui: Query<Entity, With<DragUI>>,
    mut writer: EventWriter<UpdateCellIndex>,
) {
    let Ok(_) = q_orig.get(ev.entity()) else {
        return;
    };
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
    mut update_cell_rx: EventReader<UpdateCellIndex>,
    mut update_display_tx: EventWriter<UpdateCellDisplay>,
    mut undo_tx: EventWriter<PushNewAction>,
) {
    let mut all_to_update = HashSet::new();
    for update @ &UpdateCellIndex { index, op, .. } in update_cell_rx.read() {
        let puzzle_cell = puzzle.cell_selection_mut(index.loc);
        let update_count = puzzle_cell.apply(index.index, op);
        if update_count == 0 {
            continue;
        }
        let mut to_update = HashSet::new();
        to_update.insert(index.loc);
        let inferred_count = puzzle.run_inference(&mut to_update);
        undo_tx.send(PushNewAction {
            new_state: puzzle.clone(),
            action: Action {
                update: update.clone(),
                update_count,
                inferred_count,
            },
        });
        all_to_update.extend(to_update);
    }
    for loc in all_to_update {
        update_display_tx.send(UpdateCellDisplay { loc });
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
const DEFAULT_BUTTON_BORDER_COLOR: Color = Color::hsla(33., 1., 0.32, 1.);
const HOVER_BUTTON_BORDER_COLOR: Color = Color::hsla(33., 1., 0.6, 1.);
const CLICKED_BUTTON_BORDER_COLOR: Color = Color::hsla(0., 0., 0.8, 1.);
const DEFAULT_CELL_BORDER_COLOR: Color = Color::hsla(33., 1., 0.26, 1.);
// const DEFAULT_CELL_BORDER_COLOR: Color = Color::hsla(0., 0., 0.8, 1.);
const INVALID_CELL_BORDER_COLOR: Color = Color::hsla(0., 1., 0.5, 1.);

impl animation::SavedAnimationNode for HoverAlphaEdge {
    type AnimatedFrom = Sprite;

    fn node_mut(&mut self) -> &mut Option<NodeIndex> {
        &mut self.0
    }
}

fn cell_update_display(
    puzzle: Single<&Puzzle>,
    mut reader: EventReader<UpdateCellDisplay>,
    mut q_bg: Query<(&DisplayCell, &mut Sprite), Without<DisplayCellButton>>,
    q_cell: Query<(Entity, &DisplayCellButton), Without<DisplayCell>>,
    mut commands: Commands,
) {
    let mut bg_map = HashMap::new();
    for (cell, sprite) in &mut q_bg {
        bg_map.insert(cell.loc, sprite);
    }
    let mut entity_map = HashMap::<_, Vec<_>>::new();
    for (entity, &DisplayCellButton { index }) in &q_cell {
        entity_map
            .entry(index.loc)
            .or_default()
            .push((entity, index));
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

        for (entity, index) in buttons.iter() {
            let alpha = if sel.is_enabled(index.index) {
                1.
            } else if sel_solo.is_some() {
                0.03
            } else {
                0.2
            };

            AnimatorPlugin::<HoverAlphaEdge>::start_animation_system(
                &mut commands,
                *entity,
                RepeatAnimation::Never,
                move |sprite, target| {
                    let mut clip = AnimationClip::default();
                    clip.add_curve_to_target(
                        target,
                        AnimatableCurve::new(
                            ButtonOpacityAnimation,
                            EasingCurve::new(sprite.color.alpha(), alpha, EaseFunction::CubicOut)
                                .reparametrize_linear(interval(0., 0.25).unwrap())
                                .unwrap(),
                        ),
                    );
                    clip
                },
            );
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
        .with_children(|parent| {
            parent.spawn((
                DisplayMatrix,
                FitWithinBundle::new(),
                FitWithinBackground::new(19).colored(DEFAULT_BORDER_COLOR),
                AnimationPlayer::default(),
                AnimationGraphHandle(animation_graphs.add(AnimationGraph::new())),
            ));
            parent.spawn((
                DisplayCluebox,
                FitWithinBundle::new(),
                FitWithinBackground::new(24).colored(DEFAULT_BORDER_COLOR),
                AnimationPlayer::default(),
                AnimationGraphHandle(animation_graphs.add(AnimationGraph::new())),
            ));
            parent.spawn((
                DisplayButtonbox,
                FitWithinBundle::new(),
                FitWithinBackground::new(20).colored(DEFAULT_BORDER_COLOR),
                AnimationPlayer::default(),
                AnimationGraphHandle(animation_graphs.add(AnimationGraph::new())),
            ));
        });
}
