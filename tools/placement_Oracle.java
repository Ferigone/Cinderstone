import java.io.*;
import java.lang.reflect.*;
import java.util.*;

/** The original block methods, with controlled world/context inputs only. */
class PlacementContextOracle extends cyd {
    PlacementContextOracle(){super(null,null,bqq.a,cuq.l,new ewy(exc.b,ji.b,new jd(0,0,0),false));}
    PlacementWorld world; jd target; ji face,horizontal,nearest; exc location; cuq stack;boolean replaceClicked,secondary;float rotation;ji[] ordered;
    public jd a(){return target;}
    public dcw q(){return world;}
    public ji k(){return face;}
    public ji g(){return horizontal;}
    public ji d(){return nearest;}
    public ji e(){return nearest.ordinal()%2==1?ji.b:ji.a;}
    public exc l(){return location;}
    public cuq n(){return stack;}
    public boolean c(){return replaceClicked;}
    public boolean h(){return secondary;}
    public float i(){return rotation;}
    public ji[] f(){return ordered;}
}
class PlacementOracle {
    static final PrintStream OUT=System.out,ERR=System.err;
    static sun.misc.Unsafe unsafe;
    static PlacementWorld world;
    static double d(String s){return Double.longBitsToDouble(Long.parseUnsignedLong(s,16));}
    static String run(String line)throws Exception{
        String[] f=line.split("\t",-1);int placing=Integer.parseInt(f[1]),current=Integer.parseInt(f[2]),x=Integer.parseInt(f[6]),y=Integer.parseInt(f[7]),z=Integer.parseInt(f[8]);
        PlacementWorld.BLOCKS.clear();PlacementWorld.BLOCKS.put(x+","+y+","+z,dfy.a(current));
        ji[] directions=ji.values();for(int i=0;i<6;i++){ji dir=directions[i];PlacementWorld.BLOCKS.put((x+dir.j())+","+(y+dir.k())+","+(z+dir.l()),dfy.a(Integer.parseInt(f[12+i])));}
        PlacementContextOracle context=(PlacementContextOracle)unsafe.allocateInstance(PlacementContextOracle.class);context.world=world;context.target=new jd(x,y,z);context.face=directions[Integer.parseInt(f[3])];context.horizontal=directions[Integer.parseInt(f[4])];context.nearest=directions[Integer.parseInt(f[5])];context.location=new exc(d(f[9]),d(f[10]),d(f[11]));context.stack=new cuq(f[0].equals("item")?cul.b(placing):dfy.a(placing).b().r());context.replaceClicked=Boolean.parseBoolean(f[18]);context.secondary=Boolean.parseBoolean(f[19]);context.rotation=Float.intBitsToFloat(Integer.parseUnsignedInt(f[24],16));String[] order=f[25].split(",");context.ordered=new ji[6];for(int i=0;i<6;i++)context.ordered[i]=directions[Integer.parseInt(order[i])];if(!context.replaceClicked){ji first=context.face.g();int index=Arrays.asList(context.ordered).indexOf(first);for(int i=index;i>0;i--)context.ordered[i]=context.ordered[i-1];context.ordered[0]=first;}
        PlacementWorld.POWER.clear();int power=Integer.parseInt(f[20]);if((power&1)!=0)PlacementWorld.POWER.add(x+","+y+","+z);if((power&2)!=0)PlacementWorld.POWER.add(x+","+(y+1)+","+z);PlacementWorld.BORDER=Boolean.parseBoolean(f[21]);PlacementWorld.MAX_HEIGHT=Integer.parseInt(f[22]);PlacementWorld.BRIGHTNESS=Integer.parseInt(f[23]);if(f.length>26){ji left=context.horizontal.i(),right=context.horizontal.h();PlacementWorld.BLOCKS.put((x+left.j())+","+(y+1)+","+(z+left.l()),dfy.a(Integer.parseInt(f[26])));PlacementWorld.BLOCKS.put((x+right.j())+","+(y+1)+","+(z+right.l()),dfy.a(Integer.parseInt(f[27])));}

        return switch(f[0]){case "plan"->{
        Method select=cso.class.getDeclaredMethod("c",cyd.class),write=cso.class.getDeclaredMethod("a",cyd.class,dtc.class);select.setAccessible(true);write.setAccessible(true);
        Object item=context.stack.g();dtc state=(dtc)select.invoke(item,context);if(state==null)yield "rejected";
        PlacementWorld.EFFECTS.clear();PlacementWorld.ORIGIN=context.target;write.invoke(item,context,state);
        state.b().a(world,context.target,state,null,context.stack);yield String.join("|",PlacementWorld.EFFECTS);
        }case "item"->{Method select=cso.class.getDeclaredMethod("c",cyd.class);select.setAccessible(true);dtc result=(dtc)select.invoke(cul.b(placing),context);yield result==null?"rejected":""+dfy.i(result);}case "survive"->""+dfy.a(placing).a((dcz)world,new jd(x,y,z));case "place"->{dtc result=dfy.a(placing).b().a(context);yield result==null?"rejected":""+dfy.i(result);}case "replace"->""+dfy.a(current).a(context);default->throw new IllegalArgumentException(f[0]);};
    }
    public static void main(String[]args)throws Exception{
        ab.a();akt.a();oracle.placement_tags.bind(args[0]);lt.c.a(Map.of(awk.a,List.of(lt.c.e(epf.c),lt.c.e(epf.d))));Field f=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");f.setAccessible(true);unsafe=(sun.misc.Unsafe)f.get(null);world=(PlacementWorld)unsafe.allocateInstance(PlacementWorld.class);ERR.println("ORACLE_CLASS=dfy,dml,dnt,dnc,BlockState;SOURCE="+dfy.class.getProtectionDomain().getCodeSource().getLocation());
        BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null)try{OUT.println(run(line));}catch(Throwable t){ERR.println(line);t.printStackTrace(ERR);System.exit(1);}
    }
}
