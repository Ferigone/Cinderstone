use mc121_rust::{
    geometry::{Direction, Vec3},
    placement::{self, PlacementContext, PlacementResult, PlacementWorld, SurvivalResult},
};
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
struct TestWorld {
    blocks: BTreeMap<(i32, i32, i32), u32>,
    pos: (i32, i32, i32),
    power: i32,
    border: bool,
    max: i32,
    brightness: i32,
}
impl PlacementWorld for TestWorld {
    fn block(&mut self, p: (i32, i32, i32)) -> anyhow::Result<u32> {
        Ok(*self.blocks.get(&p).unwrap_or(&0))
    }
    fn max_build_height(&mut self) -> anyhow::Result<i32> {
        Ok(self.max)
    }
    fn min_build_height(&mut self) -> anyhow::Result<i32> {
        Ok(-64)
    }
    fn has_neighbor_signal(&mut self, p: (i32, i32, i32)) -> anyhow::Result<bool> {
        Ok((p == self.pos && self.power & 1 != 0)
            || (p == (self.pos.0, self.pos.1 + 1, self.pos.2) && self.power & 2 != 0))
    }
    fn inside_world_border(&mut self, _p: (i32, i32, i32)) -> anyhow::Result<bool> {
        Ok(self.border)
    }
    fn raw_brightness(&mut self, _p: (i32, i32, i32), _d: i32) -> anyhow::Result<i32> {
        Ok(self.brightness)
    }
}
fn d(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).unwrap())
}
fn run(line: &str) -> String {
    let f: Vec<_> = line.split('\t').collect();
    let directions = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];
    let placing = f[1].parse().unwrap();
    let current = f[2].parse().unwrap();
    let pos = (
        f[6].parse::<i32>().unwrap(),
        f[7].parse::<i32>().unwrap(),
        f[8].parse::<i32>().unwrap(),
    );
    let context = PlacementContext {
        position: pos,
        clicked_face: directions[f[3].parse::<usize>().unwrap()],
        horizontal_direction: directions[f[4].parse::<usize>().unwrap()],
        nearest_looking_direction: directions[f[5].parse::<usize>().unwrap()],
        nearest_vertical_direction: if f[5].parse::<usize>().unwrap() % 2 == 1 {
            Direction::Up
        } else {
            Direction::Down
        },
        secondary_use: f[19].parse().unwrap(),
        replacing_clicked: f[18].parse().unwrap(),
        rotation: f32::from_bits(u32::from_str_radix(f[24], 16).unwrap()),
        ordered_directions: f[25]
            .split(',')
            .map(|v| directions[v.parse::<usize>().unwrap()])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        hit_location: Vec3::new(d(f[9]), d(f[10]), d(f[11])),
    };
    let mut world = BTreeMap::new();
    world.insert(pos, current);
    for (i, dir) in directions.iter().enumerate() {
        let step = dir.step();
        world.insert(
            (pos.0 + step.0, pos.1 + step.1, pos.2 + step.2),
            f[12 + i].parse().unwrap(),
        );
    }
    if f.len() > 26 {
        let left = context.horizontal_direction.counter_clockwise().step();
        let right = context.horizontal_direction.clockwise().step();
        world.insert(
            (pos.0 + left.0, pos.1 + 1, pos.2 + left.2),
            f[26].parse().unwrap(),
        );
        world.insert(
            (pos.0 + right.0, pos.1 + 1, pos.2 + right.2),
            f[27].parse().unwrap(),
        );
    }
    let mut test_world = TestWorld {
        blocks: world.clone(),
        pos,
        power: f[20].parse().unwrap(),
        border: f[21].parse().unwrap(),
        max: f[22].parse().unwrap(),
        brightness: f[23].parse().unwrap(),
    };
    match f[0] {
        "place" => {
            match placement::placement_state_in_world(placing, &context, &mut test_world).unwrap() {
                PlacementResult::State(state) => state.to_string(),
                PlacementResult::Rejected => "rejected".into(),
                PlacementResult::Unsupported(c) => format!("unsupported:{c}"),
            }
        }
        "plan" => match placement::placement_plan(placing, &context, &mut test_world).unwrap() {
            placement::PlacementPlanResult::Rejected => "rejected".into(),
            placement::PlacementPlanResult::Unsupported(c) => format!("unsupported:{c}"),
            placement::PlacementPlanResult::Plan(plan) => {
                let relative =
                    |p: (i32, i32, i32)| format!("{},{},{}", p.0 - pos.0, p.1 - pos.1, p.2 - pos.2);
                let mut effects: Vec<String> = plan
                    .writes
                    .iter()
                    .map(|w| format!("W:{},{},{}", relative(w.position), w.state, w.flags))
                    .collect();
                effects.extend(
                    plan.extra_neighbour_notices
                        .iter()
                        .map(|p| format!("N:{},0", relative(*p))),
                );
                for update in &plan.extra_shape_updates {
                    for direction in [
                        Direction::West,
                        Direction::East,
                        Direction::North,
                        Direction::South,
                        Direction::Down,
                        Direction::Up,
                    ] {
                        let d = direction.step();
                        let p = (
                            update.position.0 + d.0,
                            update.position.1 + d.1,
                            update.position.2 + d.2,
                        );
                        effects.push(format!(
                            "S:{},{},{},{},{},{}",
                            direction.opposite() as u8,
                            relative(p),
                            update.state,
                            relative(update.position),
                            update.flags,
                            update.recursion_left
                        ));
                    }
                }
                effects.join("|")
            }
        },
        "item" => match placement::placement_for_item(placing as i32, &context, &mut test_world)
            .unwrap()
        {
            PlacementResult::State(s) => s.to_string(),
            PlacementResult::Rejected => "rejected".into(),
            PlacementResult::Unsupported(c) => format!("unsupported:{c}"),
        },
        "survive" => match placement::can_survive(placing, pos, &mut test_world).unwrap() {
            SurvivalResult::Supported(value) => value.to_string(),
            SurvivalResult::Unsupported(c) => format!("unsupported:{c}"),
        },
        "replace" => {
            placement::can_replace_with_block(current, placing, &context, f[18].parse().unwrap())
                .unwrap()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "unsupported".into())
        }
        _ => panic!("unknown operation"),
    }
}
fn main() {
    for line in io::stdin().lock().lines() {
        println!("{}", run(&line.unwrap()));
    }
}
