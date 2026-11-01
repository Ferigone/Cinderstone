//! Apply repeated native edits before independent vanilla packet comparison.
//! The checker supplies a fresh disposable copy, never the oracle world.
use mc121_rust::storage::VanillaWorld;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(args.len(), 2, "heightmap_world_probe WORLD_COPY");
    let mut world = VanillaWorld::open(&args[1]).expect("open disposable world");
    assert_eq!(world.block(1, 63, 1).unwrap(), 9);
    assert_eq!(world.block(-1, 64, -1).unwrap(), 0);
    // The first edit primes each imported chunk; its two following edits must
    // advance and lower the same column through the incremental path. Final
    // states match two vanilla setblock commands used by storage_capture.py.
    for (x, y, z, state) in [
        (1, 63, 1, 0),
        (1, 63, 1, 1),
        (1, 63, 1, 0),
        (-1, 64, -1, 1),
        (-1, 64, -1, 0),
        (-1, 64, -1, 1),
    ] {
        assert!(world.set_block(x, y, z, state).unwrap());
    }
    assert_eq!(world.save().unwrap(), 2);
    println!(
        "{{\"ok\":true,\"edits\":6,\"chunks\":2,\"scope\":\"native prime plus repeated incremental edits saved before vanilla comparison\"}}"
    );
}
