package oracle;

import com.google.gson.Gson;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;

/** Pinned BucketItem contracts and original outline shapes, not test vectors. */
public final class bucket_interaction_facts {
    static Class<?> c(String n)throws Exception{return Class.forName(n);}
    static Method method(Class<?> type,String name,Class<?>[] args)throws Exception{
        while(type!=null){try{return type.getDeclaredMethod(name,args);}catch(NoSuchMethodException e){type=type.getSuperclass();}}throw new NoSuchMethodException(name);
    }
    static Object call(Object target,String name,Class<?>[] types,Object... args)throws Exception{
        Class<?> type=target instanceof Class<?>?(Class<?>)target:target.getClass();
        Method m=type.getMethod(name,types);m.setAccessible(true);return m.invoke(target instanceof Class<?>?null:target,args);
    }
    static List<double[]> boxes(Object shape)throws Exception{
        List<double[]> result=new ArrayList<>();for(Object box:(List<?>)call(shape,"e",new Class<?>[0])){
            double[] values=new double[6];for(int j=0;j<6;j++)values[j]=c("ewx").getField("abcdef".substring(j,j+1)).getDouble(box);result.add(values);
        }return result;
    }
    public static void main(String[] args)throws Exception{
        call(c("ab"),"a",new Class<?>[0]);call(c("akt"),"a",new Class<?>[0]);
        Object empty=c("dcl").getField("a").get(null),origin=c("jd").getConstructor(int.class,int.class,int.class).newInstance(0,0,0),context=call(c("exh"),"a",new Class<?>[0]);
        Object water=c("epf").getField("c").get(null),lava=c("epf").getField("e").get(null);
        Class<?>[] shapeTypes={c("dtc"),c("dcc"),c("jd"),c("exh")},interactionTypes={c("dtc"),c("dcc"),c("jd")};
        Class<?>[] pickupTypes={c("cmx"),c("dcx"),c("jd"),c("dtc")};
        List<Object> shapes=new ArrayList<>(),rows=new ArrayList<>();Map<String,Integer> unique=new LinkedHashMap<>();Set<String> owners=new TreeSet<>();
        for(int id=0;id<26684;id++){
            Object state=call(c("dfy"),"a",new Class<?>[]{int.class},id),block=call(state,"b",new Class<?>[0]);
            String owner=method(block.getClass(),"a",shapeTypes).getDeclaringClass().getName(),interaction=method(block.getClass(),"a",interactionTypes).getDeclaringClass().getName();owners.add(owner);
            int[] indexes={-1,-1};
            for(int n=0;n<2;n++){try{Object shape=n==0?call(state,"a",new Class<?>[]{c("dcc"),c("jd"),c("exh")},empty,origin,context):call(state,"m",new Class<?>[]{c("dcc"),c("jd")},empty,origin);List<double[]> b=boxes(shape);String key=new Gson().toJson(b);Integer index=unique.get(key);if(index==null){index=shapes.size();unique.put(key,index);shapes.add(b);}indexes[n]=index;}catch(InvocationTargetException e){}}
            String pickup=c("dgf").isInstance(block)?block.getClass().getMethod("a",pickupTypes).getDeclaringClass().getName():"";
            rows.add(Arrays.asList(indexes[0],owner,indexes[1],interaction,(boolean)call(state,"q",new Class<?>[0]),(boolean)call(state,"i",new Class<?>[0]),(boolean)call(state,"k",new Class<?>[0]),(boolean)call(state,"a",new Class<?>[]{c("epd")},water),(boolean)call(state,"a",new Class<?>[]{c("epd")},lava),pickup));
        }
        Object sounds=c("lt").getField("b").get(null);Map<String,Integer> soundIds=new TreeMap<>();
        for(String name:new String[]{"dc","df","di","dl","iU"})soundIds.put(name,(int)call(sounds,"a",new Class<?>[]{Object.class},c("avp").getField(name).get(null)));
        Map<String,Integer> bucketIds=new TreeMap<>();for(String name:new String[]{"qy","qz","qA"})bucketIds.put(name,(int)call(c("cul"),"a",new Class<?>[]{c("cul")},c("cut").getField(name).get(null)));
        Map<String,Integer> sourceIds=new TreeMap<>();for(Object fluid:new Object[]{water,lava}){Object fs=call(fluid,"g",new Class<?>[0]),bs=call(fs,"g",new Class<?>[0]);sourceIds.put(fluid==water?"water":"lava",(int)call(c("dfy"),"i",new Class<?>[]{c("dtc")},bs));}
        List<Integer> indexes=new ArrayList<>();List<Object> dedupRows=new ArrayList<>();Map<String,Integer> uniqueRows=new LinkedHashMap<>();for(Object row:rows){String key=new Gson().toJson(row);Integer index=uniqueRows.get(key);if(index==null){index=dedupRows.size();uniqueRows.put(key,index);dedupRows.add(row);}indexes.add(index);}
        Map<String,Object> out=new LinkedHashMap<>();out.put("minecraft","1.21");out.put("jar_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");out.put("shapes",shapes);out.put("states",indexes);out.put("rows",dedupRows);out.put("sounds",soundIds);out.put("bucket_ids",bucketIds);out.put("source_ids",sourceIds);Files.writeString(Path.of(args[0]),new Gson().toJson(out)+"\n");System.out.println("BUCKET_FACTS shapes="+shapes.size()+" rows="+dedupRows.size()+" owners="+owners+" buckets="+bucketIds+" sounds="+soundIds);
    }
}
