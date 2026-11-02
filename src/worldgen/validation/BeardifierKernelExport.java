package oracle;
import java.lang.reflect.*;import java.nio.file.*;import java.security.*;import java.util.*;
public final class BeardifierKernelExport {
 public static void main(String[] args)throws Exception {
  Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
  Class<?> type=Class.forName("dyl");Field field=type.getDeclaredField("g");field.setAccessible(true);float[] kernel=(float[])field.get(null);
  if(kernel.length!=13824)throw new AssertionError("unexpected kernel size");StringBuilder out=new StringBuilder("{\"original_inner_sha256\":\"");
  String sha=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(Path.of(type.getProtectionDomain().getCodeSource().getLocation().toURI()))));
  if(!sha.equals("8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"))throw new AssertionError("wrong original JAR");out.append(sha).append("\",\"bits\":[");
  for(int i=0;i<kernel.length;i++){if(i>0)out.append(',');out.append(Integer.toUnsignedLong(Float.floatToRawIntBits(kernel[i])));}
  out.append("]}\n");Files.writeString(Path.of(args[0]),out);System.out.println("original_kernel_entries="+kernel.length);
 }
}
