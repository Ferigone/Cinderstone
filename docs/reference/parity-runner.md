# End-to-end scenario runner (WP-Q1)

`tools/parity/run.py` runs one catalogue scenario against the unchanged Java
1.21 server and the actual native executable. It opens protocol 767 connections;
there are no injected gameplay callbacks or alternate implementations.

## Local run

Use JDK 21, Node dependencies from `npm ci`, and a built native executable.
Obtain the original using the existing pinned bootstrap (outside the checkout):

```sh
python3 tools/bootstrap_oracle.py --oracle-dir /tmp/cinderstone-oracle
cargo check --locked --all-targets
cargo build --locked --bin mc121-rust
python3 tools/parity/run.py \
  --jar /tmp/cinderstone-oracle/server-1.21.jar \
  --binary target/debug/mc121-rust \
  --eula-file /path/to/existing/eula.txt \
  --report /tmp/parity-result.json
python3 -m unittest discover -s tools/parity -p 'test_*.py'
```

The runner verifies the JAR's size and SHA-1 against `fixtures/source-lock.json`.
It requires an existing `eula=true` acceptance. It uses loopback ports 25591/25592,
creates isolated temporary directories, and retains logs/saves there for local
diagnosis. Native public-key input is an empty local keyset in offline loopback
mode; this does not test authenticated chat. It never touches Railway or `/data`.
Stop either conflicting local listener before running. Servers stop normally;
a forced shutdown makes the result fail.

## Comparison contract

`scenarios.json` is the catalogue keyed by scenario ID and checklist areas.
`creative-stone` creates Java's flat stone/dirt/grass world with seed 0 and uses
native Main's fresh flat backend with that same logical world/seed. Before
acting, both clients hash all 98,304 block states of chunk (0,0) and record the
hashed seed. Those observations must agree in the diff. These are distinct
storage formats, not a Java save-import claim. Full-world lighting, entities
and other chunks are outside this scenario.

The same client joins with an empty inventory, selects 64 stone, places one,
and breaks it. It projects initial inventory/scene plus target block updates
and digging acknowledgments into ordered per-action traces. Duplicate updates
and sequence numbers are retained. `diff.py` compares those traces exactly;
failed executions fail even if both failed identically. There is no tolerance
for reordering, dropped events or divergent state. Packet types/fields excluded
from the projection are enumerated in the catalogue and copied into each report.
Transport timing and nondeterministic ids are not compared. Java simulation
rules are controlled; native has no freeze/step hook. Actions are sequential
and wait for an acknowledgment and visible block update, then a bounded quiet
interval. This is a finite observation window, not a timing or parity proof.

Reports include the source commit, binary hash, original JAR hash, JVM version,
scene provenance, scope, both normalized traces and explicit differences.
A run exits nonzero on a difference or execution failure. Only sanitized JSON
reports belong under `docs/validation/`; never copy logs, saves, JARs, mappings,
private account data or dependency caches into Git. Extend the catalogue and
client together when another supported scenario becomes available. Every new
unexplained mismatch needs an issue before treating the scenario as conformant.

The earlier Java-import trial remains a failed observation: native Main rejected
BLOCK gameplay without native source provenance. That boundary belongs to
WP-G5 (#44); fresh native scenes do not remove it. The runner's first delivery
covers the stated creative flow only, not the final comprehensive compatibility
or performance work in Roadmap §10. #20 has no checklist row/count increment.

## First executed observation

[The report](../validation/parity-creative-stone.json) records both servers
completing join/place/break, equal chunk hashes and seed, and an ordered
placement mismatch: Java emits two target stone updates; native emits one.
The report deliberately keeps `ok: false`. Follow-up [#158](https://github.com/Ferigone/Cinderstone/issues/158)
tracks the discrepancy. The other excluded behaviors remain untested.
