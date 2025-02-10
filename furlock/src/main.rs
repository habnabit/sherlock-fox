use bevy::prelude::*;
use fixedbitset::FixedBitSet;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, sprite_movement)
        .run();
}

#[derive(Debug, Component)]
enum GridCell {
    One(usize),
    Many(FixedBitSet),
}

impl GridCell {
    fn new(len: usize) -> Self {
        let mut bitset = FixedBitSet::with_capacity(len);
        bitset.set_range(.., true);
        GridCell::Many(bitset)
    }
}

#[derive(Component)]
struct GridRow {
    base: String,
}

#[derive(Component)]
enum Direction {
    Up,
    Down,
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    commands
        .spawn((
            Node {
                display: Display::Grid,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                grid_template_rows: RepeatedGridTrack::flex(4, 1.0),
                ..Default::default()
            },
        ))
        .with_children(|root| {
            for base_char in "ABCD".chars() {
                let base: String = base_char.into();
                root
                    .spawn((
                        GridRow { base },
                        Visibility::default(),
                        Node {
                            display: Display::Grid,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            grid_template_columns: RepeatedGridTrack::flex(4, 1.0),
                            ..Default::default()
                        },
                        // Transform::from_xyz(-200., y_at, 0.),
                    ))
                    .with_children(|row| {
                        for x in 0..4 {
                            row
                                .spawn((
                                    GridCell::new(4),
                                    Sprite::from_image(asset_server.load("blue.jpg")),
                                    // Transform::from_xyz(x as f32 * 125., 0., 0.),
                                    Node {
                                        ..Default::default()
                                    },
                                ))
                                .with_child(Text::new(format!("{base_char}{x}")))
                                .observe(cell_clicked)
                                .observe(cell_unclicked)
                                ;
                        }
                    });
                }
            });
}

fn cell_clicked(ev: Trigger<Pointer<Down>>, sprites: Query<(&GridCell, &Parent)>) {
    info!("clicked:");
    for (cell, parent) in sprites.iter() {
        info!("  {:?}", cell);
    }
}
fn cell_unclicked(ev: Trigger<Pointer<Up>>, sprites: Query<(&GridCell, &Parent)>) {
    info!("unclicked:");
}

/// The sprite is animated by changing its translation depending on the time that has passed since
/// the last frame.
fn sprite_movement(time: Res<Time>, mut sprite_position: Query<(&mut Direction, &mut Transform)>) {
    for (mut logo, mut transform) in &mut sprite_position {
        match *logo {
            Direction::Up => transform.translation.y += 150. * time.delta_secs(),
            Direction::Down => transform.translation.y -= 150. * time.delta_secs(),
        }

        if transform.translation.y > 200. {
            *logo = Direction::Down;
        } else if transform.translation.y < -200. {
            *logo = Direction::Up;
        }
    }
}
