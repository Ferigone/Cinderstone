#!/usr/bin/env node
// Live recipe unlocks through the vanilla recipe advancements (#27), on a stock
// protocol-767 client: a fresh player gets the crafting table on its first
// tick, picks up an oak log it dropped from the creative inventory, receives
// the oak planks recipe, and after a server restart the recipe book still
// holds both. Not a claim of complete advancement or vanilla parity.
//
//   node tools/recipe_unlock_live_probe.mjs --server-binary target/debug/mc121-rust [--port 25631]
import mc from 'minecraft-protocol';
import minecraftData from 'minecraft-data';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

const args = process.argv.slice(2);
const option = (name, fallback) => { const i = args.indexOf(name); return i < 0 ? fallback : args[i + 1]; };
const binary = option('--server-binary', 'target/debug/mc121-rust');
const port = Number(option('--port', '25631'));
const md = minecraftData('1.21');
const pause = ms => new Promise(done => setTimeout(done, ms));
const report = {ok: false, scope: 'recipe advancements: tick unlock, pickup unlock, restart persistence', checks: {}};

async function startServer(world) {
  const child = spawn(binary, ['--allow-offline', '--port', String(port), '--world', world], {stdio: ['ignore', 'pipe', 'pipe']});
  let log = '';
  child.stdout.on('data', d => { log += d; });
  child.stderr.on('data', d => { log += d; });
  const deadline = Date.now() + 60000;
  while (Date.now() < deadline) {
    const ok = await new Promise(done => {
      mc.ping({host: '127.0.0.1', port, version: '1.21'}, error => done(!error));
    });
    if (ok) return {child, log: () => log};
    if (child.exitCode !== null) throw new Error(`server exited: ${log.slice(-2000)}`);
    await pause(500);
  }
  throw new Error('server did not start');
}

async function stopServer(server) {
  server.child.kill('SIGTERM');
  await new Promise(done => server.child.once('exit', done));
}

function connect(username) {
  const client = mc.createClient({host: '127.0.0.1', port, username, auth: 'offline', version: '1.21', hideErrors: true});
  const events = [];
  client.on('packet', (data, meta) => {
    if (meta.name === 'unlock_recipes') events.push({name: meta.name, action: data.action, recipes: data.recipes1, highlighted: data.recipes2 ?? []});
    else if (['collect', 'set_slot', 'window_items', 'position'].includes(meta.name)) events.push({name: meta.name, data});
  });
  client.on('position', p => {
    client.write('teleport_confirm', {teleportId: p.teleportId});
    client.write('position_look', {x: p.x, y: p.y, z: p.z, yaw: p.yaw, pitch: p.pitch, onGround: true});
  });
  return new Promise((done, fail) => {
    client.once('position', () => done({client, events}));
    client.once('error', fail);
    client.once('end', reason => fail(new Error(`disconnected: ${reason}`)));
  });
}

async function until(events, predicate, what, timeout = 15000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const found = events.findIndex(predicate);
    if (found >= 0) return found;
    await pause(50);
  }
  throw new Error(`timed out waiting for ${what}: ${JSON.stringify(events.map(e => e.name + (e.action !== undefined ? ':' + e.action : '')))}`);
}

const world = await mkdtemp(join(tmpdir(), 'recipe-unlock-'));
let server;
try {
  server = await startServer(join(world, 'world.json'));
  const name = 'RecipeProbe';
  let {client, events} = await connect(name);
  const init = await until(events, e => e.name === 'unlock_recipes' && e.action === 0, 'recipe book INIT');
  report.checks.fresh_init_empty = events[init].recipes.length === 0;
  assert.ok(report.checks.fresh_init_empty, 'a fresh player starts with an empty recipe book');

  // minecraft:recipes/decorations/crafting_table: tick criterion.
  const table = await until(events, e => e.name === 'unlock_recipes' && e.action === 1 && e.recipes.includes('minecraft:crafting_table'), 'crafting table ADD');
  report.checks.tick_unlocks_crafting_table = events[table].recipes;

  // Look straight down so the dropped stack lands at the player's feet.
  client.write('look', {yaw: 0, pitch: 90, onGround: true});
  await pause(300);
  // Drop an oak log from the creative inventory (it never enters this inventory), then pick it up.
  client.write('set_creative_slot', {slot: -1, item: {itemCount: 1, itemId: md.itemsByName.oak_log.id,
    addedComponentCount: 0, removedComponentCount: 0, components: [], removeComponents: []}});
  const collect = await until(events, e => e.name === 'collect', 'pickup of the oak log', 20000);
  const planks = await until(events, e => e.name === 'unlock_recipes' && e.action === 1 && e.recipes.includes('minecraft:oak_planks'), 'oak planks ADD');
  const slot = events.findIndex((e, i) => i > collect && e.name === 'set_slot' && e.data.item?.itemId === md.itemsByName.oak_log.id);
  report.checks.pickup_unlocks = events.filter((e, i) => i > collect && e.name === 'unlock_recipes').flatMap(e => e.recipes).sort();
  report.checks.order = {collect, set_slot: slot, recipe_add: planks};
  assert.ok(collect < planks, 'the unlock follows the pickup');
  assert.deepEqual(report.checks.pickup_unlocks, ['minecraft:charcoal', 'minecraft:oak_planks', 'minecraft:oak_wood']);
  await pause(500);
  client.end();
  await pause(1000);

  // Restart: the recipe book and the completed advancements survive.
  await stopServer(server);
  server = await startServer(join(world, 'world.json'));
  ({client, events} = await connect(name));
  const again = await until(events, e => e.name === 'unlock_recipes' && e.action === 0, 'recipe book INIT after restart');
  report.checks.restart_init = [...events[again].recipes].sort();
  for (const recipe of ['minecraft:crafting_table', 'minecraft:oak_planks', 'minecraft:oak_wood', 'minecraft:charcoal']) {
    assert.ok(events[again].recipes.includes(recipe), `${recipe} survives the restart`);
  }
  await pause(2000);
  // The log is still held: its advancements are complete, so nothing is awarded again.
  report.checks.restart_adds = events.filter(e => e.name === 'unlock_recipes' && e.action === 1).flatMap(e => e.recipes);
  assert.deepEqual(report.checks.restart_adds, [], 'completed advancements award nothing again');
  client.end();
  report.ok = true;
} catch (error) {
  report.error = String(error.stack ?? error);
  if (server) report.server_log_tail = server.log().slice(-3000);
  process.exitCode = 1;
} finally {
  if (server && server.child.exitCode === null) await stopServer(server);
  await rm(world, {recursive: true, force: true});
  console.log(JSON.stringify(report, null, 2));
}
