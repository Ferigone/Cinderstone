package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.stream.Stream;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;
import jdk.internal.org.objectweb.asm.Type;

/** Invokes unchanged signed aru.handleMovePlayer. Only oracle.* context adapters
 * are generated. Entity.move, teleport and world effects are controlled callbacks;
 * branch decisions and numeric operations execute in the original handler.
 */
public final class player_movement_oracle {
    static final Gson JSON=new GsonBuilder().serializeNulls().create();
    static final PrintStream OUT=System.out;
    static Class<?> cls(String n)throws Exception{return Class.forName(n);}
    static Class<?>[] types(Class<?>...t){return t;}
    static Object call(Object target,String name,Class<?>[] t,Object...a)throws Exception{return survival_oracle.call(target,name,t,a);}
    static void set(Object o,String owner,String name,Object value)throws Exception{survival_oracle.set(o,owner,name,value);}
    static Object unsafe;static Method allocate;
    static Class<?> PLAYER,WORLD,SERVER,LISTENER,CHUNKS;
    static Object player,world,server,listener,chunks,rate,rules,abilities,gameMode;
    static JsonObject query;static JsonArray events;static JsonObject output;
    static List<Object> collisionShapes;
    static Object alloc(Class<?> c)throws Exception{return allocate.invoke(unsafe,c);}
    static boolean flag(String n){return query.has(n)&&query.get(n).getAsBoolean();}
    static int integer(String n,int fallback){return query.has(n)?query.get(n).getAsInt():fallback;}
    static String db(double v){return String.format("%016x",Double.doubleToLongBits(v));}
    static String fb(float v){return String.format("%08x",Float.floatToIntBits(v));}
    static double d(JsonElement v){return Double.longBitsToDouble(Long.parseUnsignedLong(v.getAsString(),16));}
    static float f(JsonElement v){return Float.intBitsToFloat(Integer.parseUnsignedInt(v.getAsString(),16));}
    static Object vec(double x,double y,double z)throws Exception{return cls("exc").getConstructor(double.class,double.class,double.class).newInstance(x,y,z);}
    static Object vec(JsonArray a)throws Exception{return vec(d(a.get(0)),d(a.get(1)),d(a.get(2)));}
    static double[] xyz(Object v)throws Exception{return new double[]{cls("exc").getField("c").getDouble(v),cls("exc").getField("d").getDouble(v),cls("exc").getField("e").getDouble(v)};}
    static JsonArray bits(double...a){JsonArray r=new JsonArray();for(double v:a)r.add(db(v));return r;}
    static Object get(Object o,String owner,String n)throws Exception{Field field=cls(owner).getDeclaredField(n);field.setAccessible(true);return field.get(o);}
    static JsonArray fieldVector(String owner,String...n)throws Exception{JsonArray out=new JsonArray();for(String f:n)out.add(db((double)get(listener,owner,f)));return out;}
    static void event(String name){events.add(name);}
    public static void disconnect(Object reason)throws Exception {event("disconnect");output.addProperty("disconnect",(String)cls("wz").getMethod("getString").invoke(reason));}
    public static void teleport(double x,double y,double z,float yaw,float pitch)throws Exception {event("teleport");JsonObject t=new JsonObject();t.add("position",bits(x,y,z));JsonArray r=new JsonArray();r.add(fb(yaw));r.add(fb(pitch));t.add("rotation",r);output.add("teleport",t);position(x,y,z,yaw,pitch);}
    static void position(double x,double y,double z,float yaw,float pitch)throws Exception {
        Object before=get(player,"bsr","s");double[] p=before==null?new double[]{x,y,z}:xyz(before);
        Object bb=get(player,"bsr","aF");if(bb!=null)set(player,"bsr","aF",call(bb,"d",types(double.class,double.class,double.class),x-p[0],y-p[1],z-p[2]));
        set(player,"bsr","s",vec(x,y,z));PLAYER.getField("yaw").setFloat(player,yaw);PLAYER.getField("pitch").setFloat(player,pitch);
    }
    public static void moveTo(double x,double y,double z,float yaw,float pitch)throws Exception {event("move_to");position(x,y,z,yaw,pitch);}
    public static void move(Object type,Object requested)throws Exception {
        event("move");double[] request=xyz(requested);output.add("move_delta",bits(request));
        Object resolved=query.has("resolved_delta")?vec(query.getAsJsonArray("resolved_delta")):requested;
        double[] delta=xyz(resolved),old=xyz(get(player,"bsr","s"));
        position(old[0]+delta[0],old[1]+delta[1],old[2]+delta[2],PLAYER.getField("yaw").getFloat(player),PLAYER.getField("pitch").getFloat(player));
    }
    public static void jump()throws Exception{event("jump");if(query.has("jump_vertical_collision_below"))set(player,"bsr","S",flag("jump_vertical_collision_below"));}
    public static void ground(boolean value,Object delta)throws Exception {event("set_ground");PLAYER.getField("ground").setBoolean(player,value);output.add("ground_delta",bits(xyz(delta)));}
    public static void fall(double x,double y,double z,boolean ground){event("fall");output.add("fall_delta",bits(x,y,z));output.addProperty("fall_ground",ground);}
    public static void known(Object delta)throws Exception{event("known_movement");output.add("known_delta",bits(xyz(delta)));}
    public static void resetFall(){event("reset_fall");}
    public static void resetImpulse(){event("reset_impulse");}
    public static void stats(double x,double y,double z){event("statistics");output.add("statistics_delta",bits(x,y,z));}
    public static void chunks(Object p){event("chunks");}
    public static void playerTick()throws Exception {event("player_tick");if(query.has("tick_player_position")){JsonArray p=query.getAsJsonArray("tick_player_position"),r=query.has("tick_player_rotation")?query.getAsJsonArray("tick_player_rotation"):null;position(d(p.get(0)),d(p.get(1)),d(p.get(2)),r==null?PLAYER.getField("yaw").getFloat(player):f(r.get(0)),r==null?PLAYER.getField("pitch").getFloat(player):f(r.get(1)));}}
    public static void commonTick(){event("common_tick");}
    public static void sendPacket(Object packet)throws Exception {
        event("send_packet");JsonObject p=new JsonObject();p.addProperty("class",packet.getClass().getName());
        if(cls("aec").isInstance(packet)){p.add("position",bits((double)call(packet,"b",types()),(double)call(packet,"e",types()),(double)call(packet,"f",types())));JsonArray r=new JsonArray();r.add(fb((float)call(packet,"g",types())));r.add(fb((float)call(packet,"h",types())));p.add("rotation",r);p.addProperty("id",(int)call(packet,"i",types()));int mask=0;for(Object relative:(Set<?>)call(packet,"j",types()))mask|=(int)call(relative,"a",types());p.addProperty("relative_mask",mask);}
        else if(cls("abz").isInstance(packet))p.addProperty("sequence",(int)call(packet,"b",types()));
        if(cls("aec").isInstance(packet)||cls("abz").isInstance(packet)){
            Class<?> byteBuf=cls("io.netty.buffer.ByteBuf");Object storage=cls("io.netty.buffer.Unpooled").getMethod("buffer").invoke(null);Object friendly=cls("vw").getConstructor(byteBuf).newInstance(storage);
            Method write=packet.getClass().getDeclaredMethod("a",cls("vw"));write.setAccessible(true);write.invoke(packet,friendly);int size=(int)byteBuf.getMethod("readableBytes").invoke(storage);byte[] data=new byte[size];byteBuf.getMethod("getBytes",int.class,byte[].class).invoke(storage,0,data);p.addProperty("wire",(cls("aec").isInstance(packet)?"40":"05")+HexFormat.of().formatHex(data));byteBuf.getMethod("release").invoke(storage);
        }
        if(!output.has("packets"))output.add("packets",new JsonArray());output.getAsJsonArray("packets").add(p);
    }
    public static boolean oldCollision(Object p,Object b){event("old_no_collision");return flag("old_no_collision");}
    public static Object collisions(Object p,Object b)throws Exception {event("new_collision_query");List<Object> filtered=player_physics_oracle.filter(collisionShapes,b);output.addProperty("new_collision_candidates",filtered.size());return filtered;}
    public static Object blockStates(Object bounds)throws Exception {event("air_query");return Stream.of(call(cls("dfy"),"a",types(int.class),flag("air")?0:1));}
    static void forward(ClassWriter c,String name,String desc,String hook){
        MethodVisitor m=survival_oracle.method(c,name,desc);Type[] args=Type.getArgumentTypes(desc);int local=1;StringBuilder target=new StringBuilder("(");
        for(Type t:args){m.visitVarInsn(t.getOpcode(ILOAD),local);local+=t.getSize();target.append(t.getSort()==Type.OBJECT||t.getSort()==Type.ARRAY?"Ljava/lang/Object;":t.getDescriptor());}
        Type ret=Type.getReturnType(desc);target.append(')').append(ret.getSort()==Type.OBJECT||ret.getSort()==Type.ARRAY?"Ljava/lang/Object;":ret.getDescriptor());
        m.visitMethodInsn(INVOKESTATIC,"oracle/player_movement_oracle",hook,target.toString(),false);
        if(ret.getSort()==Type.OBJECT||ret.getSort()==Type.ARRAY)m.visitTypeInsn(CHECKCAST,ret.getInternalName());
        m.visitInsn(ret.getOpcode(IRETURN));survival_oracle.end(m);
    }
    static void build()throws Exception {
        survival_oracle loader=new survival_oracle();String s="oracle/MovementServer";ClassWriter c=survival_oracle.begin(s,"net/minecraft/server/MinecraftServer");survival_oracle.getterConst(c,"bx","()Z",ICONST_1,IRETURN);survival_oracle.field(c,"flightAllowed","Z");survival_oracle.getter(c,s,"ae","()Z","flightAllowed","Z",IRETURN);SERVER=survival_oracle.define(loader,s,c);
        String ch="oracle/MovementChunks";c=survival_oracle.begin(ch,"aqs");forward(c,"a","(Laqv;)V","chunks");CHUNKS=survival_oracle.define(loader,ch,c);
        String w="oracle/MovementWorld";c=survival_oracle.begin(w,"aqu");for(String[] row:new String[][]{{"server","Lnet/minecraft/server/MinecraftServer;","o"},{"rate","Lbrc;","s"},{"rules","Ldcs;","ab"},{"chunks","Laqs;","l"}}){survival_oracle.field(c,row[0],row[1]);survival_oracle.getter(c,w,row[2],"()"+row[1],row[0],row[1],ARETURN);}forward(c,"a","(Lbsr;Lewx;)Z","oldCollision");forward(c,"d","(Lbsr;Lewx;)Ljava/lang/Iterable;","collisions");forward(c,"a","(Lewx;)Ljava/util/stream/Stream;","blockStates");WORLD=survival_oracle.define(loader,w,c);
        String p="oracle/MovementPlayer";c=survival_oracle.begin(p,"aqv");
        for(String[] row:new String[][]{{"world","Laqu;","A"},{"velocity","Lexc;","dr"},{"abilities","Lcmu;","fZ"},{"name","Lwz;","ah"},{"yaw","F","dE"},{"pitch","F","dG"},{"passenger","Z","bS"},{"sleep","Z","fH"},{"elytra","Z","fA"},{"ground","Z","aF"},{"spin","Z","fn"},{"liquid","Z","dX"},{"climb","Z","p_"},{"spectator","Z","R_"},{"dead","Z","ex"},{"gravity","D","aZ"},{"no_gravity","Z","aY"}}){survival_oracle.field(c,row[0],row[1]);survival_oracle.getter(c,p,row[2],"()"+row[1],row[0],row[1],row[1].equals("Z")?IRETURN:row[1].equals("F")?FRETURN:row[1].equals("D")?DRETURN:ARETURN);}
        survival_oracle.field(c,"levitation","Z");survival_oracle.getter(c,p,"b","(Ljm;)Z","levitation","Z",IRETURN);
        MethodVisitor self=survival_oracle.method(c,"cZ","()Lbsr;");self.visitVarInsn(ALOAD,0);self.visitInsn(ARETURN);survival_oracle.end(self);survival_oracle.getterConst(c,"N","()J",LCONST_0,LRETURN);
        forward(c,"a","(DDDFF)V","moveTo");forward(c,"a","(Lbts;Lexc;)V","move");forward(c,"ff","()V","jump");forward(c,"a","(ZLexc;)V","ground");forward(c,"a","(DDDZ)V","fall");forward(c,"b","(Lexc;)V","known");forward(c,"n","()V","resetFall");forward(c,"gB","()V","resetImpulse");forward(c,"b","(DDD)V","stats");forward(c,"m","()V","playerTick");PLAYER=survival_oracle.define(loader,p,c);
        String l="oracle/MovementListener";c=survival_oracle.begin(l,"aru");survival_oracle.field(c,"owner","Z");survival_oracle.getter(c,l,"h","()Z","owner","Z",IRETURN);forward(c,"a","(Lwz;)V","disconnect");forward(c,"a","(DDDFF)V","teleport");forward(c,"b","(Lzg;)V","sendPacket");forward(c,"e","()V","commonTick");LISTENER=survival_oracle.define(loader,l,c);
    }
    static void init()throws Exception {call(cls("ab"),"a",types());call(cls("akt"),"a",types());Class<?> u=cls("sun.misc.Unsafe");Field uf=u.getDeclaredField("theUnsafe");uf.setAccessible(true);unsafe=uf.get(null);allocate=u.getMethod("allocateInstance",Class.class);build();OUT.println("SOURCE:"+cls("aru").getProtectionDomain().getCodeSource().getLocation());}
    static void initQuery(JsonObject q)throws Exception {
        query=q;events=new JsonArray();output=new JsonObject();player=alloc(PLAYER);world=alloc(WORLD);server=alloc(SERVER);listener=alloc(LISTENER);chunks=alloc(CHUNKS);rate=cls("brc").getConstructor().newInstance();rules=cls("dcs").getConstructor().newInstance();abilities=cls("cmu").getConstructor().newInstance();gameMode=alloc(cls("aqw"));
        for(String[] row:new String[][]{{"server","server"},{"rate","rate"},{"rules","rules"},{"chunks","chunks"}})WORLD.getField(row[0]).set(world,switch(row[1]){case"server"->server;case"rate"->rate;case"rules"->rules;default->chunks;});
        PLAYER.getField("world").set(player,world);PLAYER.getField("abilities").set(player,abilities);PLAYER.getField("velocity").set(player,vec(q.getAsJsonArray("velocity")));PLAYER.getField("name").set(player,call(cls("wz"),"b",types(String.class),"PhysicsOracle"));
        for(String flag:new String[]{"passenger","sleep","elytra","ground","spin","liquid","climb","spectator","levitation","dead","no_gravity"})PLAYER.getField(flag).setBoolean(player,flag(flag));
        set(player,"aqv","cX",flag("changing"));PLAYER.getField("gravity").setDouble(player,q.has("gravity")?d(q.get("gravity")):.08);
        SERVER.getField("flightAllowed").setBoolean(server,flag("flight_allowed"));LISTENER.getField("owner").setBoolean(listener,flag("owner"));set(rate,"brc","e",flag("runs_normally"));
        set(player,"bsr","S",flag("vertical_collision_below"));set(player,"bsr","ag",flag("no_physics"));set(player,"aqv","h",flag("won"));set(player,"bsr","r",world);set(player,"aqv","e",gameMode);set(gameMode,"aqw","b",cls("dct").getEnumConstants()[integer("game_mode",flag("spectator")?3:0)]);set(abilities,"cmu","c",flag("may_fly"));
        Object rule=call(rules,"a",types(cls("dcs$e")),cls("dcs").getField("t").get(null));call(rule,"a",types(boolean.class,cls("net.minecraft.server.MinecraftServer")),flag("elytra_rule"),null);
        set(listener,"aru","f",player);set(player,"aqv","c",listener);set(listener,"arr","d",server);set(listener,"aru","o",integer("tick",1));set(listener,"aru","M",integer("received",0));set(listener,"aru","N",integer("known",0));set(listener,"aru","H",integer("teleport_tick",0));set(listener,"aru","G",integer("teleport_id",0));set(listener,"aru","p",integer("block_ack",-1));set(listener,"aru","I",flag("floating"));set(listener,"aru","J",integer("above_ground_ticks",0));
        if(q.has("pending")&&!q.get("pending").isJsonNull())set(listener,"aru","F",vec(q.getAsJsonArray("pending")));
        for(String[] row:new String[][]{{"first_good","s","t","u"},{"last_good","v","w","x"}}){JsonArray a=q.getAsJsonArray(row[0]);for(int i=0;i<3;i++)set(listener,"aru",row[i+1],d(a.get(i)));}
        JsonArray p=q.getAsJsonArray("current"),r=q.getAsJsonArray("current_rotation");position(d(p.get(0)),d(p.get(1)),d(p.get(2)),f(r.get(0)),f(r.get(1)));
        Object bounds=cls("ewx").getConstructor(double.class,double.class,double.class,double.class,double.class,double.class).newInstance(d(p.get(0))-(double)(.6f/2f),d(p.get(1)),d(p.get(2))-(double)(.6f/2f),d(p.get(0))+(double)(.6f/2f),d(p.get(1))+(double)1.8f,d(p.get(2))+(double)(.6f/2f));set(player,"bsr","aF",bounds);
        collisionShapes=new ArrayList<>();if(q.has("shapes"))for(JsonElement a:q.getAsJsonArray("shapes"))collisionShapes.add(player_physics_oracle.shape(a.getAsJsonArray()));
    }
    static Object packet(JsonObject p)throws Exception {JsonArray pos=p.getAsJsonArray("position"),rot=p.getAsJsonArray("rotation");boolean ground=p.get("ground").getAsBoolean();return switch(p.get("kind").getAsString()){
        case"position"->cls("ahg$a").getConstructor(double.class,double.class,double.class,boolean.class).newInstance(d(pos.get(0)),d(pos.get(1)),d(pos.get(2)),ground);
        case"rotation"->cls("ahg$c").getConstructor(float.class,float.class,boolean.class).newInstance(f(rot.get(0)),f(rot.get(1)),ground);
        case"ground"->cls("ahg$d").getConstructor(boolean.class).newInstance(ground);
        case"both"->cls("ahg$b").getConstructor(double.class,double.class,double.class,float.class,float.class,boolean.class).newInstance(d(pos.get(0)),d(pos.get(1)),d(pos.get(2)),f(rot.get(0)),f(rot.get(1)),ground);
        default->throw new IllegalArgumentException(p.toString());};}
    static JsonObject run(JsonObject q)throws Exception {initQuery(q);call(listener,"a",types(cls("ahg")),packet(q.getAsJsonObject("packet")));output.add("events",events);output.add("position",bits(xyz(get(player,"bsr","s"))));JsonArray rot=new JsonArray();rot.add(fb(PLAYER.getField("yaw").getFloat(player)));rot.add(fb(PLAYER.getField("pitch").getFloat(player)));output.add("rotation",rot);output.addProperty("received",(int)get(listener,"aru","M"));output.addProperty("known",(int)get(listener,"aru","N"));output.addProperty("teleport_tick",(int)get(listener,"aru","H"));output.addProperty("floating",(boolean)get(listener,"aru","I"));output.add("first_good",fieldVector("aru","s","t","u"));output.add("last_good",fieldVector("aru","v","w","x"));output.addProperty("ground",PLAYER.getField("ground").getBoolean(player));return output;}
    static JsonObject bookkeepingState()throws Exception {
        JsonObject s=new JsonObject();s.add("events",events.deepCopy());s.add("position",bits(xyz(get(player,"bsr","s"))));JsonArray rot=new JsonArray();rot.add(fb(PLAYER.getField("yaw").getFloat(player)));rot.add(fb(PLAYER.getField("pitch").getFloat(player)));s.add("rotation",rot);Object pending=get(listener,"aru","F");s.add("pending",pending==null?JsonNull.INSTANCE:bits(xyz(pending)));for(String[] row:new String[][]{{"tick","o"},{"received","M"},{"known","N"},{"teleport_tick","H"},{"teleport_id","G"},{"above_ground_ticks","J"},{"block_ack","p"}})s.addProperty(row[0],(int)get(listener,"aru",row[1]));s.addProperty("floating",(boolean)get(listener,"aru","I"));s.addProperty("changing",(boolean)get(player,"aqv","cX"));s.add("first_good",fieldVector("aru","s","t","u"));s.add("last_good",fieldVector("aru","v","w","x"));if(output.has("packets"))s.add("packets",output.get("packets").deepCopy());if(output.has("disconnect"))s.add("disconnect",output.get("disconnect"));return s;
    }
    static JsonArray runBookkeeping(JsonObject q)throws Exception {initQuery(q);JsonArray states=new JsonArray();for(JsonElement e:q.getAsJsonArray("steps")){JsonObject step=e.getAsJsonObject();events=new JsonArray();output=new JsonObject();switch(step.get("op").getAsString()){
        case"teleport":{JsonArray p=step.getAsJsonArray("position"),r=step.getAsJsonArray("rotation");Set<Object> relatives=new HashSet<>();int mask=step.has("relative_mask")?step.get("relative_mask").getAsInt():0;Object[] values=cls("buc").getEnumConstants();for(int i=0;i<5;i++)if((mask&(1<<i))!=0)relatives.add(values[i]);call(listener,"a",types(double.class,double.class,double.class,float.class,float.class,Set.class),d(p.get(0)),d(p.get(1)),d(p.get(2)),f(r.get(0)),f(r.get(1)),relatives);break;}
        case"ack":call(listener,"a",types(cls("agk")),cls("agk").getConstructor(int.class).newInstance(step.get("id").getAsInt()));break;
        case"reset":call(listener,"l",types());break;
        case"tick":call(listener,"d",types());break;
        default:throw new IllegalArgumentException(step.toString());}states.add(bookkeepingState());}return states;}
    public static void main(String[]args)throws Exception {init();BufferedReader in=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=in.readLine())!=null;)try{JsonObject q=JsonParser.parseString(line).getAsJsonObject();OUT.println("ORACLE:"+JSON.toJson(q.has("op")&&q.get("op").getAsString().equals("bookkeeping")?runBookkeeping(q):run(q)));}catch(Throwable e){OUT.println("QUERY:"+line);e.printStackTrace(OUT);throw e;}}
}
