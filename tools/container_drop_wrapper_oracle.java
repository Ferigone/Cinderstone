package oracle;
import jdk.internal.org.objectweb.asm.*;
import java.nio.file.*;

/** Execute the unchanged two-argument Player.drop wrapper; only its final
 * three-argument environmental drop implementation is captured. */
public class container_drop_wrapper_oracle {
    public static void main(String[] arguments)throws Exception {
        Path temporary=Files.createTempFile("drop-wrapper-bootstrap",".json");
        try {container_oracle.main(new String[]{"equipment",temporary.toString()});}
        finally {Files.delete(temporary);}
        ClassWriter w=new ClassWriter(0);
        w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleDropWrapperPlayer",null,"cmx",null);
        MethodVisitor m=w.visitMethod(Opcodes.ACC_PUBLIC,"a","(Lcuq;ZZ)Lcjh;",null,new String[]{"java/lang/Exception"});
        m.visitCode();m.visitVarInsn(Opcodes.ALOAD,1);m.visitVarInsn(Opcodes.ILOAD,2);m.visitVarInsn(Opcodes.ILOAD,3);
        m.visitMethodInsn(Opcodes.INVOKESTATIC,"oracle/container_oracle","recordDrop","(Ljava/lang/Object;ZZ)V",false);
        m.visitInsn(Opcodes.ACONST_NULL);m.visitInsn(Opcodes.ARETURN);m.visitMaxs(3,4);m.visitEnd();w.visitEnd();
        Object player=container_oracle.unsafe(container_oracle.define("OracleDropWrapperPlayer",w.toByteArray()));
        Object stack=container_oracle.parse(com.google.gson.JsonParser.parseString("{\"id\":\"minecraft:stone\",\"count\":1}"));
        com.google.gson.JsonArray out=new com.google.gson.JsonArray();
        for(boolean trace:new boolean[]{false,true}) {
            container_oracle.resetDrops();
            Class.forName("cmx").getMethod("a",Class.forName("cuq"),boolean.class).invoke(player,stack,trace);
            if(container_oracle.dropped.size()!=1)throw new AssertionError("wrapper failed to dispatch");
            boolean[] flags=container_oracle.dropFlags.get(container_oracle.dropped.get(0));
            com.google.gson.JsonArray row=new com.google.gson.JsonArray();row.add(trace);row.add(flags[0]);row.add(flags[1]);out.add(row);
        }
        System.out.println("ORACLE_WRAPPER:"+out);
    }
}
