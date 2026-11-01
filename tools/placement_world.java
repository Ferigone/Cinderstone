// Generated abstract-method plumbing only; original block methods perform placement.
class PlacementWorld extends dcw {
    PlacementWorld(){super(null,null,null,null,null,false,false,0L,0);}
    static final java.util.Map<String,dtc> BLOCKS=new java.util.HashMap<>();
    public java.util.List<bsr> a(bsr entity,ewx box,java.util.function.Predicate<? super bsr> predicate){return java.util.List.of();}
    public dtc a_(jd p){return BLOCKS.getOrDefault(p.u()+","+p.v()+","+p.w(),dfy.a(0));}
    public epe b_(jd p){return a_(p).u();}
    static final java.util.Set<String> POWER=new java.util.HashSet<>();
    static int MAX_HEIGHT=320,BRIGHTNESS=15;static boolean BORDER=true;
    public boolean C(jd p){return POWER.contains(p.u()+","+p.v()+","+p.w());}
    public dut C_(){return new PlacementBorder();}
    public int b(jd p,int darkness){return BRIGHTNESS;}
    public boolean a(dtc state,jd p,exh context){return true;} // Isolate state selector; geometry oracle separately checks entity obstruction.

    static final java.util.List<String> EFFECTS=new java.util.ArrayList<>();
    static jd ORIGIN;
    static String relative(jd p){return (p.u()-ORIGIN.u())+","+(p.v()-ORIGIN.v())+","+(p.w()-ORIGIN.w());}
    public boolean a(jd p,dtc state,int flags){BLOCKS.put(p.u()+","+p.v()+","+p.w(),state);EFFECTS.add("W:"+relative(p)+","+dfy.i(state)+","+flags);return true;}
    public void b(jd p,dfy source){EFFECTS.add("N:"+relative(p)+","+dfy.i(source.o()));}
    public void a(ji direction,dtc state,jd neighbor,jd origin,int flags,int recursion){EFFECTS.add("S:"+direction.ordinal()+","+relative(neighbor)+","+dfy.i(state)+","+relative(origin)+","+flags+","+recursion);}
    public void a(jd p,dfy source,int delay){EFFECTS.add("T:"+relative(p)+","+dfy.i(source.o())+","+delay);}
    public int I_(){return -64;} public int J_(){return MAX_HEIGHT+64;}
    public java.lang.String I(){throw new UnsupportedOperationException("unused placement world method: I");}
    public brc s(){throw new UnsupportedOperationException("unused placement world method: s");}
    public bsr a(int p0){throw new UnsupportedOperationException("unused placement world method: a");}
    public void a(int p0,jd p1,int p2){throw new UnsupportedOperationException("unused placement world method: a");}
    public void a(eqr p0,eqt p1){throw new UnsupportedOperationException("unused placement world method: a");}
    public eqt a(eqr p0){throw new UnsupportedOperationException("unused placement world method: a");}
    public void a(cmx p0,bsr p1,jm p2,avq p3,float p4,float p5,long p6){throw new UnsupportedOperationException("unused placement world method: a");}
    public void a(cmx p0,double p1,double p2,double p3,jm p4,avq p5,float p6,float p7,long p8){throw new UnsupportedOperationException("unused placement world method: a");}
    public void a(jd p0,dtc p1,dtc p2,int p3){throw new UnsupportedOperationException("unused placement world method: a");}
    public eqr v(){throw new UnsupportedOperationException("unused placement world method: v");}
    public czd r(){throw new UnsupportedOperationException("unused placement world method: r");}
    public eyg M(){throw new UnsupportedOperationException("unused placement world method: M");}
    public cwt K(){throw new UnsupportedOperationException("unused placement world method: K");}
    public eyp O(){throw new UnsupportedOperationException("unused placement world method: O");}
    public void a(cmx p0,int p1,jd p2,int p3){throw new UnsupportedOperationException("unused placement world method: a");}
    public void a(jm p0,exc p1,dxz.a p2){throw new UnsupportedOperationException("unused placement world method: a");}
    public dvc N(){throw new UnsupportedOperationException("unused placement world method: N");}
    public eyp P(){throw new UnsupportedOperationException("unused placement world method: P");}
    public java.util.List x(){throw new UnsupportedOperationException("unused placement world method: x");}
    public cpl J(){throw new UnsupportedOperationException("unused placement world method: J");}
    public jm a(int p0,int p1,int p2){throw new UnsupportedOperationException("unused placement world method: a");}
    public float a(ji p0,boolean p1){throw new UnsupportedOperationException("unused placement world method: a");}
    public dxp G(){throw new UnsupportedOperationException("unused placement world method: G");}
}

class PlacementBorder extends dut {public boolean a(jd p){return PlacementWorld.BORDER;}}
