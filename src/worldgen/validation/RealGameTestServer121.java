package oracle;

import com.google.gson.*;
import com.mojang.serialization.Dynamic;
import java.lang.reflect.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.*;

/** External pinned-original integration harness. Compiling does not start it.
 * Uses original constructors, resources, world-data loader and server scheduler.
 * Does not accept an EULA, bind a listener, patch fields, fabricate statuses,
 * write DataVersion/blending metadata, or use Unsafe. Run only after approval.
 */
public final class RealGameTestServer121 {
    static final String INNER="8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
    static final Path CONTROLLED_ROOT=Path.of("/workspace/blending-region-harness/runs").toAbsolutePath().normalize();
    static final Gson JSON=new GsonBuilder().setPrettyPrinting().serializeNulls().create();
    static final CountDownLatch FINISHED=new CountDownLatch(1);
    static final AtomicInteger EXECUTOR_TASK_COUNT=new AtomicInteger();
    static final AtomicInteger GAME_TEST_CALLBACK_COUNT=new AtomicInteger();
    static volatile Throwable failure;
    static volatile Object server;
    static JsonObject config;
    static Path reportPath,worldPath;
    static Object loadedWorldData;
    static Class<?> C(String name)throws ClassNotFoundException{return Class.forName(name);}
    static Object field(Object owner,Class<?> type,String name)throws Exception {Field f=type.getDeclaredField(name);f.setAccessible(true);return f.get(owner);}
    static String sha(Path file)throws Exception{return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(file)));}
    static Throwable cause(Throwable error){while(error instanceof InvocationTargetException&&error.getCause()!=null)error=error.getCause();return error;}
    static String statusName(Object status)throws Exception{return C("dvz").getMethod("f").invoke(status).toString();}
    static void writeFailure(Throwable error){
        failure=cause(error);
        try{Files.writeString(reportPath,JSON.toJson(Map.of("phase","failed","minecraft","1.21","protocol",767,"error",failure.toString()))+"\n");}catch(Exception secondary){failure.addSuppressed(secondary);}
        failure.printStackTrace();FINISHED.countDown();
    }

    static Object loadWorldStem(Object access,Object packs,boolean upgrade)throws Exception {
        Class<?> packConfig=C("alo$d"),initConfig=C("alo$c"),dataConfig=C("ddr"),levelSettings=C("dda"),worldOptions=C("dzw"),registry=C("jz"),registryKey=C("akq"),frozen=C("ka$b");
        final Object existing;
        final Object actualPackConfig;
        final Object settings;
        if(upgrade){
            // getDataTag performs the original level-data read/datafix. The
            // source existing-world supplier below also decodes actual options
            // and dimensions. No supplied seed/version/generation flags replace it.
            existing=C("erf$c").getMethod("h").invoke(access);
            actualPackConfig=C("erf").getMethod("a",Dynamic.class,C("atp"),boolean.class).invoke(null,existing,packs,false);
            settings=null;
        }else{
            existing=null;
            C("atp").getMethod("a").invoke(packs);
            Object flags=C("cpn").getField("g").get(null);
            Object selected=C("dck").getConstructor(List.class,List.class).newInstance(new ArrayList<>((Collection<?>)C("atp").getMethod("b").invoke(packs)),List.of());
            Object dc=dataConfig.getConstructor(C("dck"),C("cpl")).newInstance(selected,flags);
            Object gameRules=C("dcs").getMethod("b").invoke(field(null,C("tc"),"t"));
            // This is an actual world-creation setting, before any world exists.
            // It reduces unrelated spawn preparation without falsifying statuses.
            Object spawnRadius=C("dcs").getMethod("a",C("dcs$e")).invoke(gameRules,C("dcs").getField("Z").get(null));
            C("dcs$d").getMethod("a",int.class,C("net.minecraft.server.MinecraftServer")).invoke(spawnRadius,0,null);
            settings=levelSettings.getConstructor(String.class,C("dct"),boolean.class,C("bqo"),boolean.class,C("dcs"),dataConfig)
                .newInstance("Original blending region harness",C("dct").getField("b").get(null),false,C("bqo").getField("c").get(null),true,gameRules,dc);
            actualPackConfig=packConfig.getConstructor(C("atp"),dataConfig,boolean.class,boolean.class).newInstance(packs,dc,false,true);
        }
        Object init=initConfig.getConstructor(packConfig,C("eu$a"),int.class).newInstance(actualPackConfig,C("eu$a").getField("b").get(null),4);
        Class<?> supplierType=C("alo$f"),factoryType=C("alo$e");
        Object supplier=Proxy.newProxyInstance(supplierType.getClassLoader(),new Class<?>[]{supplierType},(proxy,method,args)->{
            if(!method.getName().equals("get"))throw new UnsupportedOperationException(method.toString());
            Object context=args[0];Object worldgen=C("alo$a").getMethod("c").invoke(context),dimensions=C("alo$a").getMethod("d").invoke(context);
            Object dimensionRegistry=frozen.getMethod("d",registryKey).invoke(dimensions,C("lu").getField("bb").get(null));
            Object data,complete;
            if(existing!=null){
                Object dc=C("alo$a").getMethod("b").invoke(context);
                Object decoded=C("erf").getMethod("a",Dynamic.class,dataConfig,registry,frozen).invoke(null,existing,dc,dimensionRegistry,worldgen);
                data=C("erc").getMethod("a").invoke(decoded);complete=C("erc").getMethod("b").invoke(decoded);
            }else{
                Object normal=C("ejh").getMethod("a",C("ka")).invoke(null,worldgen);
                complete=C("dzt").getMethod("a",registry).invoke(normal,dimensionRegistry);
                Object options=worldOptions.getConstructor(long.class,boolean.class,boolean.class).newInstance(config.get("seed").getAsLong(),false,false);
                data=C("erj").getConstructor(levelSettings,worldOptions,C("erj$a"),C("com.mojang.serialization.Lifecycle"))
                    .newInstance(settings,options,C("dzt$b").getMethod("d").invoke(complete),C("dzt$b").getMethod("a").invoke(complete));
            }
            loadedWorldData=data;
            return C("alo$b").getConstructor(Object.class,frozen).newInstance(data,C("dzt$b").getMethod("b").invoke(complete));
        });
        Object factory=Proxy.newProxyInstance(factoryType.getClassLoader(),new Class<?>[]{factoryType},(proxy,method,args)->{
            if(!method.getName().equals("create"))throw new UnsupportedOperationException(method.toString());
            return C("alp").getConstructor(C("atu"),C("alc"),C("jt"),C("erl")).newInstance(args);
        });
        Function<Executor,CompletableFuture<Object>> load=executor->{try{
            @SuppressWarnings("unchecked") CompletableFuture<Object> result=(CompletableFuture<Object>)C("alo").getMethod("a",initConfig,supplierType,factoryType,Executor.class,Executor.class)
                .invoke(null,init,supplier,factory,C("ad").getMethod("g").invoke(null),executor);
            return result;
        }catch(Exception error){throw new CompletionException(cause(error));}};
        @SuppressWarnings("unchecked") CompletableFuture<Object> future=(CompletableFuture<Object>)C("ad").getMethod("c",Function.class).invoke(null,load);
        return future.get();
    }

    static Map<String,Object> collectActualRegion(Object level,Object cache,Object center,int cx,int cz)throws Exception {
        Object map=C("aqs").getField("a").get(cache);Class<?> initializerType=C("azi$a");List<Object> acquired=new ArrayList<>();
        Object initializer=Proxy.newProxyInstance(initializerType.getClassLoader(),new Class<?>[]{initializerType},(proxy,method,args)->{
            int x=(int)args[0],z=(int)args[1];long key=((x&0xffffffffL)|((z&0xffffffffL)<<32));
            Object holder=C("aqm").getMethod("d",long.class).invoke(map,key);acquired.add(holder);return holder;
        });
        try {
            Object dependencyCache=C("azi").getMethod("a",int.class,int.class,int.class,initializerType).invoke(null,cx,cz,8,initializer);
            Object step=C("dvy").getMethod("a",C("dvz")).invoke(C("dvy").getField("a").get(null),C("dvz").getField("f").get(null));
            Object region=C("arb").getConstructor(C("aqu"),C("azi"),C("dwc"),C("duy")).newInstance(level,dependencyCache,step,center);
            // No manufactured holder futures/statuses: missing real dependencies
            // throw through original WorldGenRegion instead of being supplied.
            Object blender=C("eaa").getMethod("a",C("arb")).invoke(null,region),empty=C("eaa").getMethod("a").invoke(null);
            Map<String,Object> result=new LinkedHashMap<>();result.put("actual_region_class",region.getClass().getName());result.put("actual_generation_holders_acquired",acquired.size());result.put("public_blender_of_executed",true);result.put("public_blender_empty_singleton",blender==empty);
            if(config.has("old_chunk")) {
                JsonArray q=config.getAsJsonArray("old_chunk");int x=q.get(0).getAsInt(),z=q.get(1).getAsInt();
                Object data=C("eab").getMethod("a",C("arb"),int.class,int.class).invoke(null,region,x,z);
                result.put("public_get_or_update_executed",true);result.put("old_data_absent",data==null);
                if(data!=null)result.put("old_data_calculated",field(data,C("eab"),"r"));
            }
            if(config.has("expect_nonempty")&&config.get("expect_nonempty").getAsBoolean()&&blender==empty)throw new AssertionError("genuine legacy collection unexpectedly returned empty Blender");
            return result;
        }finally {for(Object holder:acquired)C("aqm").getMethod("a",C("aqn")).invoke(map,holder);}
    }

    static void executeActualProbe(){
        try{
            Object level=C("net.minecraft.server.MinecraftServer").getMethod("I").invoke(server);Object cache=C("aqu").getMethod("l").invoke(level);
            Object actualServer=C("aqu").getMethod("o").invoke(level);
            if(actualServer!=server)throw new AssertionError("level belongs to another server");
            if(C("net.minecraft.server.MinecraftServer").getMethod("az").invoke(server)!=Thread.currentThread())throw new AssertionError("probe not running on actual server thread");
            boolean originalDevelopmentFlag=C("ab").getField("aV").getBoolean(null);
            if(originalDevelopmentFlag||EXECUTOR_TASK_COUNT.get()!=1||GAME_TEST_CALLBACK_COUNT.get()!=0)throw new AssertionError("unexpected original development mode or probe entry provenance");
            Object generator=C("aqs").getMethod("g").invoke(cache),randomState=C("aqs").getMethod("i").invoke(cache);
            if(!C("dzb").isInstance(generator))throw new AssertionError("actual normal generator is not NoiseBasedChunkGenerator: "+generator.getClass());
            int cx=config.has("chunk_x")?config.get("chunk_x").getAsInt():0,cz=config.has("chunk_z")?config.get("chunk_z").getAsInt():0;
            String phase=config.has("target_status")?config.get("target_status").getAsString():"noise";
            String field=switch(phase){case "biomes"->"f";case "noise"->"g";case "surface"->"h";default->throw new IllegalArgumentException("bounded target_status is biomes/noise/surface");};
            Object target=C("dvz").getField(field).get(null);
            // The actual ServerChunkCache schedules dependencies and original
            // ChunkSteps. The harness never completes futures or sets statuses.
            Object chunk=C("aqs").getMethod("a",int.class,int.class,C("dvz"),boolean.class).invoke(cache,cx,cz,target,true);
            Object noise=C("duy").getMethod("a",Function.class).invoke(chunk,(Function<Object,Object>)unused->{throw new AssertionError("source scheduler did not retain a NoiseChunk");});
            Object blender=C("dzc").getMethod("d").invoke(noise),empty=C("eaa").getMethod("a").invoke(null);
            Object connection=C("net.minecraft.server.MinecraftServer").getMethod("ai").invoke(server);
            List<?> listeners=(List<?>)field(connection,C("art"),"f");
            if(!listeners.isEmpty())throw new AssertionError("unexpected original network listener");
            Map<String,Object> result=new LinkedHashMap<>();result.put("minecraft","1.21");result.put("task_dispatch","MinecraftServer.execute after original public isReady");result.put("real_server_executor_task_executed",true);result.put("game_test_callback_executed",GAME_TEST_CALLBACK_COUNT.get()!=0);result.put("executor_task_count",EXECUTOR_TASK_COUNT.get());result.put("game_test_callback_count",GAME_TEST_CALLBACK_COUNT.get());result.put("original_development_flag",originalDevelopmentFlag);result.put("same_original_server_thread",true);result.put("protocol",767);result.put("oracle_inner_sha256",INNER);result.put("bootstrap","original WorldLoader + normal WorldStem + original GameTestServer constructor + MinecraftServer.spin");
            result.put("world_data_mode",config.get("mode").getAsString());result.put("actual_level_class",level.getClass().getName());result.put("actual_cache_class",cache.getClass().getName());result.put("actual_random_state_class",randomState.getClass().getName());result.put("actual_generator_class",generator.getClass().getName());
            result.put("world_seed",C("aqu").getMethod("C").invoke(level));result.put("chunk",List.of(cx,cz));result.put("requested_status",phase);result.put("persisted_status",statusName(C("duy").getMethod("j").invoke(chunk)));result.put("highest_generated_status",statusName(C("duy").getMethod("k").invoke(chunk)));result.put("retained_noise_chunk_class",noise.getClass().getName());result.put("retained_blender_empty_singleton",blender==empty);result.put("actual_network_listener_count",listeners.size());
            if(config.has("expect_nonempty")&&config.get("expect_nonempty").getAsBoolean()&&blender==empty)throw new AssertionError("original scheduler did not retain nonempty legacy Blender");
            if(config.has("public_region_collection")&&config.get("public_region_collection").getAsBoolean())result.put("explicit_region_collection",collectActualRegion(level,cache,chunk,cx,cz));
            result.put("public_blender_call_counter_observed",false);result.put("source_backed_original_chunkstep_path",true);result.put("eula_file_written",Files.exists(Path.of("eula.txt"))||Files.exists(worldPath.resolve("eula.txt")));
            if(Boolean.TRUE.equals(result.get("eula_file_written")))throw new AssertionError("unexpected EULA file created");
            if(EXECUTOR_TASK_COUNT.get()!=1||GAME_TEST_CALLBACK_COUNT.get()!=0)throw new AssertionError("probe entry counts changed before report");
            Files.writeString(reportPath,JSON.toJson(result)+"\n");
            FINISHED.countDown();
        }catch(Throwable error){writeFailure(error);}
    }

    public static void main(String[]args)throws Exception {
        if(args.length!=2||!args[0].equals("--execute-approved"))throw new IllegalArgumentException("server execution is on hold; coordinator must explicitly run --execute-approved <external-run-config.json>");
        config=JsonParser.parseString(Files.readString(Path.of(args[1]))).getAsJsonObject();
        String mode=config.get("mode").getAsString();if(!mode.equals("fresh")&&!mode.equals("upgrade"))throw new IllegalArgumentException("mode must be fresh or upgrade");
        Path storageRoot=CONTROLLED_ROOT.resolve(config.get("storage_group").getAsString()).normalize();
        worldPath=storageRoot.resolve(config.get("world_name").getAsString()).normalize();
        if(!storageRoot.startsWith(CONTROLLED_ROOT)||!worldPath.startsWith(storageRoot)||worldPath.equals(storageRoot))throw new IllegalArgumentException("world path escapes controlled external harness storage");
        Files.createDirectories(storageRoot);reportPath=worldPath.resolveSibling(worldPath.getFileName()+"-1.21-report.json");
        if(mode.equals("upgrade")&&!Files.exists(worldPath.resolve("original-generation-provenance.json")))throw new IllegalStateException("upgrade requires the genuine external legacy-generation provenance record");
        if(mode.equals("fresh")&&Files.exists(worldPath.resolve("level.dat")))throw new IllegalStateException("fresh mode cannot replace existing world metadata");
        C("ab").getMethod("a").invoke(null);C("akt").getMethod("a").invoke(null);
        Path source=Path.of(C("tc").getProtectionDomain().getCodeSource().getLocation().toURI());if(!sha(source).equals(INNER))throw new AssertionError("wrong pinned original 1.21 server artifact");
        Object storage=C("erf").getMethod("b",Path.class).invoke(null,storageRoot),access=C("erf").getMethod("d",String.class).invoke(storage,config.get("world_name").getAsString());
        Object packs=C("ats").getMethod("c").invoke(null),stem=loadWorldStem(access,packs,mode.equals("upgrade"));
        // This registered callback never runs the observer. The production
        // profile must observe zero invocations; it is counted independently.
        Consumer<Object> test=unused->{GAME_TEST_CALLBACK_COUNT.incrementAndGet();};
        Object function=C("tr").getConstructor(String.class,String.class,String.class,int.class,long.class,boolean.class,Consumer.class)
            .newInstance("original_region","original_region.bootstrap","minecraft:igloo/top",1200,0L,true,test);
        Object spawn=C("jd").getConstructor(int.class,int.class,int.class).newInstance(0,80,0);
        Constructor<?> constructor=C("tc").getDeclaredConstructor(Thread.class,C("erf$c"),C("atp"),C("alp"),Collection.class,C("jd"));constructor.setAccessible(true);
        Function<Thread,Object> factory=thread->{try{return constructor.newInstance(thread,access,packs,stem,List.of(function),spawn);}catch(Exception error){throw new CompletionException(cause(error));}};
        server=C("net.minecraft.server.MinecraftServer").getMethod("a",Function.class).invoke(null,factory);
        long readyDeadline=System.nanoTime()+TimeUnit.SECONDS.toNanos(100);
        while(!(boolean)C("net.minecraft.server.MinecraftServer").getMethod("aj").invoke(server)) {
            if(!(boolean)C("net.minecraft.server.MinecraftServer").getMethod("x").invoke(server)||System.nanoTime()>=readyDeadline)throw new IllegalStateException("real server did not become ready within source bootstrap deadline");
            Thread.sleep(10);
        }
        Runnable task=()->{try{
            if(EXECUTOR_TASK_COUNT.incrementAndGet()!=1)throw new IllegalStateException("executor observer is one-shot");
            executeActualProbe();
        }catch(Throwable error){writeFailure(error);}finally{try{C("net.minecraft.server.MinecraftServer").getMethod("a",boolean.class).invoke(server,false);}catch(Exception error){writeFailure(error);}}};
        C("net.minecraft.server.MinecraftServer").getMethod("execute",Runnable.class).invoke(server,task);
        if(!FINISHED.await(100,TimeUnit.SECONDS)){failure=new TimeoutException("original server probe exceeded its bounded executor-task deadline");writeFailure(failure);C("net.minecraft.server.MinecraftServer").getMethod("a",boolean.class).invoke(server,true);throw new RuntimeException(failure);}
        if(failure!=null){C("net.minecraft.server.MinecraftServer").getMethod("a",boolean.class).invoke(server,true);throw new RuntimeException(failure);}
        // Actual GameTestServer owns orderly shutdown/save and its process exit.
    }
}
