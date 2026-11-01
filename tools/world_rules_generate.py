#!/usr/bin/env python3
"""Execute original GameRules loadFromTag on raw-UTF16 NBT fixtures."""
from pathlib import Path
import base64,hashlib,itertools,json,os,random,struct,subprocess,zipfile
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
OUTER_SHA1='450698d1863ab5180c25d7c804ef0fe6369dd1ba';INNER_SHA256='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
PRIORITY=['spawnChunkRadius','spectatorsGenerateChunks','doWeatherCycle','randomTickSpeed','doMobSpawning']
def utf(value):
    units=value if isinstance(value,list)else struct.unpack('>'+'H'*(len(value.encode('utf-16-be','surrogatepass'))//2),value.encode('utf-16-be','surrogatepass'))
    out=bytearray()
    for c in units:
        if 0<c<128:out.append(c)
        elif c<2048:out.extend([0xc0|(c>>6),0x80|(c&63)])
        else:out.extend([0xe0|(c>>12),0x80|((c>>6)&63),0x80|(c&63)])
    return len(out).to_bytes(2,'big')+out

def tag(kind,value):
    formats={1:'>b',2:'>h',3:'>i',4:'>q',5:'>f',6:'>d'}
    if kind in formats:payload=struct.pack(formats[kind],value)
    elif kind==8:payload=utf(value)
    elif kind==10:payload=b''.join(bytes([child[0]])+utf(name)+tag(*child)[1:]for name,child in value)+b'\0'
    elif kind==9:
        element,values=value;payload=bytes([element])+len(values).to_bytes(4,'big')+b''.join(tag(element,v)[1:]for v in values)
    elif kind in [7,11,12]:
        fmt={7:'>b',11:'>i',12:'>q'}[kind];payload=len(value).to_bytes(4,'big')+b''.join(struct.pack(fmt,v)for v in value)
    else:raise ValueError(kind)
    return bytes([kind])+payload

def encoded(value):return base64.b64encode(tag(*value)).decode()
def compound(keys,value):return (10,[(key,value)for key in keys])
def generate(header):
    keys=list(header['types']);cases=[]
    def add(name,initial=None,loads=()):cases.append(dict(name=name,query=dict(initial=None if initial is None else encoded(initial),loads=[encoded(v)for v in loads])))
    add('missing-entire-rules');add('empty-compound',(10,[]));add('whole-tag-string-noncompound',(8,'true'));add('whole-tag-raw-utf16-noncompound',(8,[0xd800]))
    values=['','true','false','TRUE','True','TrUe','FALSE',' true','true ','\ttrue','true\n','1','0','yes','on','null','TRUE\0','t\0rue','+0','-0','+1','-1','2','3','32','33','-32','00003','+00003','-00003','2147483647','-2147483648','2147483648','-2147483649','+2147483647','+2147483648','--1','+-1','++1','-+1',' 1','1 ','\n1','1\n','1_000','1.0','0x10','NaN','Infinity','−1','＋1','１２３','٠١٢٣','١2３','0'*512,'+'+'0'*512,'-'+'0'*512,'9'*512]
    values += [''.join(chars)for chars in itertools.product(*[(c,c.upper())for c in 'true'])]
    for i,text in enumerate(dict.fromkeys(values)):add('string-priority-'+str(i),compound(PRIORITY,(8,text)))
    raw_units=[[0xd800],[0xdc00],[0xd800,0xdc00],[0x74,0x72,0x75,0xd800],[0xd800,0x74,0x72,0x75,0x65],[0x31,0xd800],[0x2d,0xdc00],[0xd835,0xdfce],[0x2b,0xd835,0xdfcf]]
    for i,units in enumerate(raw_units):add('raw-utf16-'+str(i),compound(PRIORITY,(8,units)))
    for zero in header['digit_zeros']:
        for sign in [[],[0x2b],[0x2d]]:
            units=sign+[zero+i for i in range(10)];add('bmp-digits-'+str(zero)+'-'+str(len(sign))+'-'+str(sign),compound(PRIORITY,(8,units)))
        for number in ['2147483647','-2147483648','2147483648','-2147483649']:
            units=[zero+int(c)if c.isdigit()else ord(c)for c in number];add('unicode-int-boundary-'+str(zero)+'-'+number,compound(PRIORITY,(8,units)))
    wrong=[(1,1),(2,-1),(3,2147483647),(4,-9223372036854775808),(5,1.0),(6,float('nan')),(7,[1,-1]),(9,(8,['true','123'])),(10,[('x',(8,'true'))]),(11,[1,-1]),(12,[1,-1])]
    base=(10,[(k,(8,'123'if t=='integer'else'false'))for k,t in header['types'].items()])
    for i,value in enumerate(wrong):
        add('whole-tag-wrong-type-'+str(i),value)
        add('per-rule-wrong-type-defaults-'+str(i),compound(keys,value))
        add('per-rule-wrong-type-keeps-current-'+str(i),base,[compound(keys,value),(10,[]),compound(keys,(8,'invalid'))])
    for text in ['true','false','-2147483648','2147483647','+0017','invalid','']:
        add('all-known-rules-'+text,compound(keys,(8,text)))
    for key in PRIORITY:
        add('duplicate-last-string-'+key,(10,[(key,(8,'true')),(key,(8,'7'))]))
        add('duplicate-last-wrongtype-'+key,(10,[(key,(8,'0')),(key,(3,123))]))
    add('unknown-features-and-datapack-keys',(10,[*base[1],('enabled_features',(9,(8,['minecraft:vanilla','minecraft:bundle','custom:test']))),('customdatapackFeatures',(10,[('doWeatherCycle',(8,'true'))])),('custom:randomTickSpeed',(8,'999')),('spawnchunkradius',(8,'99')),('spawnChunkRadius\0',(8,'999')),([0xd800],(8,'true'))]))
    add('wrongly-supplied-outer-Data-is-not-rules',(10,[('GameRules',base),('enabled_features',(9,(8,['custom:test'])))]))
    rng=random.Random(0x121767aa51)
    for i in range(180):
        chance=rng.random()
        if chance<.5:
            n=rng.randrange(-2**35,2**35);zero=rng.choice(header['digit_zeros']);text=[zero+int(c)if c.isdigit()else ord(c)for c in str(n)]
        elif chance<.75:text=[rng.randrange(65536)for _ in range(rng.randrange(0,15))]
        else:text=''.join(rng.choice('+-0123456789trueFALSE \t\0')for _ in range(rng.randrange(0,20)))
        add('seeded-utf16-string-'+str(i),base,[compound(keys,(8,text)),compound(keys,rng.choice(wrong))])
    return cases

def main():
    inner=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(inner.read_bytes()).hexdigest()==INNER_SHA256;assert hashlib.sha1((ORACLE/'server-1.21.jar').read_bytes()).hexdigest()==OUTER_SHA1
    runtime=ROOT/'runtime/world_rules';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,inner,*sorted((ORACLE/'libraries').rglob('*.jar'))]));source=ROOT/'tools/world_rules_oracle.java';compile=subprocess.run([JDK/'javac','-proc:none','-cp',cp,'-d',classes,source],text=True,capture_output=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    command=[JDK/'java','-Xmx1G','-cp',cp,'oracle.world_rules_oracle'];probe=subprocess.run(command,input='',text=True,capture_output=True,check=True);header=next(json.loads(s[7:])for s in probe.stdout.splitlines()if s.startswith('HEADER:'));assert header['source']=='file:'+str(inner);assert header['public_constructors']==['[]','[class com.mojang.serialization.DynamicLike]'];assert all(header['boolean_char_aliases'][c]==[ord(c.upper()),ord(c)]for c in 'true')
    cases=generate(header);transcript=''.join(json.dumps(c['query'],separators=(',',':'))+'\n'for c in cases);run=subprocess.run(command,input=transcript,text=True,capture_output=True);(runtime/'oracle.log').write_text(run.stdout+run.stderr)
    if run.returncode:raise RuntimeError((run.stdout+run.stderr)[-7000:])
    states=[json.loads(s[7:])for s in run.stdout.splitlines()if s.startswith('ORACLE:')];assert len(cases)==len(states)
    pool=[];index={}
    for case,result in zip(cases,states):
        assert len(result)==len(case['query']['loads'])+1;case['expected']=[]
        for state in result:
            serialized=json.dumps(state,sort_keys=True,separators=(',',':'))
            if serialized not in index:index[serialized]=len(pool);pool.append(state)
            case['expected'].append(index[serialized])
    fixture=ROOT/'fixtures/world_rules_vectors.json';fixture.write_text(json.dumps(dict(minecraft='1.21',facts=header,pool=pool,cases=cases),separators=(',',':'))+'\n');(ROOT/'tools/world_rules_facts.json').write_text(json.dumps(header,indent=2)+'\n')
    with zipfile.ZipFile(inner)as jar:hashes={n:hashlib.sha256(jar.read(n+'.class')).hexdigest()for n in ['dcs','dcs$a','dcs$d','dcs$g','dcs$f','up','uw','uo','ub']}
    report=dict(ok=True,minecraft='1.21',official_outer_sha1=OUTER_SHA1,oracle_inner_sha256=INNER_SHA256,class_sha256=hashes,jdk=header['jdk'],catalog_size=len(header['types']),cases=len(cases),states=sum(map(len,states)),unique_states=len(pool),bmp_digit_ranges=len(header['digit_zeros']),bmp_units_exhaustively_checked=65536,fixture_bytes=fixture.stat().st_size,fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),facts_sha256=hashlib.sha256((ROOT/'tools/world_rules_facts.json').read_bytes()).hexdigest(),oracle_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),generator_source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),scope=['unchanged original GameRules default/DynamicLike constructor, loadFromTag, BooleanValue/IntegerValue.deserialize, createTag logical string contents','all51 static catalog rules; no feature flag constructor or dynamic datapack rule registration in exact1.21','missing/wrongtype preserve current; invalidString resets false/zero; unclamped NBT integers; Boolean.parseBoolean casing; raw UTF16/unpaired surrogates; BMP Unicode decimal digits and overflow','original NbtIo.readAnyTag with lossless Modified UTF8; duplicate names and unknown keys'],not_verified=['gamerule command parser bounds/callback effects, assignFrom callbacks and live gamerule command handling','serialized CompoundTag HashMap key iteration/byte order; logical canonical contents are compared','full custom datapack loading/feature registry behavior beyond the fixed GameRules catalog'])
    (ROOT/'evidence/world-rules-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
