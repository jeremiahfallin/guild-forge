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

    /// Pick a random walkable tile inside a room. Room rects may contain
    /// wall tiles (carved corners), so spawn positions must go through this
    /// rather than sampling the rect directly.
    pub fn random_walkable_in_room(
        &self,
        room_idx: usize,
        rng: &mut impl Rng,
    ) -> Option<(u32, u32)> {
        let room = self.rooms.get(room_idx)?;
        let walkable: Vec<(u32, u32)> = (room.y..room.y + room.h)
            .flat_map(|y| (room.x..room.x + room.w).map(move |x| (x, y)))
            .filter(|&(x, y)| self.is_walkable(x, y))
            .collect();
        if walkable.is_empty() {
            return Some(room.center());
        }
        Some(walkable[rng.random_range(0..walkable.len())])
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
    room_idx: Option<usize>,
}

const MIN_LEAF_SIZE: u32 = 8;
const MIN_ROOM_SIZE: u32 = 4;
const ROOM_PADDING: u32 = 1;
/// Fraction of extra corridors added on top of the spanning tree so dungeons
/// have loops instead of a strict branching topology.
const LOOP_EDGE_RATIO: f32 = 0.15;
/// Chance that a large room gets its corners carved back to wall.
const CORNER_CARVE_CHANCE: f64 = 0.5;

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
        room_idx: None,
    };

    // Calculate max depth based on target rooms
    let max_depth = (target_rooms as f32).log2().ceil() as u32 + 1;
    split_node(&mut root, 0, max_depth, rng);

    // Create rooms in leaf nodes
    create_rooms(&mut root, &mut map, rng);

    // Carve corners of large rooms for shape variety. Done before corridors
    // so a corridor punching through a carved corner re-opens it.
    carve_room_corners(&mut map, rng);

    // Connect rooms with corridors, recording which room pairs got linked
    let mut edges: Vec<(usize, usize)> = Vec::new();
    connect_rooms(&root, &mut map, &mut edges, rng);

    // Add a few extra corridors so the dungeon has loops, not just a tree
    add_loop_corridors(&mut map, &mut edges, rng);

    // Turn corridor tiles into doors where they pass through room walls
    place_doors(&mut map);

    // Assign room types based on graph distance from the entrance
    assign_room_types(&mut map, &edges, rng);

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
            room_idx: None,
        }));
        node.right = Some(Box::new(BspNode {
            x: node.x,
            y: node.y + split,
            w: node.w,
            h: node.h - split,
            left: None,
            right: None,
            room_idx: None,
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
            room_idx: None,
        }));
        node.right = Some(Box::new(BspNode {
            x: node.x + split,
            y: node.y,
            w: node.w - split,
            h: node.h,
            left: None,
            right: None,
            room_idx: None,
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

        node.room_idx = Some(map.rooms.len());
        map.rooms.push(room);
    }
}

/// Carve the corners of large rooms back to wall so not every room reads as
/// a plain rectangle. Corner tiles are never a room's center, so chests and
/// hero spawns (which use centers) are unaffected; random in-room spawns must
/// use [`DungeonMap::random_walkable_in_room`].
fn carve_room_corners(map: &mut DungeonMap, rng: &mut impl Rng) {
    let rooms = map.rooms.clone();
    for room in &rooms {
        let min_dim = room.w.min(room.h);
        if min_dim < 6 || !rng.random_bool(CORNER_CARVE_CHANCE) {
            continue;
        }
        let notch = if min_dim >= 8 { 2 } else { 1 };
        for dy in 0..notch {
            for dx in 0..notch {
                map.set(room.x + dx, room.y + dy, Tile::Wall);
                map.set(room.x + room.w - 1 - dx, room.y + dy, Tile::Wall);
                map.set(room.x + dx, room.y + room.h - 1 - dy, Tile::Wall);
                map.set(
                    room.x + room.w - 1 - dx,
                    room.y + room.h - 1 - dy,
                    Tile::Wall,
                );
            }
        }
    }
}

fn connect_rooms(
    node: &BspNode,
    map: &mut DungeonMap,
    edges: &mut Vec<(usize, usize)>,
    rng: &mut impl Rng,
) {
    if let (Some(left), Some(right)) = (&node.left, &node.right) {
        // Recursively connect within children
        connect_rooms(left, map, edges, rng);
        connect_rooms(right, map, edges, rng);

        // Connect the two subtrees through their closest pair of rooms, so
        // corridors stay short instead of lancing across the map.
        let mut left_rooms = Vec::new();
        let mut right_rooms = Vec::new();
        collect_room_indices(left, &mut left_rooms);
        collect_room_indices(right, &mut right_rooms);

        if left_rooms.is_empty() || right_rooms.is_empty() {
            return;
        }

        let (li, ri) = nearest_room_pair(&map.rooms, &left_rooms, &right_rooms);
        let (lx, ly) = map.rooms[li].center();
        let (rx, ry) = map.rooms[ri].center();
        carve_corridor(map, lx, ly, rx, ry, rng);
        edges.push((li, ri));
    }
}

fn collect_room_indices(node: &BspNode, out: &mut Vec<usize>) {
    if let Some(idx) = node.room_idx {
        out.push(idx);
    }
    if let Some(ref left) = node.left {
        collect_room_indices(left, out);
    }
    if let Some(ref right) = node.right {
        collect_room_indices(right, out);
    }
}

/// Find the pair of rooms (one from each set) whose centers are closest.
fn nearest_room_pair(rooms: &[Room], left: &[usize], right: &[usize]) -> (usize, usize) {
    let mut best = (left[0], right[0]);
    let mut best_dist = u64::MAX;
    for &li in left {
        let (lx, ly) = rooms[li].center();
        for &ri in right {
            let (rx, ry) = rooms[ri].center();
            let dx = lx.abs_diff(rx) as u64;
            let dy = ly.abs_diff(ry) as u64;
            let dist = dx * dx + dy * dy;
            if dist < best_dist {
                best_dist = dist;
                best = (li, ri);
            }
        }
    }
    best
}

/// Add extra corridors between nearby rooms that aren't already connected,
/// so the dungeon graph has loops instead of being a strict tree.
fn add_loop_corridors(map: &mut DungeonMap, edges: &mut Vec<(usize, usize)>, rng: &mut impl Rng) {
    let n = map.rooms.len();
    if n < 3 {
        return;
    }
    let extra = (((n - 1) as f32) * LOOP_EDGE_RATIO).ceil() as usize;

    let connected = |edges: &[(usize, usize)], a: usize, b: usize| {
        edges.iter().any(|&(x, y)| (x, y) == (a, b) || (x, y) == (b, a))
    };

    // All unconnected pairs, closest first.
    let mut candidates: Vec<(u64, usize, usize)> = Vec::new();
    for a in 0..n {
        let (ax, ay) = map.rooms[a].center();
        for b in a + 1..n {
            if connected(edges, a, b) {
                continue;
            }
            let (bx, by) = map.rooms[b].center();
            let dx = ax.abs_diff(bx) as u64;
            let dy = ay.abs_diff(by) as u64;
            candidates.push((dx * dx + dy * dy, a, b));
        }
    }
    candidates.sort_unstable();

    for &(_, a, b) in candidates.iter().take(extra) {
        let (ax, ay) = map.rooms[a].center();
        let (bx, by) = map.rooms[b].center();
        carve_corridor(map, ax, ay, bx, by, rng);
        edges.push((a, b));
    }
}

/// Turn corridor tiles into doors where they pass through a room wall: the
/// tile touches room floor along its travel direction and has walls on both
/// perpendicular sides (a doorway, not a corridor running alongside a room).
fn place_doors(map: &mut DungeonMap) {
    let mut doors = Vec::new();
    for y in 0..map.height {
        for x in 0..map.width {
            if map.get(x, y) != Tile::Corridor {
                continue;
            }
            let left = map.get(x.wrapping_sub(1), y);
            let right = map.get(x + 1, y);
            let up = map.get(x, y.wrapping_sub(1));
            let down = map.get(x, y + 1);

            let horizontal_doorway = (left == Tile::Floor || right == Tile::Floor)
                && up == Tile::Wall
                && down == Tile::Wall;
            let vertical_doorway = (up == Tile::Floor || down == Tile::Floor)
                && left == Tile::Wall
                && right == Tile::Wall;

            if horizontal_doorway || vertical_doorway {
                doors.push((x, y));
            }
        }
    }
    for (x, y) in doors {
        map.set(x, y, Tile::Door);
    }
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

fn assign_room_types(map: &mut DungeonMap, edges: &[(usize, usize)], rng: &mut impl Rng) {
    let n = map.rooms.len();
    if n == 0 {
        return;
    }

    map.rooms[0].room_type = RoomType::Entrance;
    if n == 1 {
        return;
    }

    // BFS over the room graph from the entrance.
    let mut adjacency = vec![Vec::new(); n];
    for &(a, b) in edges {
        adjacency[a].push(b);
        adjacency[b].push(a);
    }
    let mut dist = vec![usize::MAX; n];
    dist[0] = 0;
    let mut queue = std::collections::VecDeque::from([0]);
    while let Some(room) = queue.pop_front() {
        for &next in &adjacency[room] {
            if dist[next] == usize::MAX {
                dist[next] = dist[room] + 1;
                queue.push_back(next);
            }
        }
    }
    // Rooms the edge list doesn't reach (shouldn't happen) sort first, so
    // they never get picked as boss.
    let max_dist = *dist.iter().filter(|&&d| d != usize::MAX).max().unwrap_or(&0);

    // Boss goes in the room farthest from the entrance.
    let boss_idx = (1..n)
        .filter(|&i| dist[i] != usize::MAX)
        .max_by_key(|&i| dist[i])
        .unwrap_or(n - 1);
    map.rooms[boss_idx].room_type = RoomType::Boss;

    // Treasure goes in a mid-distance room so it's neither at the door nor
    // colocated with the boss.
    if n > 3 {
        let target = max_dist / 2;
        let candidates: Vec<usize> = (1..n)
            .filter(|&i| i != boss_idx && dist[i] != usize::MAX)
            .collect();
        if !candidates.is_empty() {
            let best_offset = candidates
                .iter()
                .map(|&i| dist[i].abs_diff(target))
                .min()
                .unwrap();
            let mid: Vec<usize> = candidates
                .into_iter()
                .filter(|&i| dist[i].abs_diff(target) == best_offset)
                .collect();
            let treasure_idx = mid[rng.random_range(0..mid.len())];
            map.rooms[treasure_idx].room_type = RoomType::Treasure;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    /// Flood-fill walkable tiles from `start`, returning a reachability grid.
    fn reachable_from(map: &DungeonMap, start: (u32, u32)) -> Vec<bool> {
        let mut seen = vec![false; (map.width * map.height) as usize];
        let mut queue = vec![start];
        seen[(start.1 * map.width + start.0) as usize] = true;
        while let Some((x, y)) = queue.pop() {
            let neighbors = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ];
            for (nx, ny) in neighbors {
                if nx < map.width && ny < map.height && map.is_walkable(nx, ny) {
                    let idx = (ny * map.width + nx) as usize;
                    if !seen[idx] {
                        seen[idx] = true;
                        queue.push((nx, ny));
                    }
                }
            }
        }
        seen
    }

    /// A minimal map with `n` fully-floored rooms laid out in a horizontal row.
    fn row_of_rooms(n: u32) -> DungeonMap {
        let width = n * 10;
        let mut map = DungeonMap {
            width,
            height: 10,
            tiles: vec![Tile::Wall; (width * 10) as usize],
            rooms: Vec::new(),
        };
        for i in 0..n {
            let room = Room {
                x: i * 10 + 1,
                y: 2,
                w: 5,
                h: 5,
                room_type: RoomType::Normal,
            };
            for ry in room.y..room.y + room.h {
                for rx in room.x..room.x + room.w {
                    map.set(rx, ry, Tile::Floor);
                }
            }
            map.rooms.push(room);
        }
        map
    }

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

    #[test]
    fn dungeon_fully_connected_from_entrance() {
        for seed in 0..20 {
            let mut rng = StdRng::seed_from_u64(seed);
            let map = generate_dungeon(40, 30, 6, &mut rng);
            let entrance = map.entrance_room().expect("Should have entrance");
            let reached = reachable_from(&map, entrance.center());
            for (i, room) in map.rooms.iter().enumerate() {
                let (cx, cy) = room.center();
                assert!(
                    reached[(cy * map.width + cx) as usize],
                    "seed {seed}: room {i} center unreachable from entrance"
                );
            }
        }
    }

    #[test]
    fn nearest_pair_picks_closest_rooms() {
        let rooms = vec![
            Room { x: 1, y: 1, w: 3, h: 3, room_type: RoomType::Normal }, // center (2,2)
            Room { x: 1, y: 9, w: 3, h: 3, room_type: RoomType::Normal }, // center (2,10)
            Room { x: 11, y: 9, w: 3, h: 3, room_type: RoomType::Normal }, // center (12,10)
            Room { x: 19, y: 1, w: 3, h: 3, room_type: RoomType::Normal }, // center (20,2)
        ];
        let pair = nearest_room_pair(&rooms, &[0, 1], &[2, 3]);
        assert_eq!(pair, (1, 2), "closest cross-subtree pair is rooms 1 and 2");
    }

    #[test]
    fn loop_corridors_connect_nonadjacent_rooms() {
        let mut map = row_of_rooms(3);
        let mut edges = vec![(0, 1), (1, 2)];
        let mut rng = StdRng::seed_from_u64(7);
        add_loop_corridors(&mut map, &mut edges, &mut rng);
        assert_eq!(edges.len(), 3, "one extra edge for a 3-room dungeon");
        assert!(
            edges.contains(&(0, 2)) || edges.contains(&(2, 0)),
            "the only non-adjacent pair (0,2) should be connected"
        );
        // The new corridor must actually be carved: room 0 and 2 stay connected
        // even if the middle room were sealed off.
        let (cx, cy) = map.rooms[0].center();
        assert!(map.is_walkable(cx, cy));
    }

    #[test]
    fn boss_room_is_farthest_from_entrance_by_room_graph() {
        // Path topology 0 -> 2 -> 3 -> 1: farthest room is index 1, NOT the
        // last room in the vec — this distinguishes BFS placement from the
        // old "last room is boss" rule.
        let mut map = row_of_rooms(4);
        let edges = vec![(0, 2), (2, 3), (3, 1)];
        let mut rng = StdRng::seed_from_u64(7);
        assign_room_types(&mut map, &edges, &mut rng);
        assert_eq!(map.rooms[0].room_type, RoomType::Entrance);
        assert_eq!(map.rooms[1].room_type, RoomType::Boss);
        let treasure_idx = map
            .rooms
            .iter()
            .position(|r| r.room_type == RoomType::Treasure)
            .expect("4-room dungeon should have a treasure room");
        assert!(
            treasure_idx == 2 || treasure_idx == 3,
            "treasure should be a mid-distance room, got {treasure_idx}"
        );
    }

    #[test]
    fn doors_placed_where_corridors_meet_rooms() {
        for seed in 0..10 {
            let mut rng = StdRng::seed_from_u64(seed);
            let map = generate_dungeon(40, 30, 6, &mut rng);
            let mut door_count = 0;
            for y in 0..map.height {
                for x in 0..map.width {
                    if map.get(x, y) != Tile::Door {
                        continue;
                    }
                    door_count += 1;
                    let has_adjacent_floor = [
                        map.get(x.wrapping_sub(1), y),
                        map.get(x + 1, y),
                        map.get(x, y.wrapping_sub(1)),
                        map.get(x, y + 1),
                    ]
                    .contains(&Tile::Floor);
                    assert!(
                        has_adjacent_floor,
                        "seed {seed}: door at ({x},{y}) not adjacent to any room floor"
                    );
                }
            }
            assert!(door_count > 0, "seed {seed}: dungeon has no doors");
        }
    }

    #[test]
    fn carved_corners_appear_in_large_rooms() {
        let mut found_carved_corner = false;
        for seed in 0..20 {
            let mut rng = StdRng::seed_from_u64(seed);
            let map = generate_dungeon(40, 30, 6, &mut rng);
            for room in &map.rooms {
                if room.w.min(room.h) < 6 {
                    continue;
                }
                let corners = [
                    (room.x, room.y),
                    (room.x + room.w - 1, room.y),
                    (room.x, room.y + room.h - 1),
                    (room.x + room.w - 1, room.y + room.h - 1),
                ];
                if corners.iter().any(|&(x, y)| map.get(x, y) == Tile::Wall) {
                    found_carved_corner = true;
                }
            }
        }
        assert!(
            found_carved_corner,
            "no large room had a carved corner across 20 seeds"
        );
    }

    #[test]
    #[ignore = "visual inspection helper"]
    fn print_sample_dungeons() {
        for seed in [1u64, 2, 3] {
            let mut rng = StdRng::seed_from_u64(seed);
            let map = generate_dungeon(40, 30, 6, &mut rng);
            println!("--- seed {seed} ({} rooms) ---", map.rooms.len());
            for y in 0..map.height {
                let line: String = (0..map.width)
                    .map(|x| match map.get(x, y) {
                        Tile::Wall => '#',
                        Tile::Floor => match map.room_at(x, y).map(|i| map.rooms[i].room_type) {
                            Some(RoomType::Entrance) => 'E',
                            Some(RoomType::Boss) => 'B',
                            Some(RoomType::Treasure) => 'T',
                            _ => '.',
                        },
                        Tile::Door => '+',
                        Tile::Corridor => ':',
                    })
                    .collect();
                println!("{line}");
            }
        }
    }

    #[test]
    fn random_walkable_in_room_never_returns_wall() {
        let mut map = row_of_rooms(1);
        // Carve a corner of the room back to wall.
        let room = map.rooms[0].clone();
        map.set(room.x, room.y, Tile::Wall);
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..100 {
            let (x, y) = map
                .random_walkable_in_room(0, &mut rng)
                .expect("room has walkable tiles");
            assert!(map.is_walkable(x, y), "picked wall tile at ({x},{y})");
            assert!(
                x >= room.x && x < room.x + room.w && y >= room.y && y < room.y + room.h,
                "picked tile outside room rect"
            );
        }
    }
}
