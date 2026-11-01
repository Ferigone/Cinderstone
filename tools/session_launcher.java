package oracle;
import java.net.*;
import java.nio.file.*;
class session_launcher {
 public static void main(String[]args)throws Exception{
  try(URLClassLoader loader=new URLClassLoader(new URL[]{Path.of(args[0]).toUri().toURL()},session_launcher.class.getClassLoader())){
   var main=Class.forName("session_oracle",true,loader).getMethod("main",String[].class);main.setAccessible(true);main.invoke(null,(Object)new String[0]);
  }
 }
}
