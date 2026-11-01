# Exact Java 1.21 GameRules NBT contract

The official inner server JAR SHA256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
`world_rules_oracle.java` executes its unchanged `GameRules` (`dcs`) constructors,
private `loadFromTag`, BooleanValue/IntegerValue deserialization and `createTag`.
Every supplied input is decoded by unchanged `NbtIo.readAnyTag` using NbtOps.

The actual public constructors are `GameRules()` and
`GameRules(DynamicLike<?>)`. The default constructor builds all values from the
static registered types map; there is no FeatureFlagSet constructor or feature
filter in this exact version. Custom datapack/feature keys supplied inside the
GameRules tag are unknown keys and are ignored. Full datapack and feature-registry
loading is a separate concern.

| Rule | Original default |
|---|---:|
| spawnChunkRadius | 2 |
| spectatorsGenerateChunks | true |
| doWeatherCycle | true |
| randomTickSpeed | 3 |
| doMobSpawning | true |

The native module also preserves the other 46 registered rules and their actual
defaults. `tools/world_rules_facts.json` is extracted from the original classes.

`GameRules.loadFromTag` loops known keys. Its lambda calls
`dynamic.get(key).asString().ifSuccess(value::deserialize)`. NbtOps.getStringValue
accepts only an actual StringTag. Therefore:

* Missing or wrong-type entries preserve the current rule value. Constructors
  begin at defaults, whereas later loads preserve previously changed values.
* A non-compound whole input supplies no rule entries, leaving defaults/current
  values intact. `WorldRules::read` accepts the GameRules tag itself, not outer
  level.dat Data.
* A StringTag containing isolated UTF-16 surrogates remains a StringTag and is
  deserialized; it must not be mistaken for a wrong-type/missing entry.
* Duplicate compound names have their final value, matching original NbtIo's
  CompoundTag insertion. Earlier duplicate strings do not independently load.

BooleanValue.deserialize delegates to Boolean.parseBoolean. Only an exact
case-insensitive `true` succeeds; there is no whitespace trimming. The oracle
exhaustively checks all 65,536 UTF-16 units for aliases of each of the four target
letters. Their aliases are precisely the ASCII uppercase and lowercase pairs.

IntegerValue.deserialize delegates to safeParse. Empty or invalid text and int
overflow produce zero. It calls Integer.parseInt directly, with no command-bound
clamp: spawnChunkRadius accepts negative integers and values above command max32
when loaded from NBT. Signs must be ASCII plus/minus; whitespace is not trimmed.
Leading zeros are valid. Integer.parseInt works on UTF-16 units and
Character.digit(char,10), accepting BMP decimal scripts, including Arabic-Indic
and fullwidth digits. Supplementary decimal code points are surrogate pairs and
are rejected by this char-based parser. The oracle enumerates all 65,536 units;
37 contiguous decimal-digit ranges represent the actual JDK21 behavior exactly.

The Java safeParse method also logs a WARN for nonempty invalid integer strings;
empty strings do not log it. The native value parser does not emit that logger
side effect. The conformance comparison covers values and canonical logical NBT,
not diagnostic logs.

`GameRules.createTag` produces every registered rule as a canonical StringTag:
`true`/`false` or signed decimal int text, and omits unknown keys. The native
`create_tag()` matches those logical contents. Serialized Java CompoundTag
HashMap key iteration/byte order is outside this proof; a lossless level.dat
host can merge these known keys into its retained document.

The fixture covers constructor and later-load behavior, all tag kinds, raw
UTF-16 strings, decimal scripts/int extrema and overflow, boolean case variants,
long leading zeros, duplicate names, unknown keys and feature/datapack-shaped
unknown values. Commands, command-bound validation, change callbacks and actual
feature-registry loading are not implemented by this module.
