package oracle;
import java.net.*;
import java.nio.file.*;
public final class EmptyBeardifierBootstrap {
  public static void main(String[] args)throws Exception {
    if(args.length!=1)throw new IllegalArgumentException("child-classes directory required");
    try(URLClassLoader child=new URLClassLoader(new URL[]{Path.of(args[0]).toUri().toURL()},EmptyBeardifierBootstrap.class.getClassLoader())) {
      Class.forName("EmptyBeardifierProbe",true,child).getMethod("main",String[].class).invoke(null,(Object)new String[0]);
    }
  }
}
