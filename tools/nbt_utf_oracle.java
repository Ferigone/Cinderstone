import java.io.*;
import java.util.*;

// The JDK produces the expected bytes; no Modified UTF-8 algorithm is copied here.
class NbtUtfOracle {
    static int[] units(String s) { int[] out=new int[s.length()];for(int i=0;i<s.length();i++)out[i]=s.charAt(i);return out; }
    static String text(int[] units) { char[] chars=new char[units.length];for(int i=0;i<units.length;i++)chars[i]=(char)units[i];return new String(chars); }
    static String hex(byte[] data) { StringBuilder out=new StringBuilder();for(byte b:data)out.append(String.format("%02x",b&255));return out.toString(); }
    static byte[] unhex(String s) { byte[] out=new byte[s.length()/2];for(int i=0;i<out.length;i++)out[i]=(byte)Integer.parseInt(s.substring(i*2,i*2+2),16);return out; }
    public static void main(String[] args) throws Exception {
        int[][] writes={{},{0},{65,0,66},{127,128,2047,2048,65535},{0xd83d,0xde00},{0xd800},{0xdc00},{0xd800,65,0xdc00},{0xd800,0xd800},{0xdc00,0xd800}};
        String[] reads={"000100","0002c081","0003e08081","0002c0af","0003eda080","0003edb080","0006eda0bdedb880","0004f0908080","0001c2","0002c241","0002eda0"};
        StringBuilder out=new StringBuilder("{\"source\":\"JDK DataOutputStream.writeUTF / DataInputStream.readUTF\",\"java\":\"").append(System.getProperty("java.version")).append("\",\"writes\":[");
        boolean first=true;
        for(int[] sample:writes){if(!first)out.append(',');first=false;
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();new DataOutputStream(bytes).writeUTF(text(sample));
            out.append("{\"units\":").append(Arrays.toString(sample)).append(",\"hex\":\"").append(hex(bytes.toByteArray())).append("\"}");
        }
        out.append("],\"reads\":[");first=true;
        for(String sample:reads){if(!first)out.append(',');first=false;
            out.append("{\"hex\":\"").append(sample).append("\"");
            try{String value=new DataInputStream(new ByteArrayInputStream(unhex(sample))).readUTF();out.append(",\"units\":").append(Arrays.toString(units(value)));}
            catch(UTFDataFormatException error){out.append(",\"error\":\"UTFDataFormatException\"");}
            out.append('}');
        }
        out.append("],\"floats\":[");first=true;
        for(int bits:new int[]{0,0x80000000,0x3f800000,0x7f800000,0xff800000,0x7fa01234,0x7fc04321,0xffa01234}){
            if(!first)out.append(',');first=false;
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();new DataOutputStream(bytes).writeFloat(Float.intBitsToFloat(bits));
            out.append("{\"input_bits\":\"").append(String.format("%08x",bits)).append("\",\"hex\":\"").append(hex(bytes.toByteArray())).append("\"}");
        }
        out.append("],\"doubles\":[");first=true;
        for(long bits:new long[]{0L,0x8000000000000000L,0x3ff0000000000000L,0x7ff0000000000000L,0xfff0000000000000L,0x7ff0000000000123L,0x7ff8000000004321L,0xfff0000000000123L}){
            if(!first)out.append(',');first=false;
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();new DataOutputStream(bytes).writeDouble(Double.longBitsToDouble(bits));
            out.append("{\"input_bits\":\"").append(String.format("%016x",bits)).append("\",\"hex\":\"").append(hex(bytes.toByteArray())).append("\"}");
        }
        out.append("]}");System.out.println(out);
    }
}
