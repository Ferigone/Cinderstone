package oracle;
import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;

/** Extracts original BlockState collision shapes, retaining dynamic-state flags. */
class GeometryCollisionOracle {
    static final Gson GSON=new Gson();
    static Object call(Object receiver,String name,Class<?>[]types,Object...args)throws Exception {Class<?> c=receiver instanceof Class<?>?(Class<?>)receiver:receiver.getClass();Method m=null;Class<?> search=c;while(search!=null){try{m=search.getDeclaredMethod(name,types);break;}catch(NoSuchMethodException e){search=search.getSuperclass();}}if(m==null)throw new NoSuchMethodException(c+"."+name);m.setAccessible(true);return m.invoke(receiver instanceof Class<?>?null:receiver,args);}
    static Class<?> C(String n)throws Exception{return Class.forName(n);}
    static final Class<?>[] N={};
    static List<double[]> boxes(Object shape)throws Exception {List<double[]> boxes=new ArrayList<>();for(Object b:(List<?>)call(shape,"e",N)){double[] v=new double[6];for(int i=0;i<6;i++)v[i]=C("ewx").getField(""+(char)('a'+i)).getDouble(b);boxes.add(v);}return boxes;}
    static Object privateField(String cls,String name)throws Exception {Field f=C(cls).getDeclaredField(name);f.setAccessible(true);return f.get(null);}
    public static void main(String[]args)throws Exception{
        C("ab").getMethod("a").invoke(null);C("akt").getMethod("a").invoke(null);
        if(args.length>1){Field table=C("ayo").getDeclaredField("p");table.setAccessible(true);float[] values=(float[])table.get(null);java.nio.ByteBuffer buffer=java.nio.ByteBuffer.allocate(values.length*4);for(float value:values)buffer.putFloat(value);Files.write(Path.of(args[1]),buffer.array());}
        Object empty=C("dcl").getField("a").get(null),zero=C("jd").getConstructor(int.class,int.class,int.class).newInstance(0,0,0);
        Field cache=C("dtb$a").getDeclaredField("a");cache.setAccessible(true);
        List<List<double[]>> shapes=new ArrayList<>();List<int[]> rows=new ArrayList<>();Map<String,Integer> ids=new LinkedHashMap<>();List<String> dynamic=new ArrayList<>();List<Object> dynamicRows=new ArrayList<>();
        for(int id=0;id<26684;id++){
            Object state=call(C("dfy"),"a",new Class<?>[]{int.class},id),block=call(state,"b",N);
            Object shape=call(state,"k",new Class<?>[]{C("dcc"),C("jd")},empty,zero);
            List<double[]> boxes=new ArrayList<>();for(Object b:(List<?>)call(shape,"e",N)){double[] v=new double[6];for(int i=0;i<6;i++)v[i]=C("ewx").getField(""+(char)('a'+i)).getDouble(b);boxes.add(v);}
            String key=GSON.toJson(boxes);Integer found=ids.get(key);if(found==null){found=shapes.size();ids.put(key,found);shapes.add(boxes);}
            boolean hasDynamic=(boolean)call(block,"p",N),hasOffset=(boolean)call(state,"q",N),uncached=cache.get(state)==null;
            int flags=(hasDynamic?1:0)|(hasOffset?2:0)|(uncached?4:0);
            rows.add(new int[]{found,flags});if(flags!=0){
                dynamic.add(id+":"+state);String description=state.toString();String name=description.substring(description.indexOf("minecraft:"),description.indexOf('}'));
                int kind=name.equals("minecraft:bamboo")?2:name.equals("minecraft:pointed_dripstone")?3:name.equals("minecraft:scaffolding")?4:name.equals("minecraft:powder_snow")?5:name.endsWith("shulker_box")?6:name.equals("minecraft:moving_piston")?7:1;
                float horizontal=(float)call(block,"av_",N),vertical=(float)call(block,"ax_",N);Object offset=call(state,"n",new Class<?>[]{C("dcc"),C("jd")},empty,zero);double[] off={(double)call(offset,"a",N),(double)call(offset,"b",N),(double)call(offset,"c",N)};
                List<double[]> raw=new ArrayList<>();for(double[] box:boxes){double[] r=box.clone();for(int j=0;j<6;j++)r[j]-=off[j%3];raw.add(r);}
                int detail=0;if(kind==4){Object distance=privateField("dmo","c"),bottom=privateField("dmo","e");detail=(int)call(state,"c",new Class<?>[]{C("duf")},distance)*2+((boolean)call(state,"c",new Class<?>[]{C("duf")},bottom)?1:0);}
                dynamicRows.add(Arrays.asList(id,kind,detail,horizontal,vertical,off[1]!=0.,raw));
            }
        }
        JsonObject out=new JsonObject();out.addProperty("minecraft","1.21");out.addProperty("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");out.addProperty("description","state rows: deduplicated shape ID, flags(dynamic=1,offset=2,uncached=4). Shapes are original getCollisionShape(EmptyBlockGetter,BlockPos.ZERO) AABBs. Only flag0 is a static lookup; dynamic rows are evidence, not a context substitute.");out.add("shapes",GSON.toJsonTree(shapes));out.add("states",GSON.toJsonTree(rows));out.add("context_dependent",GSON.toJsonTree(dynamic));out.add("dynamic_rows",GSON.toJsonTree(dynamicRows));Map<String,Object> extra=new LinkedHashMap<>();for(String[] spec:new String[][]{{"SCAFFOLDING_STABLE","dmo","g"},{"SCAFFOLDING_BOTTOM","dmo","i"},{"SCAFFOLDING_BELOW","dmo","j"},{"POWDER_SNOW_FALLING","dlt","f"}})extra.put(spec[0],boxes(privateField(spec[1],spec[2])));out.add("special_shapes",GSON.toJsonTree(extra));Files.writeString(Path.of(args[0]),GSON.toJson(out)+"\n");System.out.println("ORACLE:"+rows.size()+" states, "+shapes.size()+" shapes, "+dynamic.size()+" dynamic/offset/uncached");
    }
}
