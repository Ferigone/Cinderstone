#![allow(dead_code)]
#[path = "../src/geometry.rs"]
mod geometry;
use geometry::*;
use std::io::{self, BufRead};
fn d(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).unwrap())
}
fn bits(n: f64) -> String {
    format!(
        "{:016x}",
        if n.is_nan() {
            f64::NAN.to_bits()
        } else {
            n.to_bits()
        }
    )
}
fn vec(f: &[&str], i: usize) -> Vec3 {
    Vec3::new(d(f[i]), d(f[i + 1]), d(f[i + 2]))
}
fn box_(f: &[&str], i: usize) -> Aabb {
    Aabb::new(
        d(f[i]),
        d(f[i + 1]),
        d(f[i + 2]),
        d(f[i + 3]),
        d(f[i + 4]),
        d(f[i + 5]),
    )
}
fn vo(v: Vec3) -> String {
    [v.x, v.y, v.z].map(bits).join(",")
}
fn bo(b: Aabb) -> String {
    [b.min_x, b.min_y, b.min_z, b.max_x, b.max_y, b.max_z]
        .map(bits)
        .join(",")
}
fn run(line: &str) -> String {
    let f: Vec<_> = line.split('\t').collect();
    match f[0] {
        "look" => {
            let pitch = f32::from_bits(u32::from_str_radix(f[1], 16).unwrap());
            let yaw = f32::from_bits(u32::from_str_radix(f[2], 16).unwrap());
            format!(
                "{}|{}",
                ordered_look_directions(pitch, yaw)
                    .map(|d| (d as u8).to_string())
                    .join(","),
                horizontal_look_direction(yaw) as u8
            )
        }
        "dynamic" => {
            let context = if f[5].parse::<bool>().unwrap() {
                CollisionContext::Empty
            } else {
                CollisionContext::Entity(EntityCollisionContext {
                    bottom_y: d(f[6]),
                    descending: f[7].parse().unwrap(),
                    fall_distance: f32::from_bits(u32::from_str_radix(f[8], 16).unwrap()),
                    is_falling_block: f[9].parse().unwrap(),
                    can_walk_on_powder_snow: f[10].parse().unwrap(),
                })
            };
            block_collision_boxes(
                f[1].parse().unwrap(),
                (
                    f[2].parse().unwrap(),
                    f[3].parse().unwrap(),
                    f[4].parse().unwrap(),
                ),
                context,
                BlockEntityShape::Absent,
            )
            .unwrap()
            .into_iter()
            .map(bo)
            .collect::<Vec<_>>()
            .join(";")
        }
        "static" => static_block_collision_boxes(f[1].parse().unwrap())
            .map(|boxes| {
                boxes
                    .iter()
                    .map(|b| bo(Aabb::new(b[0], b[1], b[2], b[3], b[4], b[5])))
                    .collect::<Vec<_>>()
                    .join(";")
            })
            .unwrap_or_else(|| "dynamic".into()),
        "reach" => can_interact_with_block(
            player_eye_position(
                vec(&f, 1),
                f32::from_bits(u32::from_str_radix(f[4], 16).unwrap()),
            ),
            (
                f[5].parse().unwrap(),
                f[6].parse().unwrap(),
                f[7].parse().unwrap(),
            ),
            d(f[8]),
            d(f[9]),
        )
        .to_string(),
        "obstruct" => box_(&f, 1).shape_intersects(box_(&f, 7)).to_string(),
        "vec" => {
            let a = vec(&f, 1);
            let b = vec(&f, 4);
            format!(
                "{}|{}|{}|{}|{}",
                vo(a.normalize()),
                bits(a.dot(b)),
                vo(a.cross(b)),
                bits(a.distance_squared(b)),
                vo(a.lerp(b, d(f[7])))
            )
        }
        "box" => {
            let a = box_(&f, 1);
            let b = box_(&f, 7);
            let v = vec(&f, 13);
            format!(
                "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                bo(a),
                a.intersects(&b),
                a.contains(v),
                bo(a.intersect(b)),
                bo(a.minmax(b)),
                bo(a.contract(v)),
                bo(a.expand_towards(v)),
                bo(a.inflate(v)),
                bits(a.distance_squared(v)),
                vo(a.center())
            )
        }
        "clip" => box_(&f, 1)
            .clip(vec(&f, 7), vec(&f, 10))
            .map(|h| vo(h.location))
            .unwrap_or_else(|| "null".into()),
        "dda" => {
            let mut cells = Vec::new();
            traverse_blocks(vec(&f, 1), vec(&f, 4), |p| {
                cells.push(format!("{},{},{}", p.0, p.1, p.2));
                assert!(cells.len() < 4096);
                None::<()>
            });
            cells.join(";")
        }
        "player" => bo(Aabb::player(
            vec(&f, 1),
            f32::from_bits(u32::from_str_radix(f[4], 16).unwrap()),
            f32::from_bits(u32::from_str_radix(f[5], 16).unwrap()),
        )),
        "boxes" | "shapeclip" | "motion" | "collide" => {
            let axes = [Axis::X, Axis::Y, Axis::Z];
            let coords: Vec<Vec<f64>> = (2..5).map(|i| f[i].split(',').map(d).collect()).collect();
            let shape = VoxelShape::new(
                coords[0].clone(),
                coords[1].clone(),
                coords[2].clone(),
                f[5].bytes().map(|b| b == b'1').collect(),
            )
            .unwrap();
            match f[0] {
                "boxes" => shape
                    .to_aabbs()
                    .into_iter()
                    .map(bo)
                    .collect::<Vec<_>>()
                    .join(";"),
                "shapeclip" => shape
                    .clip(
                        vec(&f, 6),
                        vec(&f, 9),
                        (
                            f[12].parse().unwrap(),
                            f[13].parse().unwrap(),
                            f[14].parse().unwrap(),
                        ),
                    )
                    .map(|h| format!("{}|{}|{}", vo(h.location), h.direction as u8, h.inside))
                    .unwrap_or_else(|| "null".into()),
                "motion" => {
                    let shifted = VoxelShape::new(
                        coords[0].iter().map(|v| v + 1.).collect(),
                        coords[1].iter().map(|v| v + 0.5).collect(),
                        coords[2].iter().map(|v| v - 1.).collect(),
                        shape.occupied().to_vec(),
                    )
                    .unwrap();
                    vo(collide_with_shapes(
                        vec(&f, 13),
                        box_(&f, 6),
                        &[shape, shifted],
                    ))
                }
                _ => {
                    bits(shape.collide(axes[f[1].parse::<usize>().unwrap()], box_(&f, 6), d(f[12])))
                }
            }
        }
        _ => panic!("unknown command"),
    }
}
fn main() {
    for line in io::stdin().lock().lines() {
        println!("{}", run(&line.unwrap()));
    }
}
