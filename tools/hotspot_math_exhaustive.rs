//! Validation executable; not a runtime fallback or server dependency.
#[path = "../src/hotspot_math.rs"]
mod hotspot_math;
use std::io::{self, Write};
fn main() -> io::Result<()> {
    let mut out=io::BufWriter::with_capacity(1<<20,io::stdout().lock());
    for i in 0..(1u32<<24){let random=(i as f32)*f32::from_bits(0x33800000);let angle=f64::from(random*std::f32::consts::TAU);let(sin,cos)=hotspot_math::sin_cos(angle);out.write_all(&sin.to_bits().to_be_bytes())?;out.write_all(&cos.to_bits().to_be_bytes())?;}
    out.flush()
}
