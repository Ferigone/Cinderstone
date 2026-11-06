// SPDX-License-Identifier: GPL-2.0-only
// Invokes unchanged 1.21 WorldBorder and ClientboundInitializeBorderPacket.
package oracle;
import com.google.gson.*;
import io.netty.buffer.Unpooled;
import java.lang.reflect.*;
import java.util.HexFormat;

public class border_login_oracle {
    static long now;
    public static void main(String[] args) throws Exception {
        Class.forName("ab").getMethod("a").invoke(null);
        Class.forName("akt").getMethod("a").invoke(null);
        Class<?> source = Class.forName("azo$a");
        Object clock = Proxy.newProxyInstance(source.getClassLoader(), new Class<?>[]{source},
            (proxy, method, values) -> {
                if (method.getName().equals("getAsLong")) return now * 1_000_000L;
                throw new UnsupportedOperationException(method.toString());
            });
        Class.forName("ad").getField("c").set(null, clock);
        Class<?> borderClass = Class.forName("dut"), packetClass = Class.forName("ade");
        Class<?> bufferClass = Class.forName("vw");
        Method write = packetClass.getDeclaredMethod("a", bufferClass);
        write.setAccessible(true);
        // x,z,from,to,duration,sample-time,absolute-limit,warning-blocks,warning-time
        double[][] cases = {
            {0,0,59999968,59999968,0,0,29999984,5,15},
            {12.25,-7.5,200,200,0,0,29999984,7,23},
            {12.25,-7.5,200,100,1000,250,29999984,7,23},
            {0,0,100,200,1000,1000,29999984,5,15},
            {0,0,100,200,1000,1001,29999984,5,15},
            {-4,8,64,128,100000,1,123456,-1,-2}
        };
        JsonArray output = new JsonArray();
        for (double[] c : cases) {
            now = 0;
            Object border = borderClass.getConstructor().newInstance();
            borderClass.getMethod("c",double.class,double.class).invoke(border,c[0],c[1]);
            borderClass.getMethod("a",double.class).invoke(border,c[2]);
            borderClass.getMethod("a",int.class).invoke(border,(int)c[6]);
            borderClass.getMethod("c",int.class).invoke(border,(int)c[7]);
            borderClass.getMethod("b",int.class).invoke(border,(int)c[8]);
            if (c[4] != 0) borderClass.getMethod("a",double.class,double.class,long.class)
                .invoke(border,c[2],c[3],(long)c[4]);
            now = (long)c[5];
            Object packet = packetClass.getConstructor(borderClass).newInstance(border);
            var buffer = Unpooled.buffer();
            try {
                Object wrapped = bufferClass.getConstructor(io.netty.buffer.ByteBuf.class).newInstance(buffer);
                write.invoke(packet,wrapped);
                byte[] bytes = new byte[buffer.readableBytes()]; buffer.readBytes(bytes);
                JsonObject row = new JsonObject();
                String[] names={"x","z","from","to","duration","now","absolute","warning_blocks","warning_time"};
                for(int i=0;i<names.length;i++) {
                    if(i>=4) row.addProperty(names[i],(long)c[i]);
                    else row.addProperty(names[i],c[i]);
                }
                row.addProperty("payload_hex",HexFormat.of().formatHex(bytes));
                output.add(row);
            } finally { buffer.release(); }
        }
        System.out.println("BORDER_PACKETS:"+new Gson().toJson(output));
    }
}
