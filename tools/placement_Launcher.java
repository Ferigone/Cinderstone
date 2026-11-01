package oracle;
import java.nio.file.*;
import java.lang.reflect.*;
import java.security.*;

/** Keeps original JAR signatures intact. Controlled subclasses are defined in
 * a separate loader using the original class protection domain; original game
 * classes are delegated unchanged to their normal signed-JAR loader. */
class PlacementLauncher extends ClassLoader {
    final Path classes;
    PlacementLauncher(Path classes){super(PlacementLauncher.class.getClassLoader());this.classes=classes;}
    protected synchronized Class<?> loadClass(String name,boolean resolve)throws ClassNotFoundException {
        if(!name.equals("PlacementOracle")&&!name.equals("PlacementWorld")&&!name.equals("PlacementContextOracle")&&!name.equals("PlacementBorder"))return super.loadClass(name,resolve);
        Class<?> found=findLoadedClass(name);if(found!=null)return found;
        try{byte[] bytes=Files.readAllBytes(classes.resolve(name+".class"));Class<?> original=Class.forName("dcw",false,getParent());found=defineClass(name,bytes,0,bytes.length,original.getProtectionDomain());if(resolve)resolveClass(found);return found;}catch(Exception e){throw new ClassNotFoundException(name,e);}
    }
    public static void main(String[]args)throws Exception{Class<?> main=new PlacementLauncher(Path.of(args[0])).loadClass("PlacementOracle");Method m=main.getMethod("main",String[].class);m.setAccessible(true);m.invoke(null,(Object)new String[]{args[1]});}
}
