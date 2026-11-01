#!/usr/bin/env python3
"""Compile isolated environmental stubs, run actual Minecraft1.21 menu logic."""
import argparse,hashlib,json,os,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
JAVA=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
ORACLE=ROOT.parent/'mc121-oracle'
OUT=Path('/tmp/mc121-container-oracle')

def command():
    OUT.mkdir(exist_ok=True)
    jar=ORACLE/'server-inner-1.21.jar'
    assert hashlib.sha256(jar.read_bytes()).hexdigest()=='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
    cp=os.pathsep.join(map(str,[OUT,jar,*(ORACLE/'libraries').rglob('*.jar')]))
    exports=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    subprocess.run([str(JAVA/'javac'),*exports,'-proc:none','-cp',cp,'-d',OUT,ROOT/'tools/container_oracle.java'],check=True)
    return [str(JAVA/'java'),*exports,'-cp',cp,'oracle.container_oracle']

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--equipment',type=Path);parser.add_argument('--queries',type=Path);parser.add_argument('--output',type=Path);args=parser.parse_args();cmd=command()
    if args.equipment:
        subprocess.run([*cmd,'equipment',args.equipment],check=True);return
    if args.queries:
        result=subprocess.run(cmd,input=args.queries.read_text(),capture_output=True,text=True,check=True)
        rows=[json.loads(line.split('ORACLE:',1)[1]) for line in result.stdout.splitlines() if 'ORACLE:' in line]
        assert len(rows)==len(args.queries.read_text().splitlines()),result.stderr
        queries=[json.loads(line) for line in args.queries.read_text().splitlines()]
        merged=[{**query,**row} for query,row in zip(queries,rows)]
        if args.output:args.output.write_text(json.dumps({'minecraft':'1.21','oracle_inner_sha256':'8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01','scope':'Actual cpu.doClick+cqw.quickMoveStack; craft-result recomputation disabled; environmental equip/drop hooks replaced; drops cumulative','cases':merged},separators=(',',':'))+'\n')
        print(json.dumps({'cases':len(rows),'errors':[(r['name'],r['error']) for r in rows if 'error' in r]}));return
    q={'name':'pickup','creative':False,'slots':[None]*46,'carried':None,'clicks':[{'slot':9,'button':0,'type':'PICKUP'},{'slot':10,'button':1,'type':'PICKUP'},{'slot':-999,'button':0,'type':'PICKUP'}]};q['slots'][9]={'id':'minecraft:stone','count':12}
    result=subprocess.run(cmd,input=json.dumps(q)+'\n',capture_output=True,text=True);print(result.stdout);print(result.stderr);assert result.returncode==0
if __name__=='__main__':main()
