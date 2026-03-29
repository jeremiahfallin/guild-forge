//! A* grid pathfinding on the dungeon map.

use std::collections::BinaryHeap;
use std::cmp::Ordering;

use super::dungeon::DungeonMap;

/// A position + cost entry for the A* open set.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct Node {
    pos: (u32, u32),
    f_score: u32,
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for min-heap behavior
        other.f_score.cmp(&self.f_score)
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Find a path from `start` to `goal` on the dungeon grid using A*.
/// Returns `None` if no path exists.
pub fn find_path(
    map: &DungeonMap,
    start: (u32, u32),
    goal: (u32, u32),
) -> Option<Vec<(u32, u32)>> {
    if start == goal {
        return Some(vec![start]);
    }
    if !map.is_walkable(start.0, start.1) || !map.is_walkable(goal.0, goal.1) {
        return None;
    }

    let w = map.width as usize;
    let h = map.height as usize;
    let size = w * h;

    let idx = |x: u32, y: u32| -> usize { (y as usize) * w + (x as usize) };

    // g_score: best cost from start to this node
    let mut g_score = vec![u32::MAX; size];
    // came_from: parent node for path reconstruction
    let mut came_from: Vec<Option<(u32, u32)>> = vec![None; size];

    g_score[idx(start.0, start.1)] = 0;

    let mut open = BinaryHeap::new();
    open.push(Node {
        pos: start,
        f_score: heuristic(start, goal),
    });

    let neighbors: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

    while let Some(current) = open.pop() {
        let (cx, cy) = current.pos;

        if current.pos == goal {
            // Reconstruct path
            let mut path = Vec::new();
            let mut pos = goal;
            path.push(pos);
            while let Some(prev) = came_from[idx(pos.0, pos.1)] {
                path.push(prev);
                pos = prev;
            }
            path.reverse();
            return Some(path);
        }

        let current_g = g_score[idx(cx, cy)];

        // Skip if we've already found a better path to this node
        if current.f_score > current_g + heuristic(current.pos, goal) + 1 {
            continue;
        }

        for &(dx, dy) in &neighbors {
            let nx = cx as i32 + dx;
            let ny = cy as i32 + dy;

            if nx < 0 || ny < 0 || nx >= map.width as i32 || ny >= map.height as i32 {
                continue;
            }

            let nx = nx as u32;
            let ny = ny as u32;

            if !map.is_walkable(nx, ny) {
                continue;
            }

            let tentative_g = current_g + 1;
            let ni = idx(nx, ny);

            if tentative_g < g_score[ni] {
                g_score[ni] = tentative_g;
                came_from[ni] = Some((cx, cy));
                open.push(Node {
                    pos: (nx, ny),
                    f_score: tentative_g + heuristic((nx, ny), goal),
                });
            }
        }
    }

    None // No path found
}

/// Manhattan distance heuristic.
fn heuristic(a: (u32, u32), b: (u32, u32)) -> u32 {
    a.0.abs_diff(b.0) + a.1.abs_diff(b.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::dungeon::generate_dungeon;

    #[test]
    fn finds_path_between_rooms() {
        let mut rng = rand::rng();
        let map = generate_dungeon(40, 30, 6, &mut rng);

        // Path from entrance center to boss center
        let entrance = map.entrance_room().expect("Should have entrance");
        let start = entrance.center();

        let boss = map
            .rooms
            .iter()
            .find(|r| r.room_type == crate::mission::dungeon::RoomType::Boss)
            .expect("Should have boss room");
        let goal = boss.center();

        let path = find_path(&map, start, goal);
        assert!(path.is_some(), "Should find a path from entrance to boss");

        let path = path.unwrap();
        assert_eq!(path.first(), Some(&start));
        assert_eq!(path.last(), Some(&goal));
        assert!(path.len() > 1, "Path should have multiple steps");

        // Verify all steps are walkable
        for &(x, y) in &path {
            assert!(map.is_walkable(x, y), "All path tiles should be walkable");
        }
    }

    #[test]
    fn path_to_self_is_single_step() {
        let mut rng = rand::rng();
        let map = generate_dungeon(40, 30, 4, &mut rng);
        let entrance = map.entrance_room().expect("Should have entrance");
        let pos = entrance.center();

        let path = find_path(&map, pos, pos);
        assert_eq!(path, Some(vec![pos]));
    }

    #[test]
    fn no_path_to_wall() {
        let mut rng = rand::rng();
        let map = generate_dungeon(40, 30, 4, &mut rng);
        let entrance = map.entrance_room().expect("Should have entrance");
        let start = entrance.center();

        // (0, 0) is almost certainly a wall
        let path = find_path(&map, start, (0, 0));
        assert!(path.is_none(), "Should not find path to a wall");
    }
}
