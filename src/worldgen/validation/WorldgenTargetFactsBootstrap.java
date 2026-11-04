package oracle;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Path;
/** Invoke the unchanged-original exporter with an explicit current catalog. */
public final class WorldgenTargetFactsBootstrap {
 public static void main(String[] args)throws Exception {
  if(args.length!=3)throw new IllegalArgumentException("child-classes catalog output required");
  try(URLClassLoader child=new URLClassLoader(new URL[]{Path.of(args[0]).toUri().toURL()},WorldgenTargetFactsBootstrap.class.getClassLoader())) {
   Class.forName("WorldgenExistingTargetOracle",true,child).getMethod("main",String[].class).invoke(null,(Object)new String[]{"--export-state-facts",args[1],args[2]});
  }
 }
}
