#!/usr/bin/env python3
"""Compare Rust weather against actual ServerLevel.advanceWeatherCycle bytecode."""
import argparse, hashlib, json, os, random, struct, subprocess, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
EXPECTED_JAR='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def main():
    p=argparse.ArgumentParser();p.add_argument('--oracle-dir',type=Path,default=ROOT.parent/'mc121-oracle');p.add_argument('--java-home',type=Path,default=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1'));p.add_argument('--probe',type=Path,default=ROOT/'target/debug/tick_probe');p.add_argument('--rounds',type=int,default=100);args=p.parse_args()
    jar=args.oracle_dir/'server-inner-1.21.jar'
    if hashlib.sha256(jar.read_bytes()).hexdigest()!=EXPECTED_JAR:raise RuntimeError('Wrong Minecraft oracle JAR')
    if args.rounds<1:p.error('--rounds must be positive')
    rng=random.Random(0x121777)
    def bits(v):return struct.unpack('>I',struct.pack('>f',v))[0]
    level_bits=[bits(v)for v in [0.,-0.,0.01,0.19,0.2,0.21,0.89,0.9,0.91,0.99,1.,1.01,-0.01]]
    timer_values=[-2147483648,-1,0,1,2,30,2147483647]
    commands=[]
    for case in range(args.rounds):
        clear=timer_values[case%len(timer_values)];thunder=rng.choice(timer_values);rain=rng.choice(timer_values)
        thundering=bool(case&1);raining=bool(case&2);skylight=not bool(case&4);cycle=not bool(case&8)
        levels=rng.choices(level_bits,k=4)
        if case==0:clear=thunder=rain=0;levels=[0]*4
        if case==1:clear=0;thunder=rain=1;levels=[bits(0.19)]*4
        if case==2:clear=0;thunder=rain=1;levels=[bits(0.21)]*4
        if case%13==12:levels=rng.choices([0x7fc01234,0x7f800000,0xff800000,0x7f7fffff,0xff7fffff],k=4)
        seed=rng.randrange(-(1<<63),1<<63)
        values=[clear,thunder,rain,str(thundering).lower(),str(raining).lower(),*levels,seed,str(skylight).lower(),str(cycle).lower()]
        commands.append('weather.new\t'+'\t'.join(map(str,values)))
        commands.append('weather.get')
        for tick in range(300):
            if tick==100:commands.extend(['weather.reset','weather.get','weather.prepare','weather.get'])
            if tick==180:commands.extend([f'weather.set\t{rng.choice([0,1,30])}\t{rng.choice([0,1,30])}\t{str(bool(case&1)).lower()}\t{str(bool(case&2)).lower()}','weather.get','weather.prepare','weather.get'])
            commands.append('weather.tick')
    transcript='\n'.join(commands)+'\n'
    libraries=sorted((args.oracle_dir/'libraries').rglob('*.jar'))
    with tempfile.TemporaryDirectory(prefix='mc121-weather-oracle-')as classes:
        subprocess.run([str(args.java_home/'bin/javac'),'-proc:none','-cp',os.pathsep.join([str(jar),*map(str,libraries)]),'-d',classes,str(ROOT/'tools/tick_WeatherOracle.java'),str(ROOT/'tools/tick_WeatherPlayerList.java')],check=True)
        cp=os.pathsep.join([classes,str(jar),*map(str,libraries)])
        java=subprocess.run([str(args.java_home/'bin/java'),'-cp',cp,'oracle.WeatherOracle'],input=transcript,text=True,capture_output=True,check=True)
        if 'WEATHER_ORACLE_CLASS=aqu,dcw,dyz;SOURCE='not in java.stderr or jar.name not in java.stderr:raise RuntimeError('Oracle provenance not confirmed')
        rust=subprocess.run([str(args.probe.resolve())],input=transcript,text=True,capture_output=True,check=True)
    expected=java.stdout.splitlines();actual=rust.stdout.splitlines()
    if len(expected)!=len(commands)or len(actual)!=len(commands):raise RuntimeError(f'Incomplete traces: commands={len(commands)}, Java={len(expected)}, Rust={len(actual)}; Java beginning={expected[:3]}')
    mismatches=[{'line':i+1,'command':commands[i],'java':a,'rust':b}for i,(a,b)in enumerate(zip(expected,actual))if a!=b]
    result={'success':not mismatches,'oracle':'actual Minecraft 1.21 aqu.advanceWeatherCycle/dcw weather interpolation/dyz random','jar_sha256':EXPECTED_JAR,'rounds':args.rounds,'compared_outputs':len(commands),'trace_sha256':hashlib.sha256(java.stdout.encode()).hexdigest(),'covered':['clear/rain/thunder timers and toggle boundaries','weather random sampling bounds/order/consumption','hasSkylight and doWeatherCycle gates','float32 weather smoothing and clamping','rain/thunder interpolation threshold predicates','prepareWeather and resetWeatherCycle','setWeatherParameters','signed zero, NaN/infinity and overflow edge levels','actual ServerLevel weather packet reasons, values, order and dimension/global routing'],'not_covered':['chunk lightning/precipitation/ice/snow effects','weather persistence integration'],'mismatches':mismatches[:10]}
    (ROOT/'tools/tick_weather_conformance.json').write_text(json.dumps(result,indent=2)+'\n')
    if mismatches:(ROOT/'tools/tick_weather_failure.tsv').write_text(transcript)
    print(json.dumps(result,indent=2));return int(bool(mismatches))
if __name__=='__main__':raise SystemExit(main())
