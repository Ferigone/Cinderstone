package oracle;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.function.*;
import java.util.function.Predicate;

/** Calls original Minecraft classes; no geometry implementation is duplicated. */
class GeometryOracle {
    static final PrintStream OUT=System.out,ERR=System.err;
    static Class<?> V,B,D,A,S,GRID,ARR,POS,DIM;
    static Object[] axes;
    static Object player,attributes;
    static {
        try {
            Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
            V=Class.forName("exc");B=Class.forName("ewx");D=Class.forName("dcc");A=Class.forName("ji$a");S=Class.forName("exv");GRID=Class.forName("exf");ARR=Class.forName("exe");POS=Class.forName("jd");DIM=Class.forName("bsu");axes=A.getEnumConstants();
            ERR.println("ORACLE_CLASS=exc,ewx,dcc,exv,bsu;SOURCE="+V.getProtectionDomain().getCodeSource().getLocation());
        }catch(Exception e){throw new ExceptionInInitializerError(e);}
    }
    static double d(String s){return Double.longBitsToDouble(Long.parseUnsignedLong(s,16));}
    static String bits(double n){return String.format("%016x",Double.doubleToLongBits(n));}
    static Object vec(String[] f,int i)throws Exception{return V.getConstructor(double.class,double.class,double.class).newInstance(d(f[i]),d(f[i+1]),d(f[i+2]));}
    static Object box(String[] f,int i)throws Exception{return B.getConstructor(double.class,double.class,double.class,double.class,double.class,double.class).newInstance(d(f[i]),d(f[i+1]),d(f[i+2]),d(f[i+3]),d(f[i+4]),d(f[i+5]));}
    static String vecOut(Object v)throws Exception{return bits(V.getField("c").getDouble(v))+","+bits(V.getField("d").getDouble(v))+","+bits(V.getField("e").getDouble(v));}
    static String boxOut(Object b)throws Exception{List<String> n=new ArrayList<>();for(String s:List.of("a","b","c","d","e","f"))n.add(bits(B.getField(s).getDouble(b)));return String.join(",",n);}
    static String run(String line)throws Exception{
        String[] f=line.split("\t",-1);
        switch(f[0]){
            case "look": {Class<?> unsafeClass=Class.forName("sun.misc.Unsafe");Field uf=unsafeClass.getDeclaredField("theUnsafe");uf.setAccessible(true);Object unsafe=uf.get(null);Object entity=unsafeClass.getMethod("allocateInstance",Class.class).invoke(unsafe,Class.forName("aqv"));for(int i=0;i<2;i++){Field angle=Class.forName("bsr").getDeclaredField(i==0?"aE":"aD");angle.setAccessible(true);angle.setFloat(entity,Float.intBitsToFloat(Integer.parseUnsignedInt(f[i+1],16)));}Field head=Class.forName("btn").getField("ba");head.setFloat(entity,Float.intBitsToFloat(Integer.parseUnsignedInt(f[2],16)));Object[] ordered=(Object[])Class.forName("ji").getMethod("a",Class.forName("bsr")).invoke(null,entity);List<String> out=new ArrayList<>();for(Object direction:ordered)out.add(""+((Enum<?>)direction).ordinal());return String.join(",",out)+"|"+((Enum<?>)Class.forName("bsr").getMethod("cH").invoke(entity)).ordinal();}

            case "dynamic": {
                int id=Integer.parseInt(f[1]),x=Integer.parseInt(f[2]),y=Integer.parseInt(f[3]),z=Integer.parseInt(f[4]);Object state=Class.forName("dfy").getMethod("a",int.class).invoke(null,id);Object pos=POS.getConstructor(int.class,int.class,int.class).newInstance(x,y,z);Class<?> ctx=Class.forName("exh");Object context;
                if(Boolean.parseBoolean(f[5]))context=ctx.getMethod("a").invoke(null);else {
                    Class<?> unsafeClass=Class.forName("sun.misc.Unsafe");Field uf=unsafeClass.getDeclaredField("theUnsafe");uf.setAccessible(true);Object unsafe=uf.get(null);boolean falling=Boolean.parseBoolean(f[9]);Object entity=unsafeClass.getMethod("allocateInstance",Class.class).invoke(unsafe,Class.forName(falling?"cjg":"aqv"));Class<?> entityClass=Class.forName("bsr");Field type=entityClass.getDeclaredField("n");type.setAccessible(true);type.set(entity,Class.forName("bsx").getField(falling?"O":"by").get(null));Field fall=entityClass.getField("ac");fall.setFloat(entity,Float.intBitsToFloat(Integer.parseUnsignedInt(f[8],16)));
                    if(!falling){Object inventory=Class.forName("cmw").getConstructor(Class.forName("cmx")).newInstance(entity);Field inv=Class.forName("cmx").getDeclaredField("h");inv.setAccessible(true);inv.set(entity,inventory);if(Boolean.parseBoolean(f[10])){Object boots=Class.forName("cuq").getConstructor(Class.forName("dcv")).newInstance(Class.forName("cut").getField("pB").get(null));Object armor=Class.forName("cmw").getField("i").get(inventory);((List<Object>)armor).set(0,boots);}}
                    Constructor<?> constructor=Class.forName("exm").getDeclaredConstructor(boolean.class,double.class,Class.forName("cuq"),Predicate.class,entityClass);constructor.setAccessible(true);context=constructor.newInstance(Boolean.parseBoolean(f[7]),d(f[6]),Class.forName("cuq").getField("l").get(null),(Predicate<Object>)(fluid->false),entity);
                }
                Object empty=Class.forName("dcl").getField("a").get(null);Object shape=state.getClass().getMethod("b",D,POS,ctx).invoke(state,empty,pos,context);shape=S.getMethod("a",double.class,double.class,double.class).invoke(shape,(double)x,(double)y,(double)z);List<String> out=new ArrayList<>();for(Object b:(List<?>)S.getMethod("e").invoke(shape))out.add(boxOut(b));return String.join(";",out);
            }

            case "static": {Object state=Class.forName("dfy").getMethod("a",int.class).invoke(null,Integer.parseInt(f[1]));Object block=state.getClass().getMethod("b").invoke(state);Field cache=Class.forName("dtb$a").getDeclaredField("a");cache.setAccessible(true);if((boolean)block.getClass().getMethod("p").invoke(block)||(boolean)state.getClass().getMethod("q").invoke(state)||cache.get(state)==null)return "dynamic";Object empty=Class.forName("dcl").getField("a").get(null);Object pos=POS.getConstructor(int.class,int.class,int.class).newInstance(0,0,0);Object shape=state.getClass().getMethod("k",D,POS).invoke(state,empty,pos);List<String> out=new ArrayList<>();for(Object b:(List<?>)S.getMethod("e").invoke(shape))out.add(boxOut(b));return String.join(";",out);}

            case "reach": {
                if(player==null){Class<?> unsafeClass=Class.forName("sun.misc.Unsafe");Field uf=unsafeClass.getDeclaredField("theUnsafe");uf.setAccessible(true);Object unsafe=uf.get(null);player=unsafeClass.getMethod("allocateInstance",Class.class).invoke(unsafe,Class.forName("aqv"));Object builder=Class.forName("cmx").getMethod("fM").invoke(null);Object supplier=builder.getClass().getMethod("a").invoke(builder);attributes=Class.forName("but").getConstructor(Class.forName("buv")).newInstance(supplier);Field af=Class.forName("btn").getDeclaredField("bU");af.setAccessible(true);af.set(player,attributes);}
                Field position=Class.forName("bsr").getDeclaredField("s");position.setAccessible(true);position.set(player,vec(f,1));Field eye=Class.forName("bsr").getDeclaredField("be");eye.setAccessible(true);eye.setFloat(player,Float.intBitsToFloat(Integer.parseUnsignedInt(f[4],16)));
                Object holder=Class.forName("buw").getField("g").get(null);Object instance=Class.forName("but").getMethod("a",Class.forName("jm")).invoke(attributes,holder);Class.forName("bus").getMethod("a",double.class).invoke(instance,d(f[8]));Object pos=POS.getConstructor(int.class,int.class,int.class).newInstance(Integer.parseInt(f[5]),Integer.parseInt(f[6]),Integer.parseInt(f[7]));return Class.forName("cmx").getMethod("a",POS,double.class).invoke(player,pos,d(f[9])).toString();
            }
            case "obstruct": {Object a=Class.forName("exs").getMethod("a",B).invoke(null,box(f,1));Object b=Class.forName("exs").getMethod("a",B).invoke(null,box(f,7));return Class.forName("exs").getMethod("c",S,S,Class.forName("exg")).invoke(null,a,b,Class.forName("exg").getField("i").get(null)).toString();}

            case "vec":{Object a=vec(f,1),b=vec(f,4);return vecOut(V.getMethod("d").invoke(a))+"|"+bits((double)V.getMethod("b",V).invoke(a,b))+"|"+vecOut(V.getMethod("c",V).invoke(a,b))+"|"+bits((double)V.getMethod("g",V).invoke(a,b))+"|"+vecOut(V.getMethod("a",V,double.class).invoke(a,b,d(f[7])));}
            case "box":{Object a=box(f,1),b=box(f,7),v=vec(f,13);return boxOut(a)+"|"+B.getMethod("c",B).invoke(a,b)+"|"+B.getMethod("d",V).invoke(a,v)+"|"+boxOut(B.getMethod("a",B).invoke(a,b))+"|"+boxOut(B.getMethod("b",B).invoke(a,b))+"|"+boxOut(B.getMethod("a",double.class,double.class,double.class).invoke(a,d(f[13]),d(f[14]),d(f[15])))+"|"+boxOut(B.getMethod("b",double.class,double.class,double.class).invoke(a,d(f[13]),d(f[14]),d(f[15])))+"|"+boxOut(B.getMethod("c",double.class,double.class,double.class).invoke(a,d(f[13]),d(f[14]),d(f[15])))+"|"+bits((double)B.getMethod("e",V).invoke(a,v))+"|"+vecOut(B.getMethod("f").invoke(a));}
            case "clip":{Object a=box(f,1),from=vec(f,7),to=vec(f,10);Optional<?> hit=(Optional<?>)B.getMethod("b",V,V).invoke(a,from,to);return hit.isEmpty()?"null":vecOut(hit.get());}
            case "dda":{Object from=vec(f,1),to=vec(f,4);List<String> cells=new ArrayList<>();D.getMethod("a",V,V,Object.class,BiFunction.class,Function.class).invoke(null,from,to,null,(BiFunction<Object,Object,Object>)((c,p)->{try{cells.add(POS.getMethod("u").invoke(p)+","+POS.getMethod("v").invoke(p)+","+POS.getMethod("w").invoke(p));if(cells.size()>4096)throw new IllegalStateException("unbounded ray");return null;}catch(Exception e){throw new RuntimeException(e);}}),(Function<Object,Object>)(c->null));return String.join(";",cells);}
            case "player":{Object dim=DIM.getMethod("b",float.class,float.class).invoke(null,Float.intBitsToFloat(Integer.parseUnsignedInt(f[4],16)),Float.intBitsToFloat(Integer.parseUnsignedInt(f[5],16)));return boxOut(DIM.getMethod("a",V).invoke(dim,vec(f,1)));}
            case "boxes": case "shapeclip": case "motion": case "collide":{
                int axis=Integer.parseInt(f[1]);double[][] coords=new double[3][];int[] n=new int[3];for(int a=0;a<3;a++){String[] values=f[2+a].split(",");coords[a]=new double[values.length];for(int i=0;i<values.length;i++)coords[a][i]=d(values[i]);n[a]=values.length-1;}
                Object grid=GRID.getConstructor(int.class,int.class,int.class).newInstance(n[0],n[1],n[2]);int k=0;for(int x=0;x<n[0];x++)for(int y=0;y<n[1];y++)for(int z=0;z<n[2];z++){if(f[5].charAt(k++)=='1')GRID.getMethod("c",int.class,int.class,int.class).invoke(grid,x,y,z);}
                Constructor<?> ctor=ARR.getDeclaredConstructor(Class.forName("exl"),double[].class,double[].class,double[].class);ctor.setAccessible(true);Object shape=ctor.newInstance(grid,coords[0],coords[1],coords[2]);
                if(f[0].equals("boxes")){List<String> out=new ArrayList<>();for(Object b:(List<?>)S.getMethod("e").invoke(shape))out.add(boxOut(b));return String.join(";",out);}
                if(f[0].equals("shapeclip")){Object p=POS.getConstructor(int.class,int.class,int.class).newInstance(Integer.parseInt(f[12]),Integer.parseInt(f[13]),Integer.parseInt(f[14]));Object hit=S.getMethod("a",V,V,POS).invoke(shape,vec(f,6),vec(f,9),p);if(hit==null)return "null";Class<?> hitClass=Class.forName("ewy");Object dir=hitClass.getMethod("b").invoke(hit);return vecOut(hitClass.getMethod("e").invoke(hit))+"|"+((Enum<?>)dir).ordinal()+"|"+hitClass.getMethod("d").invoke(hit);}
                if(f[0].equals("motion")){Method method=Class.forName("bsr").getDeclaredMethod("a",V,B,List.class);method.setAccessible(true);List<Object> shapes=List.of(shape,S.getMethod("a",double.class,double.class,double.class).invoke(shape,1.0,0.5,-1.0));return vecOut(method.invoke(null,vec(f,13),box(f,6),shapes));}
                return bits((double)S.getMethod("a",A,B,double.class).invoke(shape,axes[axis],box(f,6),d(f[12])));
            }
            default:throw new IllegalArgumentException(f[0]);
        }
    }
    public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null)try{OUT.println(run(line));}catch(InvocationTargetException e){throw new RuntimeException(line,e.getCause());}}
}
