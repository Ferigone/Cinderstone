use anyhow::{anyhow, Result};
use mc121_rust::density::{ChunkDensity, DensityGraph, DensityPoint, NodeId, NoiseGrid};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{self, BufRead},
};
fn text<'a>(q: &'a Value, key: &str) -> Result<&'a str> {
    q[key]
        .as_str()
        .ok_or_else(|| anyhow!("missing string {key}"))
}
fn bounds(graph: &DensityGraph, id: NodeId) -> Value {
    let bounds = graph.bounds(id);
    json!([
        bounds.min.to_bits().to_string(),
        bounds.max.to_bits().to_string()
    ])
}
fn points(q: &Value) -> Result<Vec<DensityPoint>> {
    q["points"]
        .as_array()
        .ok_or_else(|| anyhow!("missing points"))?
        .iter()
        .map(|p| {
            Ok(DensityPoint::new(
                p[0].as_i64().ok_or_else(|| anyhow!("invalid x"))? as i32,
                p[1].as_i64().ok_or_else(|| anyhow!("invalid y"))? as i32,
                p[2].as_i64().ok_or_else(|| anyhow!("invalid z"))? as i32,
            ))
        })
        .collect()
}
fn run(
    graph: &mut Option<DensityGraph>,
    ids: &mut HashMap<String, NodeId>,
    chunk: &mut Option<ChunkDensity>,
    q: &Value,
) -> Result<Value> {
    match text(q, "op")? {
        "chunk.new" => {
            let data = &q["grid"];
            let integer = |key: &str| {
                data[key]
                    .as_i64()
                    .ok_or_else(|| anyhow!("missing grid field"))
            };
            let grid = NoiseGrid {
                min_y: integer("min_y")? as i32,
                height: integer("height")? as i32,
                cell_width: integer("cell_width")? as i32,
                cell_height: integer("cell_height")? as i32,
            };
            let mut sampler = ChunkDensity::new(
                grid,
                q["x"].as_i64().ok_or_else(|| anyhow!("missing x"))? as i32,
                q["z"].as_i64().ok_or_else(|| anyhow!("missing z"))? as i32,
                q["count"]
                    .as_u64()
                    .ok_or_else(|| anyhow!("missing count"))? as usize,
            )?;
            if q["router"].as_bool().unwrap_or(false) {
                let graph = graph.as_ref().ok_or_else(|| anyhow!("missing world"))?;
                sampler.wrap_router(graph)?;
                for (name, id) in graph.roots() {
                    ids.insert(name.clone(), *id);
                }
                if let Some(id) = graph.block_density() {
                    ids.insert("block_density".into(), id);
                }
            }
            *chunk = Some(sampler);
            Ok(Value::Null)
        }
        "chunk.wrap" => {
            let graph = graph.as_ref().ok_or_else(|| anyhow!("missing world"))?;
            let sampler = chunk.as_mut().ok_or_else(|| anyhow!("missing chunk"))?;
            sampler.wrap(
                graph,
                *ids.get(text(q, "id")?)
                    .ok_or_else(|| anyhow!("missing node"))?,
            )?;
            Ok(Value::Null)
        }
        "chunk.state" => {
            let state = chunk
                .as_ref()
                .ok_or_else(|| anyhow!("missing chunk"))?
                .state();
            Ok(
                json!({"y":state.interpolating,"z":state.selecting_cell,"G":state.interpolation_counter,"H":state.array_counter,"I":state.array_index,"A":state.base_x,"B":state.base_y,"C":state.base_z,"D":state.in_x,"E":state.in_y,"F":state.in_z}),
            )
        }
        "chunk.start" | "chunk.advance" | "chunk.select" | "chunk.stop" | "chunk.swap"
        | "chunk.index" | "chunk.x" | "chunk.y" | "chunk.z" => {
            let graph = graph.as_ref().ok_or_else(|| anyhow!("missing world"))?;
            let sampler = chunk.as_mut().ok_or_else(|| anyhow!("missing chunk"))?;
            let integer = |key: &str| {
                q[key]
                    .as_i64()
                    .ok_or_else(|| anyhow!("missing field {key}"))
            };
            match text(q, "op")? {
                "chunk.start" => sampler.initialize_first_cell_x(graph)?,
                "chunk.advance" => sampler.advance_cell_x(graph, integer("index")? as i32)?,
                "chunk.select" => {
                    sampler.select_cell_yz(graph, integer("y")? as usize, integer("z")? as usize)?
                }
                "chunk.stop" => sampler.stop_interpolation()?,
                "chunk.swap" => sampler.swap_slices(),
                "chunk.index" => {
                    sampler.for_cell_index(integer("index")? as usize);
                }
                update => {
                    let block = integer("block")? as i32;
                    let fraction = q["fraction"]
                        .as_f64()
                        .ok_or_else(|| anyhow!("missing fraction"))?;
                    match update {
                        "chunk.x" => sampler.update_for_x(block, fraction),
                        "chunk.y" => sampler.update_for_y(block, fraction),
                        _ => sampler.update_for_z(block, fraction),
                    }
                }
            }
            Ok(Value::Null)
        }
        "chunk.sample" | "chunk.external" | "chunk.fill" | "chunk.column" | "chunk.cell" => {
            let graph = graph.as_ref().ok_or_else(|| anyhow!("missing world"))?;
            let sampler = chunk.as_mut().ok_or_else(|| anyhow!("missing chunk"))?;
            let id = *ids
                .get(text(q, "id")?)
                .ok_or_else(|| anyhow!("missing node"))?;
            match text(q, "op")? {
                "chunk.sample" => Ok(json!(sampler.sample(graph, id)?.to_bits().to_string())),
                "chunk.external" => {
                    let point = &q["point"];
                    let p = DensityPoint::new(
                        point[0].as_i64().ok_or_else(|| anyhow!("bad point"))? as i32,
                        point[1].as_i64().ok_or_else(|| anyhow!("bad point"))? as i32,
                        point[2].as_i64().ok_or_else(|| anyhow!("bad point"))? as i32,
                    );
                    Ok(json!(sampler
                        .sample_external(graph, id, p)?
                        .to_bits()
                        .to_string()))
                }
                fill => {
                    let values = match fill {
                        "chunk.fill" => sampler.fill_external(graph, id, &points(q)?)?,
                        "chunk.column" => sampler.fill_column(graph, id)?,
                        _ => sampler.fill_cell(graph, id)?,
                    };
                    Ok(json!(values
                        .iter()
                        .map(|value| value.to_bits().to_string())
                        .collect::<Vec<_>>()))
                }
            }
        }
        "world" => {
            *graph = Some(
                DensityGraph::builtin(
                    q["seed"].as_i64().ok_or_else(|| anyhow!("missing seed"))?,
                    text(q, "settings")?,
                )
                .map_err(|error| anyhow!(error.to_string()))?,
            );
            ids.clear();
            Ok(Value::Null)
        }
        "load" => {
            let graph = graph.as_mut().ok_or_else(|| anyhow!("missing world"))?;
            let id = graph
                .parse(&q["function"])
                .map_err(|error| anyhow!(error.to_string()))?;
            ids.insert(text(q, "id")?.to_owned(), id);
            Ok(bounds(graph, id))
        }
        "router" => {
            let graph = graph.as_ref().ok_or_else(|| anyhow!("missing world"))?;
            let result = graph
                .roots()
                .iter()
                .map(|(key, id)| {
                    ids.insert(key.clone(), *id);
                    (key.clone(), bounds(graph, *id))
                })
                .collect::<serde_json::Map<_, _>>();
            Ok(Value::Object(result))
        }
        "climate" => {
            let graph = graph.as_ref().ok_or_else(|| anyhow!("missing world"))?;
            let values = points(q)?
                .iter()
                .map(|point| {
                    graph
                        .sample_climate(point.x, point.y, point.z)
                        .map(|value| {
                            value
                                .parameter_array()
                                .iter()
                                .map(i64::to_string)
                                .collect::<Vec<_>>()
                        })
                        .map_err(|error| anyhow!(error.to_string()))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(json!(values))
        }
        "eval" | "fill" => {
            let graph = graph.as_ref().ok_or_else(|| anyhow!("missing world"))?;
            let id = ids
                .get(text(q, "id")?)
                .ok_or_else(|| anyhow!("missing node"))?;
            let points = points(q)?;
            let values = if text(q, "op")? == "fill" {
                graph.fill_many(*id, &points)
            } else {
                graph.eval_many(*id, &points)
            };
            Ok(json!(values
                .iter()
                .map(|value| value.to_bits().to_string())
                .collect::<Vec<_>>()))
        }
        other => Err(anyhow!("unknown operation {other}")),
    }
}
fn main() -> Result<()> {
    let mut graph = None;
    let mut ids = HashMap::new();
    let mut chunk = None;
    for line in io::stdin().lock().lines() {
        let q = serde_json::from_str(&line?)?;
        let result = match run(&mut graph, &mut ids, &mut chunk, &q) {
            Ok(value) => value,
            Err(error) => json!({"error":error.to_string()}),
        };
        println!("{}", serde_json::to_string(&result)?);
    }
    Ok(())
}
