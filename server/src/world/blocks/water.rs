use std::{
    collections::HashMap,
    ops::{Index, IndexMut},
};

use fmc::{
    blocks::{BlockFace, BlockId, BlockRotation, BlockState, Blocks},
    prelude::*,
    world::{BlockUpdate, ChangedBlockEvent},
};

pub(super) struct WaterPlugin;
impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WaterUpdateTimer(Timer::new(
            std::time::Duration::from_millis(200),
            TimerMode::Repeating,
        )))
        .add_systems(Startup, setup)
        .add_systems(Update, spread_water);
    }
}

fn setup(mut commands: Commands, blocks: Res<Blocks>) {
    let mut water = Water::default();
    water.add(
        WaterBlock {
            corners: [
                WaterLevel::Nine,
                WaterLevel::Nine,
                WaterLevel::Nine,
                WaterLevel::Nine,
            ],
            is_source: false,
        },
        vec![
            blocks.get_id("still_water_9"),
            blocks.get_id("still_water_8"),
            blocks.get_id("still_water_7"),
            blocks.get_id("still_water_6"),
            blocks.get_id("still_water_5"),
            blocks.get_id("still_water_4"),
            blocks.get_id("still_water_3"),
            blocks.get_id("still_water_2"),
            blocks.get_id("still_water_1"),
        ],
    );
    water.add(
        WaterBlock {
            corners: [
                WaterLevel::Eight,
                WaterLevel::Eight,
                WaterLevel::Nine,
                WaterLevel::Nine,
            ],
            is_source: false,
        },
        vec![
            blocks.get_id("straight_water_8"),
            blocks.get_id("straight_water_7"),
            blocks.get_id("straight_water_6"),
            blocks.get_id("straight_water_5"),
            blocks.get_id("straight_water_4"),
            blocks.get_id("straight_water_3"),
            blocks.get_id("straight_water_2"),
            blocks.get_id("straight_water_1"),
        ],
    );

    water.add(
        WaterBlock {
            corners: [
                WaterLevel::Seven,
                WaterLevel::Eight,
                WaterLevel::Nine,
                WaterLevel::Eight,
            ],
            is_source: false,
        },
        vec![
            blocks.get_id("diagonal_water_7"),
            blocks.get_id("diagonal_water_6"),
            blocks.get_id("diagonal_water_5"),
            blocks.get_id("diagonal_water_4"),
            blocks.get_id("diagonal_water_3"),
            blocks.get_id("diagonal_water_2"),
            blocks.get_id("diagonal_water_1"),
        ],
    );

    water.add(
        WaterBlock {
            corners: [
                WaterLevel::Eight,
                WaterLevel::Eight,
                WaterLevel::Nine,
                WaterLevel::Eight,
            ],
            is_source: false,
        },
        vec![
            blocks.get_id("diagonal_water_corner_up_8"),
            blocks.get_id("diagonal_water_corner_up_7"),
            blocks.get_id("diagonal_water_corner_up_6"),
            blocks.get_id("diagonal_water_corner_up_5"),
            blocks.get_id("diagonal_water_corner_up_4"),
            blocks.get_id("diagonal_water_corner_up_3"),
            blocks.get_id("diagonal_water_corner_up_2"),
            blocks.get_id("diagonal_water_corner_up_1"),
        ],
    );

    water.add(
        WaterBlock {
            corners: [
                WaterLevel::Eight,
                WaterLevel::Nine,
                WaterLevel::Nine,
                WaterLevel::Nine,
            ],
            is_source: false,
        },
        vec![
            blocks.get_id("diagonal_water_corner_down_8"),
            blocks.get_id("diagonal_water_corner_down_7"),
            blocks.get_id("diagonal_water_corner_down_6"),
            blocks.get_id("diagonal_water_corner_down_5"),
            blocks.get_id("diagonal_water_corner_down_4"),
            blocks.get_id("diagonal_water_corner_down_3"),
            blocks.get_id("diagonal_water_corner_down_2"),
            blocks.get_id("diagonal_water_corner_down_1"),
        ],
    );

    water.add(
        WaterBlock {
            corners: [
                WaterLevel::Nine,
                WaterLevel::Eight,
                WaterLevel::Nine,
                WaterLevel::Eight,
            ],
            is_source: false,
        },
        vec![
            blocks.get_id("tilted_water_8"),
            blocks.get_id("tilted_water_7"),
            blocks.get_id("tilted_water_6"),
            blocks.get_id("tilted_water_5"),
            blocks.get_id("tilted_water_4"),
            blocks.get_id("tilted_water_3"),
            blocks.get_id("tilted_water_2"),
            blocks.get_id("tilted_water_1"),
        ],
    );

    // Have to add the two source block types manually, as well as still_water_10 because it works
    // the same way as subsurface.
    water.water_to_block.insert(
        WaterBlock {
            corners: [
                WaterLevel::Nine,
                WaterLevel::Nine,
                WaterLevel::Nine,
                WaterLevel::Nine,
            ],
            is_source: true,
        },
        (blocks.get_id("surface_water"), None),
    );
    water.block_to_water.insert(
        (blocks.get_id("surface_water"), None),
        WaterBlock {
            corners: [
                WaterLevel::Nine,
                WaterLevel::Nine,
                WaterLevel::Nine,
                WaterLevel::Nine,
            ],
            is_source: true,
        },
    );

    water.water_to_block.insert(
        WaterBlock {
            corners: [
                WaterLevel::Ten,
                WaterLevel::Ten,
                WaterLevel::Ten,
                WaterLevel::Ten,
            ],
            is_source: true,
        },
        (blocks.get_id("subsurface_water"), None),
    );
    // This is one level lower to mimic surface water, since nothing can spread from level 10
    water.block_to_water.insert(
        (blocks.get_id("subsurface_water"), None),
        WaterBlock {
            corners: [
                WaterLevel::Ten,
                WaterLevel::Ten,
                WaterLevel::Ten,
                WaterLevel::Ten,
            ],
            is_source: true,
        },
    );

    water.water_to_block.insert(
        WaterBlock {
            corners: [
                WaterLevel::Ten,
                WaterLevel::Ten,
                WaterLevel::Ten,
                WaterLevel::Ten,
            ],
            is_source: false,
        },
        (blocks.get_id("still_water_10"), None),
    );
    water.block_to_water.insert(
        (blocks.get_id("still_water_10"), None),
        WaterBlock {
            corners: [
                WaterLevel::Ten,
                WaterLevel::Ten,
                WaterLevel::Ten,
                WaterLevel::Ten,
            ],
            is_source: false,
        },
    );

    // This is for removal of water
    water.water_to_block.insert(
        WaterBlock {
            corners: [
                WaterLevel::Zero,
                WaterLevel::Zero,
                WaterLevel::Zero,
                WaterLevel::Zero,
            ],
            is_source: false,
        },
        (blocks.get_id("air"), None),
    );

    commands.insert_resource(water);
}

#[derive(Resource, Default)]
struct Water {
    water_to_block: HashMap<WaterBlock, (BlockId, Option<BlockState>)>,
    block_to_water: HashMap<(BlockId, Option<BlockState>), WaterBlock>,
}

impl Water {
    #[track_caller]
    fn add(&mut self, mut water_block: WaterBlock, block_ids: Vec<BlockId>) {
        for block_id in block_ids {
            self.water_to_block
                .insert(water_block.clone(), (block_id, None));
            self.block_to_water
                .insert((block_id, None), water_block.clone());

            if water_block[Corners::Left] == water_block[Corners::Right]
                && water_block[Corners::Left] == water_block[Corners::FarRight]
                && water_block[Corners::Left] == water_block[Corners::FarLeft]
            {
                water_block[Corners::Left] = water_block[Corners::Left].decrement();
                water_block[Corners::Right] = water_block[Corners::Right].decrement();
                water_block[Corners::FarRight] = water_block[Corners::FarRight].decrement();
                water_block[Corners::FarLeft] = water_block[Corners::FarLeft].decrement();

                continue;
            }

            for i in 1..4 {
                let rotation = BlockRotation::from(i);
                self.water_to_block.insert(
                    water_block.rotate(rotation),
                    (block_id, Some(BlockState::new(rotation))),
                );
                self.block_to_water.insert(
                    (block_id, Some(BlockState::new(rotation))),
                    water_block.rotate(rotation),
                );
            }

            water_block[Corners::Left] = water_block[Corners::Left].decrement();
            water_block[Corners::Right] = water_block[Corners::Right].decrement();
            water_block[Corners::FarRight] = water_block[Corners::FarRight].decrement();
            water_block[Corners::FarLeft] = water_block[Corners::FarLeft].decrement();
        }
    }
}

#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug)]
enum WaterLevel {
    #[default]
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
}

impl WaterLevel {
    #[track_caller]
    fn decrement(self) -> Self {
        match self {
            Self::Zero => unreachable!(),
            Self::One => Self::Zero,
            Self::Two => Self::One,
            Self::Three => Self::Two,
            Self::Four => Self::Three,
            Self::Five => Self::Four,
            Self::Six => Self::Five,
            Self::Seven => Self::Six,
            Self::Eight => Self::Seven,
            Self::Nine => Self::Eight,
            Self::Ten => Self::Nine,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Corners {
    Left = 0,
    Right,
    FarRight,
    FarLeft,
}

impl Corners {
    fn rotate(&self, rotation: BlockRotation) -> Self {
        let new = (*self as usize + rotation as usize) % 4;
        match new {
            0 => Corners::Left,
            1 => Corners::Right,
            2 => Corners::FarRight,
            3 => Corners::FarLeft,
            _ => unreachable!(),
        }
    }
}

#[derive(Default, PartialEq, Eq, Hash, Clone, Debug)]
struct WaterBlock {
    corners: [WaterLevel; 4],
    is_source: bool,
}

impl WaterBlock {
    fn rotate(&self, rotation: BlockRotation) -> Self {
        match rotation {
            BlockRotation::Once => WaterBlock {
                corners: [
                    self.corners[Corners::FarLeft as usize],
                    self.corners[Corners::Left as usize],
                    self.corners[Corners::Right as usize],
                    self.corners[Corners::FarRight as usize],
                ],
                is_source: self.is_source,
            },
            BlockRotation::Twice => WaterBlock {
                corners: [
                    self.corners[Corners::FarRight as usize],
                    self.corners[Corners::FarLeft as usize],
                    self.corners[Corners::Left as usize],
                    self.corners[Corners::Right as usize],
                ],
                is_source: self.is_source,
            },
            BlockRotation::Thrice => WaterBlock {
                corners: [
                    self.corners[Corners::Right as usize],
                    self.corners[Corners::FarRight as usize],
                    self.corners[Corners::FarLeft as usize],
                    self.corners[Corners::Left as usize],
                ],
                is_source: self.is_source,
            },
            _ => unreachable!(),
        }
    }

    fn update_corner(&mut self, corner: Corners, water_level: WaterLevel) {
        if self.corners[corner as usize] == water_level
            || self.corners[corner as usize] == WaterLevel::Ten
        {
            return;
        }

        self.corners[corner as usize] = water_level;

        if self[corner.rotate(BlockRotation::Once)] < self[corner] {
            self[corner.rotate(BlockRotation::Once)] = self[corner].decrement();
        }

        if self[corner.rotate(BlockRotation::Thrice)] < self[corner] {
            self[corner.rotate(BlockRotation::Thrice)] = self[corner].decrement();
        }

        if self[corner.rotate(BlockRotation::Twice)] < self[corner.rotate(BlockRotation::Once)]
            || self[corner.rotate(BlockRotation::Twice)]
                < self[corner.rotate(BlockRotation::Thrice)]
        {
            self[corner.rotate(BlockRotation::Twice)] = self[corner.rotate(BlockRotation::Once)]
                .max(self[corner.rotate(BlockRotation::Thrice)])
                .decrement();
        }
    }
}

impl Index<Corners> for WaterBlock {
    type Output = WaterLevel;
    fn index(&self, index: Corners) -> &Self::Output {
        &self.corners[index as usize]
    }
}

impl IndexMut<Corners> for WaterBlock {
    fn index_mut(&mut self, index: Corners) -> &mut Self::Output {
        &mut self.corners[index as usize]
    }
}

const TEN: WaterBlock = WaterBlock {
    corners: [
        WaterLevel::Ten,
        WaterLevel::Ten,
        WaterLevel::Ten,
        WaterLevel::Ten,
    ],
    is_source: false,
};

#[derive(Debug)]
struct ChangedBlockAsWater {
    pub to: Option<WaterBlock>,
    pub top: Option<WaterBlock>,
    pub bottom: Option<WaterBlock>,
    pub back: Option<WaterBlock>,
    pub back_right: Option<WaterBlock>,
    pub back_left: Option<WaterBlock>,
    pub right: Option<WaterBlock>,
    pub left: Option<WaterBlock>,
    pub front: Option<WaterBlock>,
    pub front_right: Option<WaterBlock>,
    pub front_left: Option<WaterBlock>,
}

impl ChangedBlockAsWater {
    fn new(changed_block: &ChangedBlockEvent, water: &Res<Water>) -> Self {
        Self {
            to: water.block_to_water.get(&changed_block.to).cloned(),
            top: changed_block
                .top
                .as_ref()
                .and_then(|top| water.block_to_water.get(top).cloned()),
            bottom: changed_block
                .bottom
                .as_ref()
                .and_then(|bottom| water.block_to_water.get(bottom).cloned()),
            back: changed_block
                .back
                .as_ref()
                .and_then(|back| water.block_to_water.get(back).cloned()),
            back_right: changed_block
                .back_right
                .as_ref()
                .and_then(|back_right| water.block_to_water.get(back_right).cloned()),
            back_left: changed_block
                .back_left
                .as_ref()
                .and_then(|back_left| water.block_to_water.get(back_left).cloned()),
            right: changed_block
                .right
                .as_ref()
                .and_then(|right| water.block_to_water.get(right).cloned()),
            left: changed_block
                .left
                .as_ref()
                .and_then(|left| water.block_to_water.get(left).cloned()),
            front: changed_block
                .front
                .as_ref()
                .and_then(|front| water.block_to_water.get(front).cloned()),
            front_right: changed_block
                .front_right
                .as_ref()
                .and_then(|front_right| water.block_to_water.get(front_right).cloned()),
            front_left: changed_block
                .front_left
                .as_ref()
                .and_then(|front_left| water.block_to_water.get(front_left).cloned()),
        }
    }
}

impl Index<BlockFace> for ChangedBlockAsWater {
    type Output = Option<WaterBlock>;
    fn index(&self, index: BlockFace) -> &Self::Output {
        match index {
            BlockFace::Front => &self.front,
            BlockFace::Back => &self.back,
            BlockFace::Right => &self.right,
            BlockFace::Left => &self.left,
            BlockFace::Top => &self.top,
            BlockFace::Bottom => &self.bottom,
        }
    }
}

impl Index<[BlockFace; 2]> for ChangedBlockAsWater {
    type Output = Option<WaterBlock>;
    #[track_caller]
    fn index(&self, index: [BlockFace; 2]) -> &Self::Output {
        match index {
            [BlockFace::Front, BlockFace::Left] => &self.front_left,
            [BlockFace::Left, BlockFace::Front] => &self.front_left,
            [BlockFace::Front, BlockFace::Right] => &self.front_right,
            [BlockFace::Right, BlockFace::Front] => &self.front_right,
            [BlockFace::Back, BlockFace::Left] => &self.back_left,
            [BlockFace::Left, BlockFace::Back] => &self.back_left,
            [BlockFace::Back, BlockFace::Right] => &self.back_right,
            [BlockFace::Right, BlockFace::Back] => &self.back_right,
            _ => panic!("Tried to index with non-horizontal blockfaces."),
        }
    }
}

#[derive(Resource, DerefMut, Deref)]
struct WaterUpdateTimer(Timer);

//// TODO: I want waterfalls, but there is currently no way to know which water blocks should spread
//// in a new chunk, and checking all of them would be too expensive... Maybe generate with a dummy
//// block that can use it's spawn function to trigger something.
//// This also makes for silly looking reverse moon pools when caves generate into a body of water.
fn spread_water(
    water: Res<Water>,
    time: Res<Time>,
    mut update_timer: ResMut<WaterUpdateTimer>,
    mut changed_blocks: EventReader<ChangedBlockEvent>,
    mut block_updates: EventWriter<BlockUpdate>,
    mut updates: Local<HashMap<IVec3, WaterBlock>>,
) {
    let blocks = Blocks::get();
    let air = blocks.get_id("air");

    for changed_block in changed_blocks.read() {
        // If there's an update waiting to be sent, but the block is changed, the update is stale
        updates.remove(&changed_block.position);

        let change_as_water = ChangedBlockAsWater::new(changed_block, &water);

        let mut water_block = if let Some(to) = &change_as_water.to {
            if to.is_source {
                to.clone()
            } else {
                let new_max = to.corners.iter().max().unwrap().decrement();
                WaterBlock {
                    corners: [
                        to[Corners::Left].min(new_max),
                        to[Corners::Right].min(new_max),
                        to[Corners::FarRight].min(new_max),
                        to[Corners::FarLeft].min(new_max),
                    ],
                    is_source: false,
                }
            }
        } else {
            WaterBlock::default()
        };

        if change_as_water.top.is_some() {
            water_block.corners = [
                WaterLevel::Nine,
                WaterLevel::Nine,
                WaterLevel::Nine,
                WaterLevel::Nine,
            ]
        } else if !water_block.is_source {
            for (corner, block_faces) in [
                (Corners::Left, [BlockFace::Left, BlockFace::Front]),
                (Corners::Left, [BlockFace::Front, BlockFace::Left]),
                (Corners::Right, [BlockFace::Right, BlockFace::Front]),
                (Corners::Right, [BlockFace::Front, BlockFace::Right]),
                (Corners::FarRight, [BlockFace::Right, BlockFace::Back]),
                (Corners::FarRight, [BlockFace::Back, BlockFace::Right]),
                (Corners::FarLeft, [BlockFace::Left, BlockFace::Back]),
                (Corners::FarLeft, [BlockFace::Back, BlockFace::Left]),
            ] {
                if let Some(adjacent_water_block) = &change_as_water[block_faces[0]] {
                    let (corner_one, corner_two) = match (corner, block_faces[0]) {
                        (Corners::Left, BlockFace::Left) => (Corners::Right, Corners::Left),
                        (Corners::Left, BlockFace::Front) => (Corners::FarLeft, Corners::Left),
                        (Corners::Right, BlockFace::Right) => (Corners::Left, Corners::Right),
                        (Corners::Right, BlockFace::Front) => (Corners::FarRight, Corners::Right),
                        (Corners::FarRight, BlockFace::Right) => {
                            (Corners::FarLeft, Corners::FarRight)
                        }
                        (Corners::FarRight, BlockFace::Back) => (Corners::Right, Corners::FarRight),
                        (Corners::FarLeft, BlockFace::Left) => {
                            (Corners::FarRight, Corners::FarLeft)
                        }
                        (Corners::FarLeft, BlockFace::Back) => (Corners::Left, Corners::FarLeft),
                        _ => unreachable!(),
                    };
                    if adjacent_water_block.is_source {
                        water_block.update_corner(corner, WaterLevel::Nine);
                    } else {
                        water_block.update_corner(
                            corner,
                            water_block[corner]
                                .max(adjacent_water_block[corner_one].decrement())
                                .max(adjacent_water_block[corner_two].decrement()),
                        );
                    }
                }

                if change_as_water[block_faces[0]].is_some()
                    && change_as_water[block_faces].is_some()
                {
                    let diagonal_water_block = change_as_water[block_faces].as_ref().unwrap();
                    let (corner_near, corner_far) = match block_faces {
                        // left corner
                        [BlockFace::Left, BlockFace::Front] => (Corners::FarRight, Corners::Right),
                        [BlockFace::Front, BlockFace::Left] => {
                            (Corners::FarLeft, Corners::FarRight)
                        }
                        // right corner
                        [BlockFace::Right, BlockFace::Front] => (Corners::FarLeft, Corners::Left),
                        [BlockFace::Front, BlockFace::Right] => {
                            (Corners::FarLeft, Corners::FarRight)
                        }
                        // far right corner
                        [BlockFace::Right, BlockFace::Back] => (Corners::FarLeft, Corners::Left),
                        [BlockFace::Back, BlockFace::Right] => (Corners::Left, Corners::Right),
                        // far left corner
                        [BlockFace::Left, BlockFace::Back] => (Corners::FarRight, Corners::Right),
                        [BlockFace::Back, BlockFace::Left] => (Corners::Left, Corners::Right),
                        _ => unreachable!(),
                    };
                    if diagonal_water_block.is_source {
                        water_block.update_corner(corner, WaterLevel::Nine);
                    } else {
                        water_block.update_corner(
                            corner,
                            water_block[corner]
                                .max(diagonal_water_block[corner_near].decrement())
                                .max(diagonal_water_block[corner_far].decrement()),
                        );
                    }
                }
            }
        }

        if water_block != WaterBlock::default()
            && water_block.corners.iter().any(|c| c == &WaterLevel::Zero)
        {
            water_block.corners = WaterBlock::default().corners;
        }

        if water_block == WaterBlock::default() && change_as_water.to.is_none() {
            continue;
        }

        for block_face in [
            BlockFace::Front,
            BlockFace::Right,
            BlockFace::Back,
            BlockFace::Left,
        ] {
            //let (orthogonal_one, orthogonal_two) = match block_face {
            //    BlockFace::Front | BlockFace::Back => (BlockFace::Right, BlockFace::Left),
            //    BlockFace::Left | BlockFace::Right => (BlockFace::Front, BlockFace::Back),
            //    _ => unreachable!(),
            //};
            let to_corners = match block_face {
                BlockFace::Left => [Corners::Right, Corners::FarRight],
                BlockFace::Right => [Corners::Left, Corners::FarLeft],
                BlockFace::Front => [Corners::FarLeft, Corners::FarRight],
                BlockFace::Back => [Corners::Left, Corners::Right],
                _ => unreachable!(),
            };
            let from_corners = match block_face {
                BlockFace::Left => [Corners::Left, Corners::FarLeft],
                BlockFace::Right => [Corners::Right, Corners::FarRight],
                BlockFace::Front => [Corners::Left, Corners::Right],
                BlockFace::Back => [Corners::FarLeft, Corners::FarRight],
                _ => unreachable!(),
            };
            let position = block_face.shift_position(changed_block.position);

            if let Some(adjacent_water_block) = &change_as_water[block_face] {
                //if change_as_water.to.is_some()
                //    || (change_as_water[orthogonal_one].is_some()
                //        && change_as_water[[orthogonal_one, block_face]].is_some())
                //    || (change_as_water[orthogonal_two].is_some()
                //        && change_as_water[[orthogonal_two, block_face]].is_some())
                //{
                let mut update = updates
                    .get(&position)
                    .unwrap_or(adjacent_water_block)
                    .clone();
                update.update_corner(to_corners[0], water_block[from_corners[0]]);
                update.update_corner(to_corners[1], water_block[from_corners[1]]);

                if &update != adjacent_water_block {
                    updates.insert(position, update);
                }
                //}
            } else if changed_block[block_face].is_some_and(|b| b.0 == air)
                && changed_block.bottom.is_some_and(|b| b.0 != air)
                && change_as_water.bottom.is_none()
                && water_block[from_corners[0]] > WaterLevel::One
                && water_block[from_corners[1]] > WaterLevel::One
            {
                let update = updates.entry(position).or_insert(WaterBlock::default());
                if update != &TEN {
                    update.update_corner(to_corners[0], water_block[from_corners[0]]);
                    update.update_corner(to_corners[1], water_block[from_corners[1]]);
                }
            }
        }

        for (block_faces, corner_to, corner_from) in [
            (
                [BlockFace::Front, BlockFace::Left],
                Corners::FarRight,
                Corners::Left,
            ),
            (
                [BlockFace::Front, BlockFace::Right],
                Corners::FarLeft,
                Corners::Right,
            ),
            (
                [BlockFace::Back, BlockFace::Right],
                Corners::Left,
                Corners::FarRight,
            ),
            (
                [BlockFace::Back, BlockFace::Left],
                Corners::Right,
                Corners::FarLeft,
            ),
        ] {
            if let Some(diagonal_water_block) = &change_as_water[block_faces] {
                if change_as_water[block_faces[0]].is_some()
                    || change_as_water[block_faces[1]].is_some()
                {
                    let position = block_faces[0]
                        .shift_position(block_faces[1].shift_position(changed_block.position));
                    let mut update = updates
                        .get(&position)
                        .unwrap_or(diagonal_water_block)
                        .clone();
                    update.update_corner(corner_to, water_block[corner_from]);
                    if &update != diagonal_water_block {
                        updates.insert(position, update);
                    }
                }
            }
        }

        if let Some(bottom) = &change_as_water.bottom {
            if water_block == WaterBlock::default() {
                updates.insert(
                    changed_block.position - IVec3::Y,
                    WaterBlock {
                        corners: [
                            WaterLevel::Nine,
                            WaterLevel::Nine,
                            WaterLevel::Nine,
                            WaterLevel::Nine,
                        ],
                        is_source: bottom.is_source,
                    },
                );
            }
        }

        if changed_block.bottom.is_some_and(|block| block.0 == air) {
            updates.insert(changed_block.position - IVec3::Y, TEN.clone());
        } else if let Some(bottom) = change_as_water.bottom {
            if bottom.corners != TEN.corners {
                let mut new = TEN.clone();
                new.is_source = bottom.is_source;
                updates.insert(changed_block.position - IVec3::Y, new);
            }
        }

        if changed_block.to.0 == air
            || change_as_water
                .to
                .is_some_and(|to| to != water_block && to.corners != TEN.corners)
        {
            if change_as_water.top.is_some() {
                updates.insert(changed_block.position, TEN.clone());
            } else {
                updates.insert(changed_block.position, water_block);
            }
        }
    }

    update_timer.tick(time.delta());
    if update_timer.just_finished() {
        block_updates.send_batch(updates.drain().filter_map(|(position, water_block)| {
            let (block_id, block_state) = match water.water_to_block.get(&water_block) {
                Some(k) => k.clone(),
                None => (air, None),
            };
            // TODO: The idea is that it's not supposed to generate invalid water states, but it
            // does often when trying to remove the water at edges. Ending up with states like
            // [Zero, Zero, One, One] and variations. This is probably what introduces the
            // flickering that sometimes happen.
            //let (block_id, block_state) = water.water_to_block[&water_block];
            Some(BlockUpdate::Change {
                position,
                block_id,
                block_state,
            })
        }));
    }
}
