#!/usr/bin/env python3
"""Extract exact IEEE754 coefficients/table from the pinned primary HotSpot source."""
import pathlib,re,json,hashlib
r=pathlib.Path(__file__).resolve().parent.parent;p=r/'tools/hotspot_math_sources';s=(p/'stubGenerator_x86_64_constants.cpp').read_text()
def table(name):
 text=re.search(r'_'+name+r'\[\]\s*=\s*\{(.*?)\};',s,re.S).group(1);words=[int(x,16) for x in re.findall(r'0x([0-9a-fA-F]+)UL',text)];assert len(words)%2==0;return [(words[i+1]<<32)|words[i] for i in range(0,len(words),2)]
data={n:table(n) for n in ['SC_1','SC_2','SC_3','SC_4','PI32INV','P_1','P_2','P_3','Ctable']};assert len(data['Ctable'])==256
meta=json.loads((p/'source.json').read_text());meta.update({'source_sha256':{f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in p.glob('*.cpp')},'constants':{n:[f'{v:016x}' for v in a] for n,a in data.items()}})
(r/'fixtures/hotspot_math_constants.json').write_text(json.dumps(meta,indent=2)+'\n')
header=(p/'stubGenerator_x86_64_cos.cpp').read_text().split('*/',1)[0]+'*/\n'
header=header.replace(' * Intel Math Library (LIBM) Source Code',' * Intel Math Library (LIBM) Source Code\n * Copyright (c) 2022, Oracle and/or its affiliates. All rights reserved.')
code=header+'''//! Scalar transcription of HotSpot x86_64 Intel LIBM's bounded sin/cos path.
//!
//! Preserves each SSE2 arithmetic operation's evaluation order; no fma or
//! platform libm/JVM calls are used. Validated against the pinned Microsoft
//! OpenJDK21.0.13.0.2+1 x86_64 runtime. Other JVM/CPU implementations can choose
//! different last-bit results. The public domain is finite [0, f32::TAU], plus
//! negative zero, which covers Minecraft1.21 Player.drop's f32 jitter angles.
//!
//! Source: microsoft/openjdk-jdk21u commit ccb06ed8ec75927d88ff859a9198a7ea72f698b6,
//! stubGenerator_x86_64_{sin,cos,constants}.cpp. Source headers and GPLv2 notice
//! are retained in tools/hotspot_math_sources and licenses/hotspot-math-GPL2.txt.

#[inline]
fn d(bits:u64)->f64 {f64::from_bits(bits)}
'''
for n in ['SC_1','SC_2','SC_3','SC_4']:
 code+=f'const {n}:[u64;2]=['+','.join(f'0x{v:016x}' for v in data[n])+'];\n'
for n in ['PI32INV','P_1','P_2','P_3']:code+=f'const {n}:u64=0x{data[n][0]:016x};\n'
code+='const TABLE:[[u64;4];64]=[\n'+''.join('    ['+','.join(f'0x{v:016x}' for v in data['Ctable'][i:i+4])+'],\n' for i in range(0,256,4))+'];\n'
code+='''
/// Return (sin(angle), cos(angle)) with the pinned x86_64 HotSpot kernel's bits.
/// Panics for inputs outside this bounded implementation's documented domain.
pub fn sin_cos(angle:f64)->(f64,f64) {
    assert!(angle.is_finite() && angle>=0. && angle<=f64::from(std::f32::consts::TAU),"HotSpot bounded trig requires finite [0,f32::TAU]");
    if angle==0. || angle<d(0x3030000000000000) {return (angle,1.);}
    (kernel(angle,0),kernel(angle,16))
}

#[inline]
fn kernel(x:f64,phase:usize)->f64 {
    let n=(x*d(PI32INV)+0.5) as i32;
    let nf=f64::from(n);
    let p1=nf*d(P_1);
    let p2=nf*d(P_2);
    let p3=nf*d(P_3);
    let r1=x-p1;
    let r=r1-p2;
    let c1=r1-r;
    let c2=c1-p2;
    let neg_c=p3-c2;
    let [c_hl,s_hi,s_lo,sigma]=TABLE[(n as usize+phase)&63].map(d);
    let c_plus_sigma=c_hl+sigma;
    let neg_d=s_hi*r-c_plus_sigma;
    let c_times_r=c_plus_sigma*r;
    let rs=sigma*r;
    let med=r*c_hl;
    let mut sc4=[d(SC_4[0])*r1,d(SC_4[1])*r1];
    sc4[0]*=r;sc4[1]*=r;
    let r2=r*r;
    let products=[c_times_r*r2,s_hi*r2];
    let r4=r2*r2;
    let mut sc2=[d(SC_2[0])*r2,d(SC_2[1])*r2];
    sc4[0]+=d(SC_3[0]);sc4[1]+=d(SC_3[1]);
    sc2[0]+=d(SC_1[0]);sc2[1]+=d(SC_1[1]);
    sc4[0]*=r4;sc4[1]*=r4;
    sc2[0]+=sc4[0];sc2[1]+=sc4[1];
    let poly=[sc2[0]*products[0],sc2[1]*products[1]];
    let res_int=rs+s_hi;
    let res_hi=med+res_int;
    let k0=s_hi-res_int;
    let k1=res_int-res_hi;
    let k2=rs+k0;
    let k3=k1+med;
    let corr=neg_c*neg_d+s_lo;
    let mut lo=k2+corr;
    lo+=k3;lo+=poly[0];lo+=poly[1];
    lo+res_hi
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formerly_one_ulp_cos_disagreement_matches_hotspot() {
        assert_eq!(sin_cos(5.185734748840332).1.to_bits(),4601883784787796961);
    }
    #[test]
    fn exact_pinned_hotspot_oracle_corpus() {
        let data=include_bytes!("../fixtures/hotspot_math_vectors.bin");
        assert_eq!(&data[..8],b"MC21TRIG");let count=u32::from_be_bytes(data[8..12].try_into().unwrap()) as usize;
        assert!(count>=100000);assert_eq!(data.len(),12+count*24);
        for (i,record) in data[12..].chunks_exact(24).enumerate(){let bits=|off|u64::from_be_bytes(record[off..off+8].try_into().unwrap());let angle=f64::from_bits(bits(0));let (sin,cos)=sin_cos(angle);assert_eq!(sin.to_bits(),bits(8),"case{i},sin({angle:?})");assert_eq!(cos.to_bits(),bits(16),"case{i},cos({angle:?})");}
    }
}
'''
(r/'src/hotspot_math.rs').write_text(code);print(json.dumps({'table_rows':64,'source_commit':meta['commit']}))
