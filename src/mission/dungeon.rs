//! Procedural dungeon generation using Binary Space Partitioning (BSP).

use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Tile types in the dungeon grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum Tile {
    Wall,
    Floor,
    Door,
    Corridor,
}

/// What kind of room this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum RoomType {
    Normal,
    Entrance,
    Boss,
    Treasure,
}

/// A room in the dungeon.
#[derive(Debug, Clone, Serialize, Deserialize, Reflect)]
pub struct Room {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub room_type: RoomType,
}

impl Room {
    pub fn center(&self) -> (u32, u32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
}

/// A procedurally generated dungeon map.
#[derive(Component, Debug, Clone, Serialize, Deserialize, Reflect)]
pub struct DungeonMap {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Tile>,
    pub rooms: Vec<Room>,
}

impl DungeonMap {
    /// Get the tile at (x, y). Returns Wall if out of bounds.
    pub fn get(&self, x: u32, y: u32) -> Tile {
        if x >= self.width || y >= self.height {
            return Tile::Wall;
        }
        self.tiles[(y * self.width + x) as usize]
    }

    /// Set the tile at (x, y).
    pub fn set(&mut self, x: u32, y: u32, tile: Tile) {
        if x < self.width && y < self.height {
            self.tiles[(y * self.width + x) as usize] = tile;
        }
    }

    /// Check if a position is walkable (not a wall).
    pub fn is_walkable(&self, x: u32, y: u32) -> bool {
        matches!(self.get(x, y), Tile::Floor | Tile::Door | Tile::Corridor)
    }

    /// Find which room contains a given position, if any.
    pub fn room_at(&self, x: u32, y: u32) -> Option<usize> {
        self.rooms
            .iter()
            .position(|r| x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h)
    }

    /// Get the entrance room.
    pub fn entrance_room(&self) -> Option<&Room> {
        self.rooms
            .iter()
            .find(|r| r.room_type == RoomType::Entrance)
    }
}

/// BSP node used during generation.
struct BspNode {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    left: Option<Box<BspNode>>,
    right: Option<Box<BspNode>>,
    room: Option<Room>,
}

const MIN_LEAF_SIZE: u32 = 8;
const MIN_ROOM_SIZE: u32 = 4;
const ROOM_PADDING: u32 = 1;

/// Generate a dungeon map using BSP.
pub fn generate_dungeon(
    width: u32,
    height: u32,
    target_rooms: u32,
    rng: &mut impl Rng,
) -> DungeonMap {
    let mut map = DungeonMap {
        width,
        height,
        tiles: vec![Tile::Wall; (width * height) as usize],
        rooms: Vec::new(),
    };

    // Build BSP tree
    let mut root = BspNode {
        x: 0,
        y: 0,
        w: width,
        h: height,
        left: None,
        right: None,
        room: None,
    };

    // Calculate max depth based on target rooms
    let max_depth = (target_rooms as f32).log2().ceil() as u32 + 1;
    split_node(&mut root, 0, max_depth, rng);

    // Create rooms in leaf nodes
    create_rooms(&mut root, &mut map, rng);

    // Connect rooms with corridors
    connect_rooms(&root, &mut map, rng);

    // Assign room types
    assign_room_types(&mut map, rng);

    info!(
        "Generated dungeon: {}x{}, {} rooms",
        width,
        height,
        map.rooms.len()
    );

    map
}

fn split_node(node: &mut BspNode, depth: u32, max_depth: u32, rng: &mut impl Rng) {
    if depth >= max_depth {
        return;
    }

    // Don't split if too small
    if node.w < MIN_LEAF_SIZE * 2 && node.h < MIN_LEAF_SIZE * 2 {
        return;
    }

    // Decide split direction
    let split_horizontal = if node.w < MIN_LEAF_SIZE * 2 {
        true
    } else if node.h < MIN_LEAF_SIZE * 2 {
        false
    } else {
        let ratio = node.w as f32 / node.h as f32;
        if ratio > 1.25 {
            false // Much wider than tall, force a vertical slice
        } else if ratio < 0.8 {
            true // Much taller than wide, force a horizontal slice
        } else {
            rng.random_bool(0.5) // Roughly square, pick randomly
        }
    };

    if split_horizontal {
        if node.h < MIN_LEAF_SIZE * 2 {
            return;
        }
        let split = rng.random_range(MIN_LEAF_SIZE..node.h - MIN_LEAF_SIZE + 1);
        node.left = Some(Box::new(BspNode {
            x: node.x,
            y: node.y,
            w: node.w,
            h: split,
            left: None,
            right: None,
            room: None,
        }));
        node.right = Some(Box::new(BspNode {
            x: node.x,
            y: node.y + split,
            w: node.w,
            h: node.h - split,
            left: None,
            right: None,
            room: None,
        }));
    } else {
        if node.w < MIN_LEAF_SIZE * 2 {
            return;
        }
        let split = rng.random_range(MIN_LEAF_SIZE..node.w - MIN_LEAF_SIZE + 1);
        node.left = Some(Box::new(BspNode {
            x: node.x,
            y: node.y,
            w: split,
            h: node.h,
            left: None,
            right: None,
            room: None,
        }));
        node.right = Some(Box::new(BspNode {
            x: node.x + split,
            y: node.y,
            w: node.w - split,
            h: node.h,
            left: None,
            right: None,
            room: None,
        }));
    }

    if let Some(ref mut left) = node.left {
        split_node(left, depth + 1, max_depth, rng);
    }
    if let Some(ref mut right) = node.right {
        split_node(right, depth + 1, max_depth, rng);
    }
}

fn create_rooms(node: &mut BspNode, map: &mut DungeonMap, rng: &mut impl Rng) {
    if node.left.is_some() || node.right.is_some() {
        // Internal node — recurse
        if let Some(ref mut left) = node.left {
            create_rooms(left, map, rng);
        }
        if let Some(ref mut right) = node.right {
            create_rooms(right, map, rng);
        }
    } else {
        // Leaf node — create a room
        let max_w = node.w.saturating_sub(ROOM_PADDING * 2);
        let max_h = node.h.saturating_sub(ROOM_PADDING * 2);

        if max_w < MIN_ROOM_SIZE || max_h < MIN_ROOM_SIZE {
            return;
        }

        let room_w = rng.random_range(MIN_ROOM_SIZE..=max_w);
        let room_h = rng.random_range(MIN_ROOM_SIZE..=max_h);
        let room_x = node.x + ROOM_PADDING + rng.random_range(0..=max_w - room_w);
        let room_y = node.y + ROOM_PADDING + rng.random_range(0..=max_h - room_h);

        let room = Room {
            x: room_x,
            y: room_y,
            w: room_w,
            h: room_h,
            room_type: RoomType::Normal,
        };

        // Carve room into map
        for ry in room.y..room.y + room.h {
            for rx in room.x..room.x + room.w {
                map.set(rx, ry, Tile::Floor);
            }
        }

        node.room = Some(room.clone());
        map.rooms.push(room);
    }
}

fn connect_rooms(node: &BspNode, map: &mut DungeonMap, rng: &mut impl Rng) {
    if let (Some(left), Some(right)) = (&node.left, &node.right) {
        // Recursively connect within children
        connect_rooms(left, map, rng);
        connect_rooms(right, map, rng);

        // Connect the two subtrees
        let left_center = find_room_center(left, rng);
        let right_center = find_room_center(right, rng);

        if let (Some((lx, ly)), Some((rx, ry))) = (left_center, right_center) {
            carve_corridor(map, lx, ly, rx, ry, rng);
        }
    }
}

fn find_room_center(node: &BspNode, rng: &mut impl Rng) -> Option<(u32, u32)> {
    if let Some(ref room) = node.room {
        return Some(room.center());
    }

    // Randomly choose which child to search first
    let search_left_first = rng.random_bool(0.5);

    if search_left_first {
        if let Some(ref left) = node.left {
            if let Some(center) = find_room_center(left, rng) {
                return Some(center);
            }
        }
        if let Some(ref right) = node.right {
            if let Some(center) = find_room_center(right, rng) {
                return Some(center);
            }
        }
    } else {
        if let Some(ref right) = node.right {
            if let Some(center) = find_room_center(right, rng) {
                return Some(center);
            }
        }
        if let Some(ref left) = node.left {
            if let Some(center) = find_room_center(left, rng) {
                return Some(center);
            }
        }
    }
    None
}

fn carve_corridor(map: &mut DungeonMap, x1: u32, y1: u32, x2: u32, y2: u32, rng: &mut impl Rng) {
    // L-shaped corridor: go horizontal first or vertical first
    let horizontal_first = rng.random_bool(0.5);

    if horizontal_first {
        carve_h_line(map, x1, x2, y1);
        carve_v_line(map, y1, y2, x2);
    } else {
        carve_v_line(map, y1, y2, x1);
        carve_h_line(map, x1, x2, y2);
    }
}

fn carve_h_line(map: &mut DungeonMap, x1: u32, x2: u32, y: u32) {
    let (start, end) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
    for x in start..=end {
        if map.get(x, y) == Tile::Wall {
            map.set(x, y, Tile::Corridor);
        }
    }
}

fn carve_v_line(map: &mut DungeonMap, y1: u32, y2: u32, x: u32) {
    let (start, end) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
    for y in start..=end {
        if map.get(x, y) == Tile::Wall {
            map.set(x, y, Tile::Corridor);
        }
    }
}

fn assign_room_types(map: &mut DungeonMap, rng: &mut impl Rng) {
    if map.rooms.is_empty() {
        return;
    }

    // First room = entrance, last room = boss
    map.rooms[0].room_type = RoomType::Entrance;
    let last = map.rooms.len() - 1;
    if last > 0 {
        map.rooms[last].room_type = RoomType::Boss;
    }

    // Randomly assign one room as treasure (if there are enough rooms)
    if map.rooms.len() > 3 {
        let treasure_idx = rng.random_range(1..last);
        map.rooms[treasure_idx].room_type = RoomType::Treasure;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_valid_dungeon() {
        let mut rng = rand::rng();
        let map = generate_dungeon(40, 30, 6, &mut rng);

        assert!(map.rooms.len() >= 2, "Should have at least 2 rooms");
        assert!(
            map.rooms.iter().any(|r| r.room_type == RoomType::Entrance),
            "Should have an entrance room"
        );
        assert!(
            map.rooms.iter().any(|r| r.room_type == RoomType::Boss),
            "Should have a boss room"
        );

        // Check that rooms have floor tiles
        for room in &map.rooms {
            let cx = room.x + room.w / 2;
            let cy = room.y + room.h / 2;
            assert_eq!(map.get(cx, cy), Tile::Floor, "Room center should be floor");
        }
    }

    #[test]
    fn entrance_is_walkable() {
        let mut rng = rand::rng();
        let map = generate_dungeon(40, 30, 5, &mut rng);
        let entrance = map.entrance_room().expect("Should have entrance");
        let (cx, cy) = entrance.center();
        assert!(map.is_walkable(cx, cy));
    }
}
