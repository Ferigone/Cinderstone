package oracle;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
class PlacementStubGenerate {
    static String name(Class<?> c){return c.getCanonicalName();}
    public static void main(String[]args)throws Exception{
        Class<?> level=Class.forName("dcw",false,PlacementStubGenerate.class.getClassLoader());
        StringBuilder s=new StringBuilder("// Generated abstract-method plumbing only; original block methods perform placement.\nclass PlacementWorld extends dcw {\n    PlacementWorld(){super(null,null,null,null,null,false,false,0L,0);}\n    static final java.util.Map<String,dtc> BLOCKS=new java.util.HashMap<>();\n    public dtc a_(jd p){return BLOCKS.getOrDefault(p.u()+\",\"+p.v()+\",\"+p.w(),dfy.a(0));}\n    public epe b_(jd p){return a_(p).u();}\n    public int I_(){return -64;} public int J_(){return 384;}\n");
        List<Method> all=new ArrayList<>(Arrays.asList(level.getMethods()));for(Method m:level.getDeclaredMethods())if(Modifier.isProtected(m.getModifiers()))all.add(m);
        Set<String> added=new HashSet<>();for(Method m:all){if(!Modifier.isAbstract(m.getModifiers()))continue;String key=m.getName()+Arrays.toString(m.getParameterTypes());if(!added.add(key))continue;s.append("    public ").append(name(m.getReturnType())).append(" ").append(m.getName()).append("(");Class<?>[] types=m.getParameterTypes();for(int i=0;i<types.length;i++){if(i>0)s.append(",");s.append(name(types[i])).append(" p").append(i);}s.append("){throw new UnsupportedOperationException(\"unused placement world method: ").append(m.getName()).append("\");}\n");}
        s.append("}\n");Files.writeString(Path.of(args[0]),s.toString());
    }
}
