#!/usr/bin/env node
// An independent NBT parser verifies persisted time and untouched world data.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {gunzipSync} from 'node:zlib';
import nbt from 'prismarine-nbt';
const [original,changed]=process.argv.slice(2);
const parse=path=>nbt.parseUncompressed(gunzipSync(readFileSync(path)),'big').value.Data.value;
const before=parse(original),after=parse(changed);
const number=tag=>BigInt.asIntN(64,(BigInt(tag.value[0])<<32n)|BigInt(tag.value[1]>>>0));
assert.equal(after.DataVersion.value,3953);
assert.ok(number(after.Time)>number(before.Time),'world clock did not advance');
const daylight=before.GameRules.value.doDaylightCycle?.value!=='false';
assert.equal(number(after.DayTime)-number(before.DayTime),daylight?number(after.Time)-number(before.Time):0n);
assert.ok(number(after.LastPlayed)>0n);
for(const field of ['Time','DayTime','LastPlayed']){delete before[field];delete after[field];}
assert.deepStrictEqual(after,before,'unrelated level.dat fields changed');
console.log(JSON.stringify({ok:true,independent_decoder:'prismarine-nbt',unknown_fields_preserved:true,time_advanced:true,daylight_rule_preserved:true}));
