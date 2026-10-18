//! Firmware expf/logf at 0x08029888 / 0x08029970. Avoid host libm drift.
const ROM: &[u8; 616] = include_bytes!("../data/math-rom.bin");
fn double(offset: usize) -> f64 {
    f64::from_le_bytes(ROM[offset..offset + 8].try_into().unwrap())
}
#[inline]
pub fn exp(x: f32) -> f32 {
    if x == f32::NEG_INFINITY {
        return 0.;
    }
    if !x.is_finite() {
        return x + x;
    }
    if x > 88.72283 {
        return f32::INFINITY;
    }
    if x < -103.97208 {
        return 0.;
    }
    let rounded = double(0x128).mul_add(x as f64, double(0x120));
    let u = rounded.to_bits() as u32;
    let r = double(0x128).mul_add(x as f64, -(rounded - double(0x120)));
    let poly = double(0x130).mul_add(r, double(0x138));
    let y = poly.mul_add(r * r, double(0x140).mul_add(r, 1.));
    let table = double((u as usize & 31) * 8)
        .to_bits()
        .wrapping_add((u as u64) << 47);
    (y * f64::from_bits(table)) as f32
}
#[inline]
pub fn log(x: f32) -> f32 {
    if x == 1. {
        return 0.;
    }
    if x == 0. {
        return f32::NEG_INFINITY;
    }
    if x < 0. || x.is_nan() {
        return f32::NAN;
    }
    if x == f32::INFINITY {
        return x;
    }
    let mut bits = x.to_bits();
    if bits < 0x800000 {
        bits = (x * 8388608.).to_bits().wrapping_sub(0xb800000);
    }
    let u = bits.wrapping_add(0xc0cd0000);
    let table = 0x148 + ((u >> 19) & 15) as usize * 16;
    let z = f32::from_bits(bits.wrapping_sub(u & 0xff800000));
    let r = double(table).mul_add(z as f64, -1.);
    let k = (u as i32 >> 23) as f64;
    let h = k.mul_add(double(0x248), double(table + 8));
    let r2 = r * r;
    let poly = double(0x250).mul_add(r2, double(0x258).mul_add(r, double(0x260)));
    r2.mul_add(poly, r + h) as f32
}
