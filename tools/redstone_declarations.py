#!/usr/bin/env python3
"""Extract callback declarations from the unchanged official1.21 classfiles."""
import json,re,subprocess,pathlib
root=pathlib.Path(__file__).resolve().parents[1]
jar=root.parent/'mc121-oracle/server-inner-1.21.jar'
cachefile=root/'runtime/redstone/class-declarations.json'
cache=json.loads(cachefile.read_text()) if cachefile.exists() else {}
def info(c):
 if c in cache:return cache[c]
 p=subprocess.run(['java','--module','jdk.jdeps/com.sun.tools.javap.Main','-p','-s','-classpath',str(jar),c],capture_output=True,text=True,check=True).stdout
 header=next(x for x in p.splitlines() if ' class ' in x or x.startswith('class '))
 while '<' in header:header=re.sub(r'<[^<>]*>','',header)
 m=re.search(r'\bclass\s+\S+\s+extends\s+([^\s<{]+)',header)
 parent=m[1] if m else None
 signatures=set(re.findall(r'\b([ab])\(dtc, dcc, jd, ji\);',p))
 cache[c]=(parent,list(signatures));cachefile.write_text(json.dumps(cache))
 return cache[c]
def owner(c,name):
 parent,sigs=info(c)
 if name in sigs:return c
 if not parent:raise ValueError(c)
 return owner(parent,name)
b=json.loads((root/'fixtures/placement-1.21.json').read_text())['blocks']
rows=[]
for i,v in enumerate(b):
 rows.append({'name':v['name'],'min':v['min'],'max':v['max'],'class':v['class'],'signal':owner(v['class'],'a'),'direct':owner(v['class'],'b')})
path=root/'fixtures/redstone-declarations.json'
path.write_text(json.dumps({'minecraft':'1.21','jar_sha256':'8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01','blocks':rows},separators=(',',':'))+'\n')
print({'blocks':len(rows),'classes':len(cache),'file':str(path)})
