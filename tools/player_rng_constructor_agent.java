package oracle;
import java.lang.instrument.*;import java.security.ProtectionDomain;import java.util.jar.JarFile;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Observes stock Entity/ServerPlayer constructor boundaries, without replacing
 * any RNG call, seed, arithmetic or game/environment method. */
public final class player_rng_constructor_agent {
 public static void premain(String observerJar,Instrumentation instrumentation)throws Exception {
  instrumentation.appendToBootstrapClassLoaderSearch(new JarFile(observerJar));
  instrumentation.addTransformer(new ClassFileTransformer(){
   @Override public byte[] transform(ClassLoader loader,String name,Class<?> type,ProtectionDomain domain,byte[]input){
    if(!name.equals("bsr")&&!name.equals("aqv"))return null;
    ClassReader reader=new ClassReader(input);ClassWriter writer=new ClassWriter(reader,ClassWriter.COMPUTE_MAXS);
    reader.accept(new ClassVisitor(ASM8,writer){
     @Override public MethodVisitor visitMethod(int access,String method,String desc,String signature,String[]exceptions){
      MethodVisitor target=super.visitMethod(access,method,desc,signature,exceptions);if(!method.equals("<init>"))return target;
      return new MethodVisitor(ASM8,target){
       void observe(String stage){super.visitVarInsn(ALOAD,0);super.visitLdcInsn(stage);super.visitMethodInsn(INVOKESTATIC,"oracle/player_rng_constructor_observer","observe","(Ljava/lang/Object;Ljava/lang/String;)V",false);}
       @Override public void visitFieldInsn(int opcode,String owner,String field,String desc){super.visitFieldInsn(opcode,owner,field,desc);if(name.equals("bsr")&&opcode==PUTFIELD&&owner.equals("bsr")&&field.equals("ah")&&desc.equals("Layw;"))observe("entity_rng_initialized");}
       @Override public void visitInsn(int opcode){if(opcode==RETURN)observe(name.equals("bsr")?"entity_exit":"server_player_exit");super.visitInsn(opcode);}
      };
     }
    },0);
    System.err.println("PLAYER_RNG_OBSERVER_TRANSFORMED:"+name);return writer.toByteArray();
   }
  });
 }
}
