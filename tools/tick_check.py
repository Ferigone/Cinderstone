#!/usr/bin/env python3
"""Differentially compare tick primitives with the actual signed 1.21 classes."""
import argparse,hashlib,json,os,random,subprocess,tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
EXPECTED_JAR='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'

def main():
    p=argparse.ArgumentParser()
    p.add_argument('--oracle-dir',type=Path,default=ROOT.parent/'mc121-oracle')
    p.add_argument('--java-home',type=Path,default=Path(os.environ.get('JAVA_HOME','/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1')))
    p.add_argument('--probe',type=Path,default=ROOT/'target/debug/tick_probe')
    p.add_argument('--rounds',type=int,default=100)
    args=p.parse_args()
    if args.rounds<1:p.error('--rounds must be positive')
    jar=args.oracle_dir/'server-inner-1.21.jar'
    if hashlib.sha256(jar.read_bytes()).hexdigest()!=EXPECTED_JAR:raise RuntimeError('Not the pinned official 1.21 inner server')
    rng=random.Random(0x121767)
    commands=['priority\t'+str(n) for n in [-(1<<31),-10,-4,-3,-2,-1,0,1,2,3,4,10,(1<<31)-1]]
    def tick(kind,x,y,z,t,pr,order):return ','.join(map(str,(kind,x,y,z,t,pr,order)))
    for _ in range(args.rounds):
        commands.append('new')
        scheduled=[]
        for i in range(40):
            f=(f'k{rng.randrange(4)}',rng.randrange(-3,4),rng.randrange(-2,3),rng.randrange(-3,4),rng.randrange(-10,11),rng.randrange(-5,6),rng.randrange(-4,5))
            scheduled.append(f)
            commands.append('schedule\t'+'\t'.join(map(str,f)))
            if i%7==0:
                dupe=f[:4]+(f[4]-100,-3,-100)
                commands.append('schedule\t'+'\t'.join(map(str,dupe)))
            if i%9==0:commands.extend(['peek','count','save\t'+str(rng.randrange(-10,11))])
        for _ in range(12):
            a,b=rng.sample(scheduled,2)
            for mode in ['drain','intra']:commands.append('cmp\t'+mode+'\t'+tick(*a)+'\t'+tick(*b))
        commands.extend(['poll']*45)
        # Pending entries are protected before unpack; include duplicate saved
        # keys, equal priorities, negative delays and Java narrowing boundaries.
        pending=[('k0',-1,64,-17,-2,0),('k1',0,-64,0,0,-3),('k0',-1,64,-17,5,2),('k2',16,319,16,(1<<31)-1,3)]
        commands.extend(['load\t'+';'.join(','.join(map(str,f))for f in pending),'count','peek','has\tk0\t-1\t64\t-17','schedule\tk0\t-1\t64\t-17\t0\t-3\t0','save\t999','unpack\t'+str(rng.choice([0,-5,1<<40,(1<<63)-1])),'save\t-1','peek','poll','has\tk0\t-1\t64\t-17','poll','poll','poll','poll','count'])
    for game in [0,1,-1,-(1<<63),(1<<63)-1]:
        for day in [0,1,-1,24000,-(1<<63),(1<<63)-1]:
            for cycle in ['false','true']:commands.append(f'time\t{game}\t{day}\t{cycle}')
    for case in range(max(8,args.rounds//5)):
        commands.append('level.new')
        chunks=[(x,z)for x in range(-3,4)for z in range(-3,4)]
        for x,z in chunks:commands.append(f'level.add\t{x}\t{z}')
        for i in range(180):
            x,z=rng.choice(chunks)
            f=(f'k{rng.randrange(4)}',x*16+rng.randrange(16),rng.randrange(-2,3),z*16+rng.randrange(16),rng.randrange(-5,16),rng.randrange(-3,4),rng.randrange(-4,5))
            commands.append('level.schedule\t'+'\t'.join(map(str,f)))
            if i%9==0:commands.append('level.order')
        blocked=rng.sample(chunks,8)
        for x,z in blocked:commands.append(f'level.block\t{x}\t{z}\ttrue')
        for now,limit,mode in [(0,0,'plain'),(0,7,'plain'),(5,13,'reschedule'),(5,8,'plain')]:
            commands.extend([f'level.tick\t{now}\t{limit}\t{mode}','level.order','level.count'])
        commands.extend(['level.clear\t-20\t-1\t-20\t24\t1\t24','level.order',
                         'level.copy\t-16\t-2\t-16\t15\t2\t15\t16\t0\t16','level.order',
                         'level.remove\t0\t0','level.order','level.tick\t100\t4096\tplain','level.order','level.count'])
        for x,z in blocked:commands.append(f'level.block\t{x}\t{z}\tfalse')
        commands.extend(['level.tick\t100\t4096\tplain','level.order','level.count'])
        # Completely equal comparators stress the fastutil map / Java heap tie.
        commands.append('level.new')
        for x,z in chunks:
            commands.extend([f'level.add\t{x}\t{z}',f'level.schedule\ttie\t{x*16}\t0\t{z*16}\t0\t0\t0'])
        commands.extend(['level.order','level.tick\t0\t17\tplain','level.order','level.tick\t0\t100\tplain','level.order'])
    for mode in ['query','clear','copy']:
        commands.append('level.new')
        for x in range(-2,3):
            for z in range(-2,3):commands.append(f'level.add\t{x}\t{z}')
        for kind,x,y,z,time,priority,order in [('first',-1,0,-1,0,-3,0),('other',20,0,20,0,0,1),('inside',3,0,3,0,1,2),('watch',0,0,0,0,3,3)]:
            commands.append('level.schedule\t'+'\t'.join(map(str,(kind,x,y,z,time,priority,order))))
        commands.extend([f'level.tick\t0\t100\t{mode}','level.order','level.count','level.tick\t0\t100\tquery','level.order','level.count'])
    loaded='keep,-1,64,-1,-2,0;wrong_x,0,64,-1,0,-3;wrong_z,-1,64,0,0,-3;other_chunk,-17,64,-1,0,0;keep2,-16,0,-16,3,2'
    commands.extend(['load_chunk\t-1\t-1\t'+loaded,'count','save\t0','unpack\t10','poll','poll','poll',
                     'level.new','level.pending\t-1\t-1\t'+loaded,'level.count','level.order',
                     'level.tick\t100\t100\tplain','level.unpack\t-1\t-1\t10','level.order',
                     'level.tick\t100\t100\tplain','level.order','level.count'])
    # Selector uses independent wrapping int arithmetic rather than RandomSource.
    for state in [0,1,-1,-(1<<31),(1<<31)-1,*[rng.randrange(-(1<<31),1<<31) for _ in range(100)]]:
        commands.append(f'random.new\t{state}')
        for _ in range(100):
            origin=[rng.choice([-(1<<31),(1<<31)-1,rng.randrange(-30000000,30000000)])for _ in range(3)]
            mask=rng.choice([0,15,255,511,-1,rng.randrange(-(1<<31),1<<31)])
            commands.append('random.pos\t'+'\t'.join(map(str,(*origin,mask))))
    commands.append('rate.new')
    rate_bits=[0,0x80000000,0x3f800000,0x41a00000,0x7f800000,0xff800000,0x7fc01234,0xffc04321,0x00000001,0x7f7fffff,*[rng.getrandbits(32)for _ in range(100)]]
    for bits in rate_bits:commands.extend([f'rate.set\t{bits}','rate.get'])
    for _ in range(100):
        commands.extend([f'rate.freeze\t{rng.choice(["true","false"])}',f'rate.steps\t{rng.choice([-1,0,1,2,4,2147483647])}','rate.get'])
        for _ in range(6):commands.extend(['rate.tick','rate.get'])
    transcript='\n'.join(commands)+'\n'
    libraries=sorted((args.oracle_dir/'libraries').rglob('*.jar'))
    with tempfile.TemporaryDirectory(prefix='mc121-tick-oracle-')as classes:
        subprocess.run([str(args.java_home/'bin/javac'),'-d',classes,str(ROOT/'tools/tick_Oracle.java')],check=True)
        classpath=os.pathsep.join([classes,str(jar),*map(str,libraries)])
        java=subprocess.run([str(args.java_home/'bin/java'),'-cp',classpath,'oracle.TickOracle'],input=transcript,text=True,capture_output=True,check=True)
        if 'ORACLE_CLASS=eyt,eyx,eyo,eys,afl;SOURCE='not in java.stderr or jar.name not in java.stderr:raise RuntimeError('Java class source was not verified')
        rust=subprocess.run([str(args.probe.resolve())],input=transcript,text=True,capture_output=True,check=True)
    expected,actual=java.stdout.splitlines(),rust.stdout.splitlines()
    if len(expected)!=len(commands)or len(actual)!=len(commands):raise RuntimeError('Incomplete probe traces')
    mismatches=[{'line':i+1,'command':commands[i],'java':a,'rust':b}for i,(a,b)in enumerate(zip(expected,actual))if a!=b]
    result={'success':not mismatches,'oracle':'actual Minecraft 1.21 eyt/eyx/eyo/eys/eyq/afl','jar_sha256':EXPECTED_JAR,'rounds':args.rounds,'compared_outputs':len(commands),'trace_sha256':hashlib.sha256(java.stdout.encode()).hexdigest(),'covered':['priority clamping','drain/intra comparators','Java PriorityQueue tie behavior','canonical type+position deduplication','poll reservation removal','pending saved ticks','unpack negative subtick order','saved delay narrowing','time packet freeze/zero/overflow','cross-container due draining and global limits','tickability gates and container removal','fastutil map/heap equal-comparator ties','callbacks rescheduling same-tick work','region clear/copy of scheduled ticks','willTickThisTick remaining queue queries','region clear/copy during callbacks','saved NBT tick loading filters exact owning chunk','unpack after container registration updates scheduling','Level block-random-position independent LCG/wrapping coordinates','TickRateManager rates/freeze/step state including NaN and infinities'],'not_covered':['actual block/fluid tick callbacks','server tick rate recovery or sprint'],'mismatches':mismatches[:10]}
    (ROOT/'tools/tick_conformance.json').write_text(json.dumps(result,indent=2)+'\n')
    if mismatches:(ROOT/'tools/tick_failure.tsv').write_text(transcript)
    print(json.dumps(result,indent=2))
    return int(bool(mismatches))

if __name__=='__main__':raise SystemExit(main())
