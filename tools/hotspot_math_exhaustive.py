#!/usr/bin/env python3
"""Stream-compare every possible Player.drop nextFloat jitter angle, without retaining the 256MiB streams."""
import pathlib,subprocess,os,hashlib,json,time,struct
r=pathlib.Path(__file__).resolve().parent.parent;j=pathlib.Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');build=r/'target/hotspot-math-oracle';build.mkdir(parents=True,exist_ok=True);env=os.environ.copy();env.setdefault('CARGO_HOME','/root/.local/share/mc-rust-toolchain/cargo');env.setdefault('RUSTUP_HOME','/root/.local/share/mc-rust-toolchain/rustup');rustc=pathlib.Path(env['CARGO_HOME'])/'bin/rustc'
subprocess.run([str(j/'javac'),'-proc:none','-d',str(build),str(r/'tools/hotspot_math_oracle.java')],check=True);binary=build/'native-exhaustive';subprocess.run([str(rustc),'--edition','2021','-C','opt-level=3','-C','lto=thin',str(r/'tools/hotspot_math_exhaustive.rs'),'-o',str(binary)],check=True,env=env)
start=time.monotonic();java=subprocess.Popen([str(j/'java'),'-Xmx64M','-cp',str(build),'hotspot_math_oracle','--exhaustive'],stdout=subprocess.PIPE,stderr=subprocess.PIPE);native=subprocess.Popen([str(binary)],stdout=subprocess.PIPE,stderr=subprocess.PIPE);digest=hashlib.sha256();total=0
try:
 while True:
  a=java.stdout.read(1<<20);b=native.stdout.read(1<<20)
  if a!=b:
   for i in range(0,min(len(a),len(b)),16):
    if a[i:i+16]!=b[i:i+16]:raise AssertionError({'index':(total+i)//16,'java':a[i:i+16].hex(),'rust':b[i:i+16].hex()})
   raise AssertionError({'offset':total,'java_bytes':len(a),'rust_bytes':len(b)})
  if not a:break
  digest.update(a);total+=len(a)
 assert java.wait()==0,java.stderr.read().decode();assert native.wait()==0,native.stderr.read().decode();assert total==(1<<24)*16,total
 result={'ok':True,'minecraft':'1.21','runtime':'MicrosoftOpenJDK21.0.13.0.2+1-LTS,x86_64','method':'direct streaming byte comparison of every JavaRandom.nextFloat value multiplied by f32TAU; native optimizedthinLTO vs actualMath.sin/cos','angles':1<<24,'source_float_samples':1<<24,'angle_note':'f32 multiplication can map distinct nextFloat samples to the same angle; every source sample is checked','bit_comparisons':(1<<24)*2,'bytes_compared':total,'differences':0,'stream_sha256':digest.hexdigest(),'jvm_library_sha256':hashlib.sha256((j.parent/'lib/server/libjvm.so').read_bytes()).hexdigest(),'native_source_sha256':hashlib.sha256((r/'src/hotspot_math.rs').read_bytes()).hexdigest(),'seconds':round(time.monotonic()-start,3),'scope':'all possible f32 jitter angles in Minecraft1.21 Player.drop; other JVM/CPU implementations and other trigdomains not covered'}
 (r/'evidence/hotspot-math-exhaustive.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
finally:
 for process in [java,native]:
  if process.poll() is None:process.kill();process.wait()
