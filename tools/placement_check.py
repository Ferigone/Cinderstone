#!/usr/bin/env python3
import hashlib,json,os,random,struct,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JAVA=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');JAR=ORACLE/'server-inner-1.21.jar';SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def bits(n):return struct.pack('>d',n).hex()
def fbits(n):return struct.pack('>f',n).hex()
def main():
 assert hashlib.sha256(JAR.read_bytes()).hexdigest()==SHA
 catalog=json.loads((ROOT/'tools/placement_catalog.json').read_text());items=json.loads((ROOT/'fixtures/items-1.21.json').read_text());r=random.Random(0x121767AB);commands=[]
 supported=set('dfy dml djy dgy dnc dnt det dgs djh dkq dnw dip dey dmy dkn dfe did dhh dlj dsv dpd dms dmt djr dhl dfn dnj dki dgn dmw dll dor diw djz dox die dom dix dfr dfd dnu dnb dff dku dfi dhy dgl dhc dex dio dni djv dow doz dpa dfj dpb dke dis dkf dmc dhr dkt dmx djp dou dhk dic div dig dof dia dfs din djt dka dgm dhb dlh dgx doy dhf dfu dme doq dop dhe dln dfc dfv dmo dov dky dlq'.split())
 def state(name,**overrides):
  b=items['blocks'][name];offset=0
  for p in b['properties']:
   value=overrides.get(p['name']);values=p.get('values')
   if value is None:raise ValueError((name,p))
   if p['type']=='bool':index=0 if value else 1
   elif values is not None:index=values.index(str(value))if str(value)in values else values.index(value)
   else:index=value
   offset=offset*p['num_values']+index
  return b['min_state']+offset
 def command(kind,placing,current,face=None,horizontal=None,nearest=None,yclick=None,neighbors=None,replacing=True,secondary=False,power=0,border=True,max_height=320,brightness=15,upper_sides=None):
  x,y,z=r.choice([-30000000,-1,0,30000000]),r.randrange(-64,320),r.choice([-30000000,-1,0,30000000]);face=r.randrange(6)if face is None else face;horizontal=r.randrange(2,6)if horizontal is None else horizontal;nearest=r.randrange(6)if nearest is None else nearest;yclick=r.choice([0.,.5,.50000001,1.])if yclick is None else yclick
  commands.append(kind+'\t'+'\t'.join(map(str,[placing,current,face,horizontal,nearest,x,y,z,bits(x+.5),bits(y+yclick),bits(z+.5),*(neighbors or [0]*6),str(replacing).lower(),str(secondary).lower(),power,str(border).lower(),max_height,brightness,fbits(r.uniform(-36000,36000)),','.join(map(str,r.sample(range(6),6))),*(upper_sides or [0,0])])))
 water=[i for i,v in enumerate(catalog['fluids'])if v&1];flowing=[i for i,v in enumerate(catalog['fluids'])if v&2 and not v&1]
 for b in catalog['blocks']:
  if b['placement_class']in supported:
   for i in range(12):command('place',b['default'],[0,r.choice(water),r.choice(flowing)][i%3])
  if b['replacement_class']=='dtb':
   for _ in range(3):command('replace',r.choice(catalog['blocks'])['default'],b['default'])
   command('replace',b['default'],b['default'])
  if b['placement_class']=='dnc':
   name=b['name'].removeprefix('minecraft:')
   for slab_type in ['top','bottom','double']:
    for wet in [False,True]:
     current=state(name,type=slab_type,waterlogged=wet)
     command('place',b['default'],current)
     for face in range(6):
      for hit in [.5,.500000001]:command('replace',b['default'],current,face=face,yclick=hit)
     command('replace',b['default'],current,replacing=False)

 for b in catalog['blocks']:
  if b['replacement_class']in ['dgn','dmw','dll','dor']:
   for current in range(b['min'],b['max']+1):
    for secondary in [False,True]:
     command('replace',b['default'],current,secondary=secondary)
    command('place',b['default'],current)
 for _ in range(1500):
  b=r.choice([b for b in catalog['blocks']if b['placement_class']=='dki'])
  logs=json.loads((ROOT/'fixtures/placement-block-tags-1.21.json').read_text())['tags']['minecraft:logs']
  leaf_neighbors=[r.choice([0,r.choice(logs),r.choice([s for leaf in catalog['blocks']if leaf['placement_class']=='dki'for s in range(leaf['min'],leaf['max']+1)])])for _ in range(6)]
  command('place',b['default'],r.choice([0,*water[:8]]),neighbors=leaf_neighbors)

 connection_blocks=[b for b in catalog['blocks']if b['placement_class']in ['diw','djz']]
 static_states=[s for b in catalog['blocks']if b['class']not in ['dmy','dsu']for s in range(b['min'],b['max']+1)]
 for _ in range(4000):
  b=r.choice(connection_blocks);command('place',b['default'],r.choice([0,*water[:8]]),neighbors=[r.choice(static_states)for _ in range(6)])

 walls=[b for b in catalog['blocks']if b['placement_class']=='dox']
 dynamic={int(row.split(':',1)[0])for row in json.loads((ROOT/'evidence/geometry-collision-catalog.json').read_text())['context_dependent']}
 static_above=[s for s in static_states if s not in dynamic]
 for _ in range(4000):
  b=r.choice(walls);neighbors=[r.choice(static_states),r.choice(static_above),*[r.choice(static_states)for _ in range(4)]]
  command('place',b['default'],r.choice([0,*water[:8]]),neighbors=neighbors)

 survival_classes=set('dtb dfd dgo dnu dgk dgp dfi dfj dke dpb dex dow doz dfo dgn dfm dfn dia dfb dfc djp dnk die dic div dis dkf dnp dgh dht dig dmw djk dgx dhf dfu dov dky dlq dme doq'.split())
 survival_bush={'dgh','dez','dnv','dlf','dhx','dpr','dje','dmk','dld','dfa','dkt','dmx','dpc','dmw','dht'}
 survival_blocks=[b for b in catalog['blocks']if b['survival_class']in survival_classes and(b['survival_class']!='dgh'or b['may_place_class']in survival_bush)]
 for b in survival_blocks:
  for _ in range(6):command('survive',r.randrange(b['min'],b['max']+1),0,neighbors=[r.choice(static_states)for _ in range(6)],brightness=r.randrange(16))
 soil_names=['air','stone','dirt','grass_block','farmland','coarse_dirt','rooted_dirt','podzol','mycelium','clay','moss_block','mud','sand','red_sand','gravel','netherrack','soul_sand','soul_soil','crimson_nylium','warped_nylium','magma_block','ice','frosted_ice','packed_ice','blue_ice','water','lava']
 for b in survival_blocks:
  if b['survival_class'] in ['dgh','dig','dmw']:
   for name in soil_names:
    for current in [0,*water[:1]]:
     neighbors=[items['blocks'][name]['default_state'],0,0,0,0,0]
     command('survive',b['default'],current,neighbors=neighbors,brightness=15)
 extended=[b for b in catalog['blocks']if b['placement_class']in ['die','dom','dix','dfr']]
 for _ in range(4000):
  b=r.choice(extended);neighbors=[r.choice(static_states)for _ in range(6)]
  if r.random()<.6:neighbors=[0]*6
  command('place',b['default'],r.choice([0,*water[:8]]),neighbors=neighbors,replacing=r.choice([True,False]),power=r.randrange(4),border=r.choice([True,False]),upper_sides=[r.choice(static_states)for _ in range(2)])

 for b in catalog['blocks']:
  if b['replacement_class']in ['dji','djm','dln','dmo','dmv','dni','dov']:
   states=list(range(b['min'],b['max']+1));r.shuffle(states)
   for existing in states[:64]:
    for placing in [b['default'],r.choice(catalog['blocks'])['default']]:
     for replacing in [True,False]:command('replace',placing,existing,replacing=replacing)

 attach=[b for b in catalog['blocks']if b['placement_class']in ['dow','doz','dpa','dfj','dpb','dke','dis','dkf']]
 for _ in range(3000):
  b=r.choice(attach);command('place',b['default'],r.choice([0,*water[:8]]),neighbors=[r.choice(static_states)for _ in range(6)],replacing=r.choice([True,False]))
 stairs=[b for b in catalog['blocks']if b['placement_class']=='dnt']
 for _ in range(3000):
  b=r.choice(stairs);neighbors=[0,0]
  for i in range(4):
   if r.random()<.85:nb=r.choice(stairs);neighbors.append(state(nb['name'].removeprefix('minecraft:'),facing=r.choice(['north','south','west','east']),half=r.choice(['top','bottom']),shape=r.choice(['straight','inner_left','inner_right','outer_left','outer_right']),waterlogged=r.choice([True,False])))
   else:neighbors.append(0)
  command('place',b['default'],r.choice([0,*water[:8]]),neighbors=neighbors)
 by_state={b['default']:b for b in catalog['blocks']}
 for item in catalog['items']:
  b=by_state[item['primary_state']]
  if b['placement_class']in supported and b in survival_blocks and item['context_class']=='cso':
   for _ in range(4):command('item',item['id'],r.choice([0,*water[:8]]),neighbors=[r.choice(static_above)for _ in range(6)])
 interesting_names=['air','stone','chest','trapped_chest','oak_hanging_sign','oak_wall_hanging_sign','oak_sign','oak_wall_sign','hay_block','skeleton_skull','wither_skeleton_skull','creeper_head','zombie_head','dragon_head','piglin_head','player_head','jungle_log','stripped_jungle_log','mangrove_leaves','oak_leaves','tripwire','tripwire_hook','chorus_flower','chorus_plant','end_stone','bamboo_sapling','bamboo','scaffolding','big_dripleaf','big_dripleaf_stem','vine','glow_lichen','sculk_vein','pointed_dripstone']
 interesting=[s for name in interesting_names for s in range(items['blocks'][name]['min_state'],items['blocks'][name]['max_state']+1)]
 for _ in range(4500):
  b=r.choice([b for b in catalog['blocks']if b['placement_class']in ['dia','dfs','din','djt','dka','dgm','dhb','dlh','dgx','doy','dhf','dfu','dme','doq','dop','dhe','dln','dfc','dfv','dmo','dov','dky','dlq']])
  command('place',b['default'],r.choice([0,*water[:8]]),neighbors=[r.choice(interesting)for _ in range(6)],secondary=r.choice([True,False]),replacing=r.choice([True,False]))
 for b in catalog['blocks']:
  if b['placed_by_class']in ['die','dig','dfr'] and b['item']!=0 and b in survival_blocks:
   for _ in range(16):
    neighbors=[items['blocks']['dirt']['default_state'],0,0,0,0,0]
    if b['placement_class']=='die':neighbors[0]=items['blocks']['stone']['default_state']
    command('plan',b['default'],0,neighbors=neighbors,power=r.randrange(4),border=True)
 transcript='\n'.join(commands)+'\n';classpath=os.pathsep.join([str(JAR),*map(str,sorted((ORACLE/'libraries').rglob('*.jar')))])
 probe=ROOT/'target/debug/placement_probe'
 with tempfile.TemporaryDirectory(prefix='mc121-placement-check-')as temp:
  subprocess.run([str(JAVA/'javac'),'-proc:none','-cp',classpath,'-d',temp,str(ROOT/'tools/placement_world.java'),str(ROOT/'tools/placement_Oracle.java'),str(ROOT/'tools/placement_Launcher.java'),str(ROOT/'tools/placement_tags.java')],check=True,capture_output=True)
  j=subprocess.run([str(JAVA/'java'),'-cp',temp+os.pathsep+classpath,'oracle.PlacementLauncher',temp,str(ROOT/'fixtures/placement-block-tags-1.21.json')],input=transcript,text=True,capture_output=True)
  if j.returncode:raise RuntimeError(j.stderr)
  assert 'ORACLE_CLASS=dfy,dml,dnt,dnc,BlockState;SOURCE='in j.stderr and JAR.name in j.stderr
  assert probe.is_file(),"Build cargo --bin placement_probe first"
  assert probe.stat().st_mtime >= max((ROOT/p).stat().st_mtime for p in ["src/placement.rs","tools/placement_probe.rs","fixtures/placement-1.21.json"]),"Rebuild stale placement_probe"
  rust=subprocess.run([str(probe)],input=transcript,text=True,capture_output=True)
  if rust.returncode:raise RuntimeError(rust.stderr)
 expected,actual=j.stdout.splitlines(),rust.stdout.splitlines();assert len(expected)==len(actual)==len(commands)
 mismatch=[{'line':i+1,'command':commands[i],'java':a,'rust':b}for i,(a,b)in enumerate(zip(expected,actual))if a!=b]
 report={'success':not mismatch,'oracle':'unchanged original Minecraft1.21 block getStateForPlacement + canBeReplaced with controlled world and context','jar_sha256':SHA,'compared_outputs':len(commands),'implemented_block_functions':sum(b['placement_class']in supported for b in catalog['blocks']),'total_blocks':1060,'covered':['exact default/pillar/facing state functions','slab top/bottom/merge andwaterlogging','stair inner/outer corners withmixedmaterials/halves andsideconstraints','source vsflowing fluid behavior','leaf distance fromsixneighbours+officiallogtags','stackedcandles/petals/pickles/eggs includingsecondaryuse andlimits','railaxis andsnowyaboveblocktag','fence/pane/bar connections includingface-support/connection-exceptions, gateaxis, walltags andwood/netherfencetypes','wall heights/post fromabove collision-face coverage andwallpostoverridetags','canSurvive originalsupportfamilies/full-center-rigidshapes','doorhinges/upperclearance/buildheight/redstone/trapdoors/gates/bedheadborder','rotation16signs/banners/skulls plusprioritywall/faceattached/lanternplacement','generic970blocks replacement plusall60slab replacements','worldcoordinates±30,000,000 andcursor halfboundaries'],'not_covered':['block-specific functions notlisted inimplementation','canSurvive andactual placement-worldeffects','full neighbourupdate propagation andblockentity placementdata'],'trace_sha256':hashlib.sha256(j.stdout.encode()).hexdigest(),'mismatches':mismatch[:10]}
 (ROOT/'evidence/placement-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2));return int(bool(mismatch))
if __name__=='__main__':raise SystemExit(main())
