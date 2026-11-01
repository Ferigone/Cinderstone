/*
 *  Copyright (c) 2016, 2021, Intel Corporation. All rights reserved.
 * Intel Math Library (LIBM) Source Code
 * Copyright (c) 2022, Oracle and/or its affiliates. All rights reserved.
 *
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 *
 */
//! Scalar transcription of HotSpot x86_64 Intel LIBM's bounded sin/cos path.
//!
//! Preserves each SSE2 arithmetic operation's evaluation order; no fma or
//! platform libm/JVM calls are used. Validated against the pinned Microsoft
//! OpenJDK21.0.13.0.2+1 x86_64 runtime. Other JVM/CPU implementations can choose
//! different last-bit results. The public domain is finite [0, f32::TAU], plus
//! negative zero, which covers Minecraft1.21 Player.drop's f32 jitter angles.
//!
//! Source: microsoft/openjdk-jdk21u commit ccb06ed8ec75927d88ff859a9198a7ea72f698b6,
//! stubGenerator_x86_64_{sin,cos,constants}.cpp. Source headers and GPLv2 notice
//! are retained in tools/hotspot_math_sources and licenses/hotspot-math-GPL2.txt.

#[inline]
fn d(bits: u64) -> f64 {
    f64::from_bits(bits)
}
const SC_1: [u64; 2] = [0xbfc5555555555555, 0xbfe0000000000000];
const SC_2: [u64; 2] = [0x3f81111111111111, 0x3fa5555555555555];
const SC_3: [u64; 2] = [0xbf2a01a01a01a01a, 0xbf56c16c16c16c17];
const SC_4: [u64; 2] = [0x3ec71de3a556c734, 0x3efa01a01a01a01a];
const PI32INV: u64 = 0x40245f306dc9c883;
const P_1: u64 = 0x3fb921fb54400000;
const P_2: u64 = 0x3d90b4611a600000;
const P_3: u64 = 0x3b63198a2e037073;
const TABLE: [[u64; 4]; 64] = [
    [
        0x0000000000000000,
        0x0000000000000000,
        0x0000000000000000,
        0x3ff0000000000000,
    ],
    [
        0xbf73b92e176d6d31,
        0x3fb917a6bc29b42c,
        0xbc3e2718e0000000,
        0x3ff0000000000000,
    ],
    [
        0xbf93ad06011469fb,
        0x3fc8f8b83c69a60b,
        0xbc626d19c0000000,
        0x3ff0000000000000,
    ],
    [
        0xbfa60bea939d225a,
        0x3fd294062ed59f06,
        0xbc75d28da0000000,
        0x3ff0000000000000,
    ],
    [
        0xbfb37ca1866b95cf,
        0x3fd87de2a6aea963,
        0xbc672cede0000000,
        0x3ff0000000000000,
    ],
    [
        0xbfbe3a6873fa1279,
        0x3fde2b5d3806f63b,
        0x3c5e0d8920000000,
        0x3ff0000000000000,
    ],
    [
        0xbfc592675bc57974,
        0x3fe1c73b39ae68c8,
        0x3c8b25dd20000000,
        0x3ff0000000000000,
    ],
    [
        0xbfcd0dfe53aba2fd,
        0x3fe44cf325091dd6,
        0x3c68076a20000000,
        0x3ff0000000000000,
    ],
    [
        0x3fca827999fcef32,
        0x3fe6a09e667f3bcd,
        0xbc8bdd3420000000,
        0x3fe0000000000000,
    ],
    [
        0x3fc133cc94247758,
        0x3fe8bc806b151741,
        0xbc82c5e120000000,
        0x3fe0000000000000,
    ],
    [
        0x3fac73b39ae68c87,
        0x3fea9b66290ea1a3,
        0x3c39f630e0000000,
        0x3fe0000000000000,
    ],
    [
        0xbf9d4a2c7f909c4e,
        0x3fec38b2f180bdb1,
        0xbc76e0b180000000,
        0x3fe0000000000000,
    ],
    [
        0xbfbe087565455a75,
        0x3fed906bcf328d46,
        0x3c7457e620000000,
        0x3fe0000000000000,
    ],
    [
        0x3fa4a03176acf82d,
        0x3fee9f4156c62dda,
        0x3c8760b1e0000000,
        0x3fd0000000000000,
    ],
    [
        0xbfac1d1f0e5967d5,
        0x3fef6297cff75cb0,
        0x3c75621720000000,
        0x3fd0000000000000,
    ],
    [
        0xbf9ba1650f592f50,
        0x3fefd88da3d12526,
        0xbc887df640000000,
        0x3fc0000000000000,
    ],
    [
        0x0000000000000000,
        0x3ff0000000000000,
        0x0000000000000000,
        0x0000000000000000,
    ],
    [
        0x3f9ba1650f592f50,
        0x3fefd88da3d12526,
        0xbc887df640000000,
        0xbfc0000000000000,
    ],
    [
        0x3fac1d1f0e5967d5,
        0x3fef6297cff75cb0,
        0x3c75621720000000,
        0xbfd0000000000000,
    ],
    [
        0xbfa4a03176acf82d,
        0x3fee9f4156c62dda,
        0x3c8760b1e0000000,
        0xbfd0000000000000,
    ],
    [
        0x3fbe087565455a75,
        0x3fed906bcf328d46,
        0x3c7457e620000000,
        0xbfe0000000000000,
    ],
    [
        0x3f9d4a2c7f909c4e,
        0x3fec38b2f180bdb1,
        0xbc76e0b180000000,
        0xbfe0000000000000,
    ],
    [
        0xbfac73b39ae68c87,
        0x3fea9b66290ea1a3,
        0x3c39f630e0000000,
        0xbfe0000000000000,
    ],
    [
        0xbfc133cc94247758,
        0x3fe8bc806b151741,
        0xbc82c5e120000000,
        0xbfe0000000000000,
    ],
    [
        0xbfca827999fcef32,
        0x3fe6a09e667f3bcd,
        0xbc8bdd3420000000,
        0xbfe0000000000000,
    ],
    [
        0x3fcd0dfe53aba2fd,
        0x3fe44cf325091dd6,
        0x3c68076a20000000,
        0xbff0000000000000,
    ],
    [
        0x3fc592675bc57974,
        0x3fe1c73b39ae68c8,
        0x3c8b25dd20000000,
        0xbff0000000000000,
    ],
    [
        0x3fbe3a6873fa1279,
        0x3fde2b5d3806f63b,
        0x3c5e0d8920000000,
        0xbff0000000000000,
    ],
    [
        0x3fb37ca1866b95cf,
        0x3fd87de2a6aea963,
        0xbc672cede0000000,
        0xbff0000000000000,
    ],
    [
        0x3fa60bea939d225a,
        0x3fd294062ed59f06,
        0xbc75d28da0000000,
        0xbff0000000000000,
    ],
    [
        0x3f93ad06011469fb,
        0x3fc8f8b83c69a60b,
        0xbc626d19c0000000,
        0xbff0000000000000,
    ],
    [
        0x3f73b92e176d6d31,
        0x3fb917a6bc29b42c,
        0xbc3e2718e0000000,
        0xbff0000000000000,
    ],
    [
        0x0000000000000000,
        0x0000000000000000,
        0x0000000000000000,
        0xbff0000000000000,
    ],
    [
        0x3f73b92e176d6d31,
        0xbfb917a6bc29b42c,
        0x3c3e2718e0000000,
        0xbff0000000000000,
    ],
    [
        0x3f93ad06011469fb,
        0xbfc8f8b83c69a60b,
        0x3c626d19c0000000,
        0xbff0000000000000,
    ],
    [
        0x3fa60bea939d225a,
        0xbfd294062ed59f06,
        0x3c75d28da0000000,
        0xbff0000000000000,
    ],
    [
        0x3fb37ca1866b95cf,
        0xbfd87de2a6aea963,
        0x3c672cede0000000,
        0xbff0000000000000,
    ],
    [
        0x3fbe3a6873fa1279,
        0xbfde2b5d3806f63b,
        0xbc5e0d8920000000,
        0xbff0000000000000,
    ],
    [
        0x3fc592675bc57974,
        0xbfe1c73b39ae68c8,
        0xbc8b25dd20000000,
        0xbff0000000000000,
    ],
    [
        0x3fcd0dfe53aba2fd,
        0xbfe44cf325091dd6,
        0xbc68076a20000000,
        0xbff0000000000000,
    ],
    [
        0xbfca827999fcef32,
        0xbfe6a09e667f3bcd,
        0x3c8bdd3420000000,
        0xbfe0000000000000,
    ],
    [
        0xbfc133cc94247758,
        0xbfe8bc806b151741,
        0x3c82c5e120000000,
        0xbfe0000000000000,
    ],
    [
        0xbfac73b39ae68c87,
        0xbfea9b66290ea1a3,
        0xbc39f630e0000000,
        0xbfe0000000000000,
    ],
    [
        0x3f9d4a2c7f909c4e,
        0xbfec38b2f180bdb1,
        0x3c76e0b180000000,
        0xbfe0000000000000,
    ],
    [
        0x3fbe087565455a75,
        0xbfed906bcf328d46,
        0xbc7457e620000000,
        0xbfe0000000000000,
    ],
    [
        0xbfa4a03176acf82d,
        0xbfee9f4156c62dda,
        0xbc8760b1e0000000,
        0xbfd0000000000000,
    ],
    [
        0x3fac1d1f0e5967d5,
        0xbfef6297cff75cb0,
        0xbc75621720000000,
        0xbfd0000000000000,
    ],
    [
        0x3f9ba1650f592f50,
        0xbfefd88da3d12526,
        0x3c887df640000000,
        0xbfc0000000000000,
    ],
    [
        0x0000000000000000,
        0xbff0000000000000,
        0x0000000000000000,
        0x0000000000000000,
    ],
    [
        0xbf9ba1650f592f50,
        0xbfefd88da3d12526,
        0x3c887df640000000,
        0x3fc0000000000000,
    ],
    [
        0xbfac1d1f0e5967d5,
        0xbfef6297cff75cb0,
        0xbc75621720000000,
        0x3fd0000000000000,
    ],
    [
        0x3fa4a03176acf82d,
        0xbfee9f4156c62dda,
        0xbc8760b1e0000000,
        0x3fd0000000000000,
    ],
    [
        0xbfbe087565455a75,
        0xbfed906bcf328d46,
        0xbc7457e620000000,
        0x3fe0000000000000,
    ],
    [
        0xbf9d4a2c7f909c4e,
        0xbfec38b2f180bdb1,
        0x3c76e0b180000000,
        0x3fe0000000000000,
    ],
    [
        0x3fac73b39ae68c87,
        0xbfea9b66290ea1a3,
        0xbc39f630e0000000,
        0x3fe0000000000000,
    ],
    [
        0x3fc133cc94247758,
        0xbfe8bc806b151741,
        0x3c82c5e120000000,
        0x3fe0000000000000,
    ],
    [
        0x3fca827999fcef32,
        0xbfe6a09e667f3bcd,
        0x3c8bdd3420000000,
        0x3fe0000000000000,
    ],
    [
        0xbfcd0dfe53aba2fd,
        0xbfe44cf325091dd6,
        0xbc68076a20000000,
        0x3ff0000000000000,
    ],
    [
        0xbfc592675bc57974,
        0xbfe1c73b39ae68c8,
        0xbc8b25dd20000000,
        0x3ff0000000000000,
    ],
    [
        0xbfbe3a6873fa1279,
        0xbfde2b5d3806f63b,
        0xbc5e0d8920000000,
        0x3ff0000000000000,
    ],
    [
        0xbfb37ca1866b95cf,
        0xbfd87de2a6aea963,
        0x3c672cede0000000,
        0x3ff0000000000000,
    ],
    [
        0xbfa60bea939d225a,
        0xbfd294062ed59f06,
        0x3c75d28da0000000,
        0x3ff0000000000000,
    ],
    [
        0xbf93ad06011469fb,
        0xbfc8f8b83c69a60b,
        0x3c626d19c0000000,
        0x3ff0000000000000,
    ],
    [
        0xbf73b92e176d6d31,
        0xbfb917a6bc29b42c,
        0x3c3e2718e0000000,
        0x3ff0000000000000,
    ],
];

/// Return (sin(angle), cos(angle)) with the pinned x86_64 HotSpot kernel's bits.
/// Panics for inputs outside this bounded implementation's documented domain.
pub fn sin_cos(angle: f64) -> (f64, f64) {
    assert!(
        angle.is_finite() && angle >= 0. && angle <= f64::from(std::f32::consts::TAU),
        "HotSpot bounded trig requires finite [0,f32::TAU]"
    );
    if angle == 0. || angle < d(0x3030000000000000) {
        return (angle, 1.);
    }
    (kernel(angle, 0), kernel(angle, 16))
}

#[inline]
fn kernel(x: f64, phase: usize) -> f64 {
    let n = (x * d(PI32INV) + 0.5) as i32;
    let nf = f64::from(n);
    let p1 = nf * d(P_1);
    let p2 = nf * d(P_2);
    let p3 = nf * d(P_3);
    let r1 = x - p1;
    let r = r1 - p2;
    let c1 = r1 - r;
    let c2 = c1 - p2;
    let neg_c = p3 - c2;
    let [c_hl, s_hi, s_lo, sigma] = TABLE[(n as usize + phase) & 63].map(d);
    let c_plus_sigma = c_hl + sigma;
    let neg_d = s_hi * r - c_plus_sigma;
    let c_times_r = c_plus_sigma * r;
    let rs = sigma * r;
    let med = r * c_hl;
    let mut sc4 = [d(SC_4[0]) * r1, d(SC_4[1]) * r1];
    sc4[0] *= r;
    sc4[1] *= r;
    let r2 = r * r;
    let products = [c_times_r * r2, s_hi * r2];
    let r4 = r2 * r2;
    let mut sc2 = [d(SC_2[0]) * r2, d(SC_2[1]) * r2];
    sc4[0] += d(SC_3[0]);
    sc4[1] += d(SC_3[1]);
    sc2[0] += d(SC_1[0]);
    sc2[1] += d(SC_1[1]);
    sc4[0] *= r4;
    sc4[1] *= r4;
    sc2[0] += sc4[0];
    sc2[1] += sc4[1];
    let poly = [sc2[0] * products[0], sc2[1] * products[1]];
    let res_int = rs + s_hi;
    let res_hi = med + res_int;
    let k0 = s_hi - res_int;
    let k1 = res_int - res_hi;
    let k2 = rs + k0;
    let k3 = k1 + med;
    let corr = neg_c * neg_d + s_lo;
    let mut lo = k2 + corr;
    lo += k3;
    lo += poly[0];
    lo += poly[1];
    lo + res_hi
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formerly_one_ulp_cos_disagreement_matches_hotspot() {
        assert_eq!(sin_cos(5.185734748840332).1.to_bits(), 4601883784787796961);
    }
    #[test]
    fn exact_pinned_hotspot_oracle_corpus() {
        let data = include_bytes!("../fixtures/hotspot_math_vectors.bin");
        assert_eq!(&data[..8], b"MC21TRIG");
        let count = u32::from_be_bytes(data[8..12].try_into().unwrap()) as usize;
        assert!(count >= 100000);
        assert_eq!(data.len(), 12 + count * 24);
        let (records, remainder) = data[12..].as_chunks::<24>();
        assert!(remainder.is_empty());
        for (i, record) in records.iter().enumerate() {
            let bits = |off| u64::from_be_bytes(record[off..off + 8].try_into().unwrap());
            let angle = f64::from_bits(bits(0));
            let (sin, cos) = sin_cos(angle);
            assert_eq!(sin.to_bits(), bits(8), "case{i},sin({angle:?})");
            assert_eq!(cos.to_bits(), bits(16), "case{i},cos({angle:?})");
        }
    }
}
