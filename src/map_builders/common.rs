use rltk::RandomNumberGenerator;

use crate::{Map, Rect, TileType};
use std::{
    cmp::{max, min},
    collections::HashMap,
};

pub fn apply_room_to_map(map: &mut Map, room: &Rect) {
    for y in room.y1 + 1..=room.y2 {
        for x in room.x1 + 1..=room.x2 {
            let idx = map.xy_idx(x, y);
            map.tiles[idx] = TileType::Floor;
        }
    }
}

pub fn apply_horizontal_tunnel(map: &mut Map, x1: i32, x2: i32, y: i32) {
    for x in min(x1, x2)..=max(x1, x2) {
        let idx = map.xy_idx(x, y);
        if idx > 0 && idx < (map.width * map.height) as usize {
            map.tiles[idx] = TileType::Floor;
        }
    }
}
pub fn apply_vertical_tunnel(map: &mut Map, y1: i32, y2: i32, x: i32) {
    for y in min(y1, y2)..=max(y1, y2) {
        let idx = map.xy_idx(x, y);
        if idx > 0 && idx < (map.width * map.height) as usize {
            map.tiles[idx] = TileType::Floor;
        }
    }
}

/// Searches a map via Dijkstra's, removes unreachable areas and returns the most distant tile.
pub fn remove_unreachable_areas_get_most_distant(map: &mut Map, start_idx: usize) -> usize {
    map.populate_blocked();
    // Find all tiles we can reach from the starting point
    let map_starts: Vec<usize> = vec![start_idx];
    let dijkstra_map = rltk::DijkstraMap::new(map.width, map.height, &map_starts, map, 200.0);
    let (mut exit_tile_idx, mut exit_tile_distance) = (0, 0.0f32);
    for (i, tile) in map.tiles.iter_mut().enumerate() {
        if *tile == TileType::Floor {
            let distance_to_start = dijkstra_map.map[i];
            // Unreachable tile -> Wall
            if distance_to_start == std::f32::MAX {
                *tile = TileType::Wall;
            } else if distance_to_start > exit_tile_distance {
                // Move exist if further than current exit
                exit_tile_idx = i;
                exit_tile_distance = distance_to_start;
            }
        }
    }
    exit_tile_idx
}

/// Generates a Voronoi/cellular noise map of a region, and divides it into spawn regions. <https://thebookofshaders.com/12/>
pub fn generate_voronoi_spawn_regions(
    map: &Map,
    rng: &mut RandomNumberGenerator,
) -> HashMap<i32, Vec<usize>> {
    let mut noise_areas: HashMap<i32, Vec<usize>> = HashMap::new();
    let mut noise = rltk::FastNoise::seeded(rng.roll_dice(1, 65536) as u64);
    noise.set_noise_type(rltk::NoiseType::Cellular);
    noise.set_frequency(0.08);
    noise.set_cellular_distance_function(rltk::CellularDistanceFunction::Manhattan);

    for y in 1..map.height - 1 {
        for x in 1..map.width - 1 {
            let idx = map.xy_idx(x, y);
            if map.tiles[idx] == TileType::Floor {
                let cell_value_f = noise.get_noise(x as f32, y as f32) * 10240.0;

                noise_areas
                    .entry(cell_value_f as i32)
                    .or_default()
                    .push(idx);
            }
        }
    }
    noise_areas
}
