// © 2025 <_@habnab.it>
//
// SPDX-License-Identifier: EUPL-1.2

use std::marker::PhantomData;

use bevy::{
    animation::{animated_field, AnimationTarget, AnimationTargetId},
    input::common_conditions::input_just_released,
    prelude::*,
    window::PrimaryWindow,
};
use petgraph::graph::NodeIndex;
use uuid::Uuid;

use crate::{
    animation::{AnimatorPlugin, SavedAnimationNode},
    puzzle::Puzzle,
    DisplayButtonbox, DisplayCell, DisplayCellButton, DisplayClue, DisplayCluebox, DisplayMatrix,
    DisplayPuzzle, DisplayRow, DisplayTopButton, UIBorders,
};

#[derive(Reflect, Debug, Clone, Component, Default)]
pub struct FitWithin {
    rect: Rect,
}

impl FitWithin {
    pub fn new(rect: Rect) -> Self {
        FitWithin { rect }
    }
}

pub struct FitEntity<'e> {
    entity: Entity,
    fit: &'e FitWithin,
}

impl<'e> FitEntity<'e> {
    pub fn new(entity: Entity, fit: &'e FitWithin) -> Self {
        FitEntity { entity, fit }
    }
}

pub trait FitManip {
    fn entity(&self) -> Entity;
    fn fit(&self) -> &FitWithin;

    fn refresh_rect(&self, commands: &mut Commands) {
        commands.entity(self.entity()).insert(FitWithin {
            rect: self.fit().rect,
        });
    }

    fn set_rect(&self, commands: &mut Commands, new_rect: Rect) {
        if self.fit().rect != new_rect {
            commands
                .entity(self.entity())
                .insert(FitWithin { rect: new_rect });
        }
    }
}

impl<'e> FitManip for FitEntity<'e> {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn fit(&self) -> &FitWithin {
        self.fit
    }
}

impl<'e> FitManip for (Entity, &'e FitWithin) {
    fn entity(&self) -> Entity {
        self.0
    }

    fn fit(&self) -> &FitWithin {
        self.1
    }
}

#[derive(Reflect, Debug, Component)]
pub struct FitHover;

#[derive(Reflect, Debug, Component)]
pub struct FitClicked;

#[derive(Bundle)]
pub struct FitWithinBundle {
    fit: FitWithin,
    transform: Transform,
    visibility: InheritedVisibility,
}

impl FitWithinBundle {
    pub fn new() -> Self {
        FitWithinBundle {
            fit: FitWithin::default(),
            transform: Transform::default(),
            visibility: InheritedVisibility::VISIBLE,
        }
    }
}

#[derive(Reflect, Debug, Component, Default)]
pub struct FitWithinBackground {
    index: usize,
    color: Color,
    interactable: bool,
}

impl FitWithinBackground {
    pub fn new(index: usize) -> Self {
        FitWithinBackground {
            index,
            color: Color::hsla(0., 0., 1., 1.),
            interactable: false,
        }
    }

    pub fn colored(self, color: Color) -> Self {
        FitWithinBackground { color, ..self }
    }

    pub fn with_interaction(self, interactable: bool) -> Self {
        FitWithinBackground {
            interactable,
            ..self
        }
    }
}

#[derive(Reflect, Debug, Component, Clone, Default)]
pub struct FitTransformEdge(Option<NodeIndex>);

#[derive(Bundle)]
pub struct FitTransformAnimationBundle {
    target: AnimationTarget,
    translation_tracker: FitTransformEdge,
}

impl FitTransformAnimationBundle {
    pub fn new(player: Entity) -> Self {
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
    for e_fit in &q_fit_root {
        e_fit.set_rect(&mut commands, window_size);
    }
}

macro_rules! get_child {
    ($ret:pat_param = $q:expr, $children:expr) => {
        let q = &$q;
        let Some($ret) = $children
            .iter()
            .filter_map(|e| {
                q.get(*e)
                    .ok()
                    .map(|(entity, fit)| FitEntity { entity, fit })
            })
            .next()
        else {
            return;
        };
    };
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
    q_matrix: Query<(Entity, &FitWithin), With<DisplayMatrix>>,
    q_clues: Query<(Entity, &FitWithin), With<DisplayCluebox>>,
    q_buttons: Query<(Entity, &FitWithin), With<DisplayButtonbox>>,
    mut commands: Commands,
) {
    // info!("testing matrix fit of {:?}", ev.entity());
    let Ok((within, children)) = q_about_target.get(ev.entity()) else {
        return;
    };
    get_child!(matrix = q_matrix, children);
    get_child!(clues = q_clues, children);
    get_child!(buttons = q_buttons, children);
    let fit = within.rect;
    let buttonbox_width = fit.width() / 6.;
    let buttonbox_x = fit.max.x - buttonbox_width;
    let cluebox_height = fit.height() / 4.;
    let cluebox_y = fit.max.y - cluebox_height;
    let matrix_rect = Rect::new(fit.min.x, fit.min.y, buttonbox_x, cluebox_y);
    let cluebox_rect = Rect::new(fit.min.x, cluebox_y, buttonbox_x, fit.max.y);
    let buttonbox_rect = Rect::new(buttonbox_x, fit.min.y, fit.max.x, fit.max.y);
    matrix.set_rect(&mut commands, matrix_rect);
    clues.set_rect(&mut commands, cluebox_rect);
    buttons.set_rect(&mut commands, buttonbox_rect);
}

fn fit_inside_clues(
    ev: Trigger<OnInsert, (FitWithin, DisplayCluebox)>,
    q_about_target: Query<(&FitWithin, &Children), (With<DisplayCluebox>, Without<DisplayClue>)>,
    q_children: Query<(Entity, &FitWithin), With<DisplayClue>>,
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
    for e_fit in children {
        let new_x = current_x + clue_width;
        let clue_rect = Rect::new(current_x, fit.min.y, new_x, fit.max.y);
        e_fit.set_rect(&mut commands, clue_rect);
        current_x = new_x;
    }
}

fn fit_inside_buttonbox(
    ev: Trigger<OnInsert, (FitWithin, DisplayButtonbox)>,
    q_about_target: Query<
        (&FitWithin, &Children),
        (With<DisplayButtonbox>, Without<DisplayTopButton>),
    >,
    q_children: Query<(Entity, &FitWithin), With<DisplayTopButton>>,
    mut commands: Commands,
) {
    let Ok((within, children)) = q_about_target.get(ev.entity()) else {
        return;
    };
    let children = children
        .iter()
        .filter_map(|e| q_children.get(*e).ok())
        .collect::<Vec<_>>();
    let fit = within.rect.inflate(-10.);
    // let fit_height = fit.height();
    let row_height = 50.;
    let mut current_y = fit.min.y;
    for e_fit in children {
        let new_y = current_y + row_height + 20.;
        let row_rect = Rect::new(fit.min.x, current_y, fit.max.x, new_y).inflate(-5.);
        e_fit.set_rect(&mut commands, row_rect);
        current_y = new_y;
    }
}

fn fit_inside_matrix(
    ev: Trigger<OnInsert, (FitWithin, DisplayMatrix)>,
    q_about_target: Query<(&FitWithin, &Children), (With<DisplayMatrix>, Without<DisplayRow>)>,
    q_children: Query<((Entity, &FitWithin), &DisplayRow)>,
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
        children.sort_by_key(|(_, row)| row.row_nr);
        children
    };
    let fit = within.rect;
    let fit_height = fit.height();
    let row_height = fit_height / children.len() as f32;
    let mut current_y = fit.max.y;
    for (e_fit, _) in children {
        let new_y = current_y - row_height;
        let row_rect = Rect::new(fit.min.x, current_y, fit.max.x, new_y).inflate(-5.);
        e_fit.set_rect(&mut commands, row_rect);
        current_y = new_y;
    }
}

fn fit_inside_row(
    ev: Trigger<OnInsert, (FitWithin, DisplayRow)>,
    q_about_target: Query<(&FitWithin, &Children), (With<DisplayRow>, Without<DisplayCell>)>,
    q_children: Query<((Entity, &FitWithin), &DisplayCell)>,
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
        children.sort_by_key(|(_, cell)| cell.loc);
        children
    };
    let fit = within.rect;
    let fit_width = fit.width();
    let prospective_cell_width = fit_width / children.len() as f32;
    let cell_spacing = prospective_cell_width * 0.15;
    let total_cell_spacing = cell_spacing * (children.len() - 1) as f32;
    let cell_width = (fit_width - total_cell_spacing) / children.len() as f32;
    let mut current_x = fit.min.x;
    for (e_fit, _) in children {
        let new_x = current_x + cell_width;
        let cell_rect = Rect::new(current_x, fit.min.y, new_x, fit.max.y).inflate(-5.);
        e_fit.set_rect(&mut commands, cell_rect);
        current_x = new_x + cell_spacing;
    }
}

fn fit_inside_cell(
    ev: Trigger<OnInsert, (FitWithin, DisplayCell)>,
    q_about_target: Query<(&FitWithin, &Children, &DisplayCell), Without<DisplayCellButton>>,
    q_children: Query<((Entity, &FitWithin), &DisplayCellButton)>,
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
        children.sort_by_key(|(_, button)| button.index);
        children
    };
    let sel = q_puzzle.cell_selection(display.loc);
    let sel_solo = sel.is_any_solo();
    let fit = within.rect;
    let fit_width = fit.width();
    let button_width = fit_width / children.len() as f32;
    let mut current_x = fit.min.x;
    for (e_fit, button) in children {
        let new_x = current_x + button_width;
        // TODO: update the parent rect to lay this out
        let button_rect = if sel_solo == Some(button.index.index) {
            Rect::from_center_size(Vec2::default(), Vec2::new(50., 50.))
        } else {
            Rect::new(current_x, fit.min.y, new_x, fit.max.y)
        };
        e_fit.set_rect(&mut commands, button_rect);
        current_x = new_x;
    }
}

impl SavedAnimationNode for FitTransformEdge {
    type AnimatedFrom = Transform;

    fn node_mut(&mut self) -> &mut Option<NodeIndex> {
        &mut self.0
    }
}

fn fit_to_transform(
    ev: Trigger<OnInsert, FitWithin>,
    mut q_fit: Query<(Entity, &FitWithin, &Parent, &mut Transform)>,
    q_just_fit: Query<&FitWithin>,
    q_can_animate: Query<&AnimationTarget, With<FitTransformEdge>>,
    mut commands: Commands,
) {
    let Ok((entity, fit, parent, mut transform)) = q_fit.get_mut(ev.entity()) else {
        return;
    };
    let Ok(parent_fit) = q_just_fit.get(**parent) else {
        return;
    };
    // info!("fit to transform before={fit:?}");
    // TODO: unsure why this needs to be Y-reflected
    let new_translation = Vec3::from((
        (fit.rect.center() - parent_fit.rect.center()) * Vec2::new(1., -1.),
        1.,
    ));
    if q_can_animate.get(entity).is_ok() {
        AnimatorPlugin::<FitTransformEdge>::start_animation_system(
            &mut commands,
            entity,
            move |transform, target| {
                let mut clip = AnimationClip::default();
                clip.add_curve_to_target(
                    target,
                    AnimatableCurve::new(
                        animated_field!(Transform::translation),
                        EasingCurve::new(
                            transform.translation,
                            new_translation,
                            EaseFunction::CubicOut,
                        )
                        .reparametrize_linear(interval(0., 0.5).unwrap())
                        .unwrap(),
                    ),
                );
                clip
            },
        );
    } else {
        transform.translation = new_translation;
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

fn make_fit_background_sprite(
    ev: Trigger<OnInsert, FitWithinBackground>,
    borders: Res<UIBorders>,
    mut q_target: Query<(&FitWithinBackground, &mut Transform)>,
    mut commands: Commands,
) {
    let Ok((background, mut transform)) = q_target.get_mut(ev.entity()) else {
        return;
    };
    // transform.translation.z -= 5.;
    // info!("transform: {transform:?}");
    commands.entity(ev.entity()).insert((
        borders.make_sprite(background.index, background.color),
        PickingBehavior {
            should_block_lower: background.interactable,
            is_hoverable: background.interactable,
        },
        // NO_PICK,
    ));
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

fn fit_clicked_down(
    mut ev: Trigger<Pointer<Down>>,
    q_hovered: Query<Entity, With<FitHover>>,
    mut commands: Commands,
) {
    let mut trapped = false;
    for entity in &q_hovered {
        commands.entity(entity).insert(FitClicked);
        trapped = true;
    }
    if trapped {
        ev.propagate(false);
    }
}

fn fit_clear_clicked(q_clicked: Query<Entity, With<FitClicked>>, mut commands: Commands) {
    info!("clicked up");
    for entity in &q_clicked {
        info!("clearing click on {entity:?}");
        commands.entity(entity).remove::<FitClicked>();
    }
}

pub trait FitMouse {
    const NEUTRAL: Color;
    const HOVER: Color;
    const CLICKED: Color;

    type OnClick: Send + Sync + Clone + std::fmt::Debug + 'static;
    fn clicked(&self) -> Self::OnClick;
}

#[derive(Debug)]
pub struct FitMouseInteractionPlugin<T>(PhantomData<fn() -> T>);

impl<T> Default for FitMouseInteractionPlugin<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

#[derive(Debug, Reflect, Event)]
pub struct FitClickedEvent<D>(pub D);

impl<C: FitMouse + Component> FitMouseInteractionPlugin<C> {
    fn interact_hover_in(
        ev: Trigger<OnAdd, FitHover>,
        mut q_target: Query<(&mut Sprite, Option<&FitClicked>), With<C>>,
        mouse: Res<ButtonInput<MouseButton>>,
    ) {
        let Ok((mut sprite, clicked)) = q_target.get_mut(ev.entity()) else {
            return;
        };
        sprite.color = if clicked.is_some() {
            C::CLICKED
        } else if mouse.pressed(MouseButton::Left) {
            C::NEUTRAL
        } else {
            C::HOVER
        };
    }

    fn interact_hover_out(
        ev: Trigger<OnRemove, FitHover>,
        mut q_target: Query<&mut Sprite, With<C>>,
    ) {
        let Ok(mut sprite) = q_target.get_mut(ev.entity()) else {
            return;
        };
        sprite.color = C::NEUTRAL;
    }

    fn interact_click_down(
        ev: Trigger<OnAdd, FitClicked>,
        mut q_target: Query<&mut Sprite, With<C>>,
    ) {
        let Ok(mut sprite) = q_target.get_mut(ev.entity()) else {
            return;
        };
        sprite.color = C::CLICKED;
    }

    fn interact_click_up(
        ev: Trigger<OnRemove, FitClicked>,
        mut q_target: Query<(&mut Sprite, Option<&FitHover>, &C)>,
        mut ev_tx: EventWriter<FitClickedEvent<C::OnClick>>,
    ) {
        let Ok((mut sprite, hover, data)) = q_target.get_mut(ev.entity()) else {
            return;
        };
        info!("click up, hover: {:?}", hover);
        sprite.color = if hover.is_some() {
            ev_tx.send(FitClickedEvent(data.clicked()));
            C::HOVER
        } else {
            C::NEUTRAL
        };
    }
}

impl<C: FitMouse + Component> Plugin for FitMouseInteractionPlugin<C> {
    fn build(&self, app: &mut App) {
        app.add_event::<FitClickedEvent<C::OnClick>>()
            .add_observer(Self::interact_click_down)
            .add_observer(Self::interact_click_up)
            .add_observer(Self::interact_hover_in)
            .add_observer(Self::interact_hover_out);
    }
}

pub struct FitPlugin;

impl Plugin for FitPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(fit_clicked_down)
            .add_observer(fit_background_sprite)
            .add_observer(fit_inside_buttonbox)
            .add_observer(fit_inside_cell)
            .add_observer(fit_inside_clues)
            .add_observer(fit_inside_matrix)
            .add_observer(fit_inside_puzzle)
            .add_observer(fit_inside_row)
            .add_observer(fit_to_transform)
            .add_observer(make_fit_background_sprite)
            .add_observer(mouse_out_fit)
            .add_observer(mouse_over_fit)
            .add_systems(
                Update,
                (
                    fit_clear_clicked.run_if(input_just_released(MouseButton::Left)),
                    fit_inside_window.run_if(any_with_component::<PrimaryWindow>),
                ),
            );
    }
}
