package oracle;
import java.net.*;import java.nio.file.*;
class metadata_launcher {public static void main(String[]args)throws Exception{try(URLClassLoader loader=new URLClassLoader(new URL[]{Path.of(args[0]).toUri().toURL()},metadata_launcher.class.getClassLoader())){var main=Class.forName("metadata_oracle",true,loader).getMethod("main",String[].class);main.setAccessible(true);main.invoke(null,(Object)new String[0]);}}}
