package oracle;
import java.io.*;
import java.lang.invoke.MethodHandles;

/** Generates a two-method broadcast interceptor in oracle's unsigned package.
 * Java source cannot extend an unnamed-package class from a named package;
 * the JVM permits it. No Minecraft class or JAR bytes are changed. There is no
 * constructor because the test uses Unsafe.allocateInstance, as for the world. */
final class WeatherPlayerList {
    static Class<?> createType()throws Exception {
        ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);
        out.writeInt(0xcafebabe);out.writeShort(0);out.writeShort(61);out.writeShort(19);
        utf(out,"oracle/WeatherCapture");clazz(out,1);utf(out,"apm");clazz(out,3);
        utf(out,"a");utf(out,"(Lzg;)V");utf(out,"(Lzg;Lakq;)V");utf(out,"Code");
        utf(out,"oracle/WeatherOracle");clazz(out,9);utf(out,"capture");utf(out,"(Ljava/lang/String;Ljava/lang/Object;)V");
        out.writeByte(12);out.writeShort(11);out.writeShort(12);
        out.writeByte(10);out.writeShort(10);out.writeShort(13);
        utf(out,"all");out.writeByte(8);out.writeShort(15);utf(out,"dimension");out.writeByte(8);out.writeShort(17);
        out.writeShort(0x21);out.writeShort(2);out.writeShort(4);out.writeShort(0);out.writeShort(0);out.writeShort(2);
        method(out,6,16,2);method(out,7,18,3);out.writeShort(0);out.flush();
        return MethodHandles.lookup().defineClass(bytes.toByteArray());
    }
    static void utf(DataOutputStream out,String value)throws Exception{out.writeByte(1);out.writeUTF(value);}
    static void clazz(DataOutputStream out,int name)throws Exception{out.writeByte(7);out.writeShort(name);}
    static void method(DataOutputStream out,int descriptor,int route,int locals)throws Exception{
        out.writeShort(1);out.writeShort(5);out.writeShort(descriptor);out.writeShort(1);out.writeShort(8);out.writeInt(19);
        out.writeShort(2);out.writeShort(locals);out.writeInt(7);out.write(new byte[]{0x12,(byte)route,0x2b,(byte)0xb8,0,14,(byte)0xb1});out.writeShort(0);out.writeShort(0);
    }
}
