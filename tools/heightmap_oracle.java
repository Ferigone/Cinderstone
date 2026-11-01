package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import java.util.function.Predicate;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Calls original Heightmap.primeHeightmaps/update; only ChunkAccess world data
 * is supplied by an oracle-only ASM subclass. Minecraft classes stay unchanged.
 */
public final class heightmap_oracle {
    static final Gson JSON=new Gson();static final PrintStream OUT=System.out;
    static final String[] NAMES={"WORLD_SURFACE","OCEAN_FLOOR","MOTION_BLOCKING","MOTION_BLOCKING_NO_LEAVES"};
    static Class<?> cls(String n)throws Exception{return Class.forName(n);}
    static Class<?>[] types(Class<?>...t){return t;}
    static Object call(Object o,String n,Class<?>[]t,Object...a)throws Exception{return survival_oracle.call(o,n,t,a);}
    static Object unsafe;static Method allocate;static Class<?> CHUNK,HEIGHT,STATE,POS,TYPE;
    static Object[] states,types,heightmaps;static int[] column;static int sceneX,sceneZ;static int reads;
    static final Map<Object,Object> maps=new HashMap<>();
    public static Object block(Object pos)throws Exception {
        reads++;
        int x=(int)call(pos,"u",types()),y=(int)call(pos,"v",types()),z=(int)call(pos,"w",types());
        if(x==sceneX&&z==sceneZ&&y>=-64&&y<320)return states[column[y+64]];
        return states[0];
    }
    public static Object map(Object type){return maps.get(type);}
    static void constant(ClassWriter c,String name,int value){MethodVisitor m=survival_oracle.method(c,name,"()I");m.visitLdcInsn(value);m.visitInsn(IRETURN);survival_oracle.end(m);}
    static void init()throws Exception {
        call(cls("ab"),"a",types());call(cls("akt"),"a",types());
        Class<?> u=cls("sun.misc.Unsafe");Field f=u.getDeclaredField("theUnsafe");f.setAccessible(true);unsafe=f.get(null);allocate=u.getMethod("allocateInstance",Class.class);
        HEIGHT=cls("dyy");TYPE=cls("dyy$a");STATE=cls("dtc");POS=cls("jd");
        states=new Object[26684];for(int i=0;i<states.length;i++)states[i]=call(cls("dfy"),"a",types(int.class),i);
        types=new Object[4];Object[] all=TYPE.getEnumConstants();for(int i=0;i<4;i++)for(Object t:all)if(((String)call(t,"a",types())).equals(NAMES[i]))types[i]=t;
        String name="oracle/HeightmapChunk";ClassWriter c=survival_oracle.begin(name,"duy");constant(c,"I_",-64);constant(c,"J_",384);constant(c,"b",304);
        MethodVisitor m=survival_oracle.method(c,"a_","(Ljd;)Ldtc;");m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/heightmap_oracle","block","(Ljava/lang/Object;)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"dtc");m.visitInsn(ARETURN);survival_oracle.end(m);
        m=survival_oracle.method(c,"a","(Ldyy$a;)Ldyy;");m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/heightmap_oracle","map","(Ljava/lang/Object;)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"dyy");m.visitInsn(ARETURN);survival_oracle.end(m);
        CHUNK=survival_oracle.define(new survival_oracle(),name,c);
        OUT.println("SOURCE:"+HEIGHT.getProtectionDomain().getCodeSource().getLocation());
    }
    static int[] predicates()throws Exception {
        int[] masks=new int[states.length];
        for(int k=0;k<4;k++){
            @SuppressWarnings("unchecked") Predicate<Object> p=(Predicate<Object>)call(types[k],"e",types());
            for(int i=0;i<states.length;i++)if(p.test(states[i]))masks[i]|=1<<k;
        }
        return masks;
    }
    static JsonArray heights()throws Exception {JsonArray a=new JsonArray();for(Object h:heightmaps)a.add((int)call(h,"a",types(int.class,int.class),sceneX,sceneZ));return a;}
    static JsonArray raw()throws Exception {JsonArray a=new JsonArray();for(Object h:heightmaps){JsonArray words=new JsonArray();for(long v:(long[])call(h,"a",types()))words.add(Long.toUnsignedString(v));a.add(words);}return a;}
    static JsonObject scene(JsonObject q)throws Exception {
        sceneX=q.get("x").getAsInt();sceneZ=q.get("z").getAsInt();column=JSON.fromJson(q.get("initial"),int[].class);
        if(column.length!=384)throw new IllegalArgumentException("384blockcolumnrequired");
        Object chunk=allocate.invoke(unsafe,CHUNK);heightmaps=new Object[4];maps.clear();
        for(int i=0;i<4;i++){heightmaps[i]=HEIGHT.getConstructor(cls("duy"),TYPE).newInstance(chunk,types[i]);maps.put(types[i],heightmaps[i]);}
        call(HEIGHT,"a",types(cls("duy"),Set.class),chunk,new LinkedHashSet<>(Arrays.asList(types)));
        JsonObject out=new JsonObject();out.add("initial_heights",heights());out.add("initial_raw",raw());JsonArray steps=new JsonArray();
        for(JsonElement e:q.getAsJsonArray("edits")){
            JsonArray edit=e.getAsJsonArray();int y=edit.get(0).getAsInt(),state=edit.get(1).getAsInt();if(y < -64 || y >=320)throw new IllegalArgumentException("ybound");
            column[y+64]=state;int changed=0;JsonArray counts=new JsonArray();
            for(int k=0;k<4;k++){reads=0;if((boolean)call(heightmaps[k],"a",types(int.class,int.class,int.class,STATE),sceneX,y,sceneZ,states[state]))changed|=1<<k;counts.add(reads);}
            JsonArray row=new JsonArray();row.add(y);row.add(state);row.add(changed);row.add(heights());row.add(counts);steps.add(row);
        }
        out.add("steps",steps);out.add("final_raw",raw());return out;
    }
    public static void main(String[]args)throws Exception {
        init();
        if(args.length>0&&args[0].equals("facts")){OUT.println("PREDICATES:"+JSON.toJson(predicates()));return;}
        BufferedReader reader=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=reader.readLine())!=null;){try{OUT.println("ORACLE:"+JSON.toJson(scene(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable e){OUT.println("QUERY:"+line);e.printStackTrace(OUT);throw e;}}
    }
}
