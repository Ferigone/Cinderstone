import java.io.*;import java.lang.reflect.*;import java.util.*;
/** Executes unchanged Player.updatePlayerPose with controlled state/fit accessors. */
class entity_pose_oracle {
 static class Player extends cmx {
  int flags,mask,pose;List<Integer> calls;
  Player(){super(null,jd.c,0,null);}
  public boolean R_(){return (flags&64)!=0;}
  public boolean f(){return false;}
  public boolean fA(){return (flags&1)!=0;}
  public boolean fH(){return (flags&2)!=0;}
  public boolean cd(){return (flags&4)!=0;}
  public boolean fn(){return (flags&8)!=0;}
  public boolean bW(){return (flags&16)!=0;}
  public boolean bS(){return (flags&128)!=0;}
  protected boolean h(bua pose){calls.add(pose.a());return (mask&(1<<pose.a()))!=0;}
  public void b(bua pose){this.pose=pose.a();}
  void update(){fR();}
 }
 public static void main(String[]args)throws Exception{
  ab.a();akt.a();Field field=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);sun.misc.Unsafe unsafe=(sun.misc.Unsafe)field.get(null);
  Player player=(Player)unsafe.allocateInstance(Player.class);cmu abilities=new cmu();Field a=cmx.class.getDeclaredField("cA");a.setAccessible(true);a.set(player,abilities);
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));
  for(String line;(line=input.readLine())!=null;){String[]f=line.split("\\t");player.flags=Integer.parseInt(f[0]);player.mask=Integer.parseInt(f[1]);player.pose=Integer.parseInt(f[2]);player.calls=new ArrayList<>();abilities.b=(player.flags&32)!=0;player.update();System.out.println("ORACLE:"+player.pose+"|"+String.join(",",player.calls.stream().map(Object::toString).toList()));}
 }
}
