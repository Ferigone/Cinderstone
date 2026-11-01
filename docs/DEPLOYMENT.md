# Deploying Cinderstone

Cinderstone is a native Rust server for Minecraft Java 1.21, protocol 767.
It is a development implementation with a limited creative gameplay profile.
Deploying it does not establish complete vanilla compatibility. See
[CURRENT_WORK.md](CURRENT_WORK.md) for the supported mechanics and remaining gaps.

## Railway

Use one service connected to [Ferigone/Cinderstone](https://github.com/Ferigone/Cinderstone)
and its `master` branch. Enable GitHub autodeploys for that branch. The repository
root is the build root, and the builder is `DOCKERFILE` with `Dockerfile`.
Leave any custom start command empty so the image entrypoint runs.

Configure these service settings explicitly:

| Setting | Value |
| --- | --- |
| Replicas | 1 |
| Persistent volume mount | `/data` |
| TCP proxy target port | 25565 |
| HTTP healthcheck path | Unset |
| Restart policy | On failure, at most 10 retries |
| Deployment overlap | 0 seconds |
| Graceful shutdown / draining time | 60 seconds |

The public endpoint is the hostname and external port assigned by Railway's
TCP proxy. Enter `hostname:external-port` in a Java 1.21 client's server list.
An HTTP domain is not a Minecraft endpoint. The proxy target remains 25565
inside the container; its public port may differ.

Mount the volume before the first server start. The image's `VOLUME` declaration
does not provision a Railway volume. All of these paths belong to the same
persistent volume and must be preserved together:

- `/data/world.json`: native world changes and simulation owners.
- `/data/world.players/`: native player data and statistics.
- `/data/world.entities.native.json`: native entity/source checkpoint paired
  with the world snapshot.

Run one writer for this volume. Restoring only one file of a paired checkpoint
can be rejected as an inconsistent save. For backups, stop the server gracefully
and snapshot the entire volume. Deployments that reuse this volume can have a
short interruption while the previous process saves and releases it.

Public startup uses `--online-mode`, binds to `0.0.0.0`, and verifies player
sessions through the Mojang session service over outbound HTTPS. A valid Java
account is required. The container includes trusted CA certificates and the
OpenSSL runtime. No Java server process or JAR is included.

`railway.toml` records compatible deployment settings but does not create the
GitHub connection, TCP proxy or volume. Railway's current documentation deprecates
this older configuration format for new services; set the service settings
explicitly or manage them with Railway's current Infrastructure as Code.

## Configuration

The image entrypoint reads the following optional environment variables:

| Variable | Default | Purpose |
| --- | --- | --- |
| `MC_BIND` | `0.0.0.0` | Listening interface |
| `MC_PORT` | `25565` | Internal Minecraft TCP port |
| `MC_DATA_DIR` | `/data` | Directory containing the paired native saves |
| `MC_VIEW_DISTANCE` | `2` | Supported range 2–6 |
| `MC_DIFFICULTY` | `easy` | `peaceful`, `easy`, `normal` or `hard` |

If `MC_PORT` changes, update the Railway TCP proxy target as well. If
`MC_DATA_DIR` changes, update the persistent volume mount. The standard startup
uses the immutable `/app/fixtures/configuration.json` and sibling `join.json`.
The other generated catalogue files are included in the image and/or binary.

The server's normal shutdown path saves its world and players when it receives
SIGTERM or SIGINT. Allow the draining interval to finish; force termination
can interrupt a save. Startup and runtime failures are written to standard
output/error for Railway logs. A running process alone does not prove that
every gameplay path is supported.

## Local Docker

```sh
docker build -t cinderstone:local .
docker volume create cinderstone-data
docker run --name cinderstone --rm \
  --stop-timeout 60 \
  -p 25565:25565 \
  -v cinderstone-data:/data \
  cinderstone:local
```

Use `docker stop --time 60 cinderstone` to stop it gracefully. Connect a
Java 1.21 client to `localhost:25565`. Local startup uses the same online
authentication as Railway.

The multi-stage build pins Rust 1.99.1 on Debian Bookworm. It builds with
`cargo build --locked --release`, uses system OpenSSL during compilation, and
copies only the native executable, generated data and entrypoint into the final
image. Java analysis tools, Oracle JARs, local saves and build caches are excluded.

## References

- [Railway Dockerfiles](https://docs.railway.com/builds/dockerfiles)
- [Railway GitHub autodeploys](https://docs.railway.com/deployments/github-autodeploys)
- [Railway TCP proxy](https://docs.railway.com/networking/tcp-proxy)
- [Railway volumes](https://docs.railway.com/volumes)
- [Railway deployment teardown](https://docs.railway.com/deployments/deployment-teardown)
- [Railway configuration reference](https://docs.railway.com/config-as-code/reference)
- [Official Rust container images](https://hub.docker.com/_/rust)
