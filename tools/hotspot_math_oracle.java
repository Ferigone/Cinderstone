import java.io.*;
import java.nio.file.*;
import java.util.*;
/** Exact Math.sin/cos bit oracle; runs only during differential validation. */
public final class hotspot_math_oracle {
    static volatile double sink;
    public static void main(String[] args)throws Exception {
        if(args.length>0&&args[0].equals("--exhaustive")){
            try(DataOutputStream out=new DataOutputStream(new BufferedOutputStream(System.out,1<<20))){for(int i=0;i<(1<<24);i++){double angle=(double)((i*0x1.0p-24f)*((float)Math.PI*2.0f));out.writeLong(Double.doubleToRawLongBits(Math.sin(angle)));out.writeLong(Double.doubleToRawLongBits(Math.cos(angle)));}}return;
        }
        ArrayList<Double> angles=new ArrayList<>(210000);
        angles.add(5.185734748840332);angles.add(0.0);angles.add(-0.0);angles.add((double)((float)Math.PI*2.0f));
        for(float value:new float[]{Float.MIN_VALUE,Float.MIN_NORMAL,Math.nextDown(Float.MIN_NORMAL),1e-30f,1e-20f,1e-10f})angles.add((double)value);
        int tauBits=Float.floatToRawIntBits((float)Math.PI*2.0f);
        for(int n=0;n<=64;n++){
            float point=(float)(n*Math.PI/32.0);
            for(float f:new float[]{Math.nextDown(point),point,Math.nextUp(point)})if(f>=0&&Float.floatToRawIntBits(f)<=tauBits)angles.add((double)f);
        }
        int boundary=angles.size();
        Random random=new Random(0x13198a2e03707344L);
        for(int i=0;i<120000;i++)angles.add((double)(random.nextFloat()*((float)Math.PI*2.0f)));
        for(int i=0;i<20000;){int bits=random.nextInt()&0x7fffffff;float value=Float.intBitsToFloat(bits);if(value>=0&&value<=(float)Math.PI*2.0f){angles.add((double)value);i++;}}
        int drops=0;
        for(String line:Files.readAllLines(Path.of(args[1]))){if(line.isEmpty())continue;String[] f=line.split(",");Random r=new Random(Long.parseLong(f[0]));if(Boolean.parseBoolean(f[1]))r.nextFloat();angles.add((double)(r.nextFloat()*((float)Math.PI*2.0f)));drops++;}
        for(int i=0;i<50000;i++)angles.add(random.nextDouble()*(Math.PI*2.0));
        // Warm the same call sites before recording: no StrictMath substitution.
        for(int round=0;round<8;round++){double sum=0;for(double x:angles)sum+=Math.sin(x)+Math.cos(x);sink=sum;}
        try(DataOutputStream out=new DataOutputStream(new BufferedOutputStream(Files.newOutputStream(Path.of(args[0]))))){
            out.writeBytes("MC21TRIG");out.writeInt(angles.size());
            for(double x:angles){out.writeLong(Double.doubleToRawLongBits(x));out.writeLong(Double.doubleToRawLongBits(Math.sin(x)));out.writeLong(Double.doubleToRawLongBits(Math.cos(x)));}
        }
        System.out.printf(Locale.ROOT,"{\"cases\":%d,\"float_random_angles\":140000,\"double_random_angles\":50000,\"boundary_angles\":%d,\"drop_seed_angles\":%d,\"runtime\":\"%s\",\"arch\":\"%s\"}%n",angles.size(),boundary,drops,System.getProperty("java.runtime.version"),System.getProperty("os.arch"));
    }
}
