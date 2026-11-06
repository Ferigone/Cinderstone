# Weather commands (#177)

Main routes player and console `weather clear|rain|thunder [duration]` through
the native Brigadier dispatcher at permission level 2. The advertised tree uses
`minecraft:time` with minimum 1; suggestions use that same dispatcher. Durations
accept Java's float rounding and `t`, `s`, `d` units. Invalid or denied input
changes no weather state and consumes no random draw.

`main_weather_command.rs` borrows the actual `WorldContext.weather` and retained
`item_runtime` level RNG. With no duration, the original inclusive ranges are
12000–180000 ticks (clear), 12000–24000 (rain), and 3600–15600 (thunder). The
command returns the input duration, including **-1** when it samples a default.
It changes timers/flags only: the existing weather tick owns interpolation and
ordered client packets. `doWeatherCycle=false` prevents natural timer changes
without disabling the command. Existing world/entity checkpoint owners save it.

## Evidence

`tools/weather_command_oracle.java` executes the unchanged 1.21 `WeatherCommand`
registration and setters, with recording `ServerLevelData` setters and a seeded
original `LegacyRandomSource`. The small fixture checks permission trees 0–4,
parse/completion, timers/flags, command results and the next RNG draw. It does
not pretend to tick a complete `ServerLevel`; the real protocol flow does that.
Regenerate using `tools/weather_command_vectors.py --named-jar ...
--original-jar ... --libraries ... --out fixtures/weather-command-vectors.json`.
Prepare the named original with the existing pinned bootstrap and
`tools/loot_remap.java`; keep all Java artifacts outside the checkout.

Run `node tools/weather_command_flow.mjs BINARY PINNED_JAR EXISTING_EULA REPORT`
from the repository after `npm ci`. It uses isolated temporary server/admin
directories and ports 25641/25642; it never accepts the EULA or touches `/data`.
The Java scene is prepared with ordinary `forceload` and `save-all flush` so the
imported view has real saved chunks. Normal shutdown is required.

[The executed report](../validation/weather-command-runtime.json) retains and
compares all 410 ordered weather transition packets per server, with no
deduplication, plus successful feedback bytes, command graph and suggestions.
It checks non-op/invalid refusal, console execution, `doWeatherCycle=false`,
saved imported timers and cold restart. A separate fresh-native run checks an
omitted-duration rain command and native checkpoint/restart. Each live server's
default RNG is its own; deterministic consumption is checked in the class vectors.

This delivers the single-overworld weather command, not lightning, snow, ice,
cauldron precipitation, general imported gameplay, multi-level administration,
external command sources or the complete WP-E3 catalog. Checklist scope/counts
stay unchanged. Hosted deployment/TCP verification is tracked under #19.
