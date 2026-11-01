package oracle;
import java.net.*;
import java.nio.file.*;
/** Keeps unsigned harness classes outside the signed game's class loader. */
class entity_motion_launcher {
 public static void main(String[] args)throws Exception {
  try(URLClassLoader loader=new URLClassLoader(new URL[]{Path.of(args[0]).toUri().toURL()},entity_motion_launcher.class.getClassLoader())) {
   var main=Class.forName(args.length>1?args[1]:"entity_motion_oracle",true,loader).getMethod("main",String[].class);
   main.setAccessible(true);main.invoke(null,(Object)new String[0]);
  }
 }
}
