package oracle;
import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;

/** Extract immutable fluid and cached FULL support-face facts from the unchanged
 * signed Minecraft1.21 BlockState registry. Uncached support faces stay unknown. */
public final class fluid_physics_facts {
 static final Gson JSON=new Gson();
 static Class<?> C(String n)throws Exception {return Class.forName(n);}
 static Class<?>[] T(Class<?>...t){return t;}
 static Object call(Object receiver,String name,Class<?>[]types,Object...args)throws Exception {
  Class<?> c=receiver instanceof Class<?>?(Class<?>)receiver:receiver.getClass();Method m=null;
  try{m=c.getMethod(name,types);}catch(NoSuchMethodException e){for(Class<?> s=c;s!=null;s=s.getSuperclass())try{m=s.getDeclaredMethod(name,types);break;}catch(NoSuchMethodException ignored){}}
  if(m==null)throw new NoSuchMethodException(c+"."+name+Arrays.toString(types));m.setAccessible(true);return m.invoke(receiver instanceof Class<?>?null:receiver,args);
 }
 public static void main(String[] args)throws Exception {
  call(C("ab"),"a",T());call(C("akt"),"a",T());
  Object empty=C("dcl").getField("a").get(null),origin=C("jd").getConstructor(int.class,int.class,int.class).newInstance(0,0,0);
  Object[] directions=C("ji").getEnumConstants();String[] names={"down","up","north","south","west","east"};
  // Direction enum order is part of the serialized face-bit contract.
  for(int i=0;i<6;i++)if(!directions[i].toString().equals(names[i]))throw new AssertionError("direction order");
  Field cache=C("dtb$a").getDeclaredField("a");cache.setAccessible(true);
  Object falling=C("epc").getField("a").get(null);
  List<Object> rows=new ArrayList<>();List<Integer> states=new ArrayList<>();Map<String,Integer> indices=new LinkedHashMap<>();
  int[] families=new int[3];int cached=0,uncached=0,ice=0;List<Integer> unknownStates=new ArrayList<>();
  Map<String,Integer> unknownBlocks=new TreeMap<>();Map<String,Integer> unknownSupportClasses=new TreeMap<>();
  for(int id=0;id<26684;id++){
   Object state=call(C("dfy"),"a",T(int.class),id),block=call(state,"b",T()),fluid=call(state,"u",T()),type=call(fluid,"a",T());
   boolean isEmpty=(boolean)call(fluid,"c",T());int family=isEmpty?0:C("epk").isInstance(type)?1:C("eph").isInstance(type)?2:-1;
   if(family<0)throw new AssertionError("unknown exact1.21 fluid family "+type);
   long height=Integer.toUnsignedLong(Float.floatToRawIntBits((float)call(fluid,"d",T())));
   int amount=(int)call(fluid,"e",T());boolean isFalling=!isEmpty&&(boolean)call(fluid,"c",T(C("duf")),falling),source=(boolean)call(fluid,"b",T()),blocksMotion=(boolean)call(state,"d",T()),iceBlock=C("djw").isInstance(block),cacheAvailable=cache.get(state)!=null;
   int sturdy=0;if(cacheAvailable){cached++;for(int dir=0;dir<6;dir++)if((boolean)call(state,"d",T(C("dcc"),C("jd"),C("ji")),empty,origin,directions[dir]))sturdy|=1<<dir;}else{
    uncached++;unknownStates.add(id);String desc=state.toString(),name=desc.substring(desc.indexOf("minecraft:"),desc.indexOf('}'));unknownBlocks.merge(name,1,Integer::sum);
    Method support=null;for(Class<?> s=block.getClass();s!=null;s=s.getSuperclass())try{support=s.getDeclaredMethod("b_",C("dtc"),C("dcc"),C("jd"));break;}catch(NoSuchMethodException ignored){}
    unknownSupportClasses.merge(support==null?"unknown":support.getDeclaringClass().getName(),1,Integer::sum);
   }
   List<Object> row=Arrays.asList(family,height,amount,isFalling,source,blocksMotion,iceBlock,sturdy,cacheAvailable);String key=JSON.toJson(row);Integer index=indices.get(key);if(index==null){index=rows.size();indices.put(key,index);rows.add(row);}states.add(index);families[family]++;if(iceBlock)ice++;
  }
  Map<String,Object> out=new LinkedHashMap<>();out.put("minecraft","1.21");out.put("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");
  out.put("description","rows=[fluidkind0empty1water2lava,ownHeight_f32rawbits,amount_i32,falling,source,blocksMotion,iceBlock,cachedFaceSturdy6bits,faceCacheAvailable]. Face order DOWN,UP,NORTH,SOUTH,WEST,EAST; FULL support type. faceCacheAvailable=false means uncached support geometry needs the real world; its bits0 are not a false-face answer.");
  out.put("states",states);out.put("rows",rows);out.put("family_state_counts",families);out.put("cached_face_states",cached);out.put("uncached_face_states",uncached);out.put("ice_block_states",ice);out.put("uncached_state_ids",unknownStates);out.put("uncached_blocks",unknownBlocks);out.put("uncached_support_classes",unknownSupportClasses);
  Files.writeString(Path.of(args[0]),JSON.toJson(out)+"\n");
  System.out.println("EXTRACT:"+states.size()+" states,"+rows.size()+" deduplicated rows,"+cached+" cached-face states,"+uncached+" unknown-face states");
 }
}
