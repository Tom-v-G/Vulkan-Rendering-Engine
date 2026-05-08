/// Packed into 4 bytes: RGB + active flag (via alpha or a sentinel value)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Voxel(u32);

impl Voxel {
    pub const EMPTY: Self = Self(0);

    pub fn new(r: u8, g: u8, b: u8) -> Self {
        // Alpha = 0xFF not used atm
        // Probably repurpose for materials (liqued, gas, metal, etc.)
        Self(0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    pub fn r(self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }
    pub fn g(self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }
    pub fn b(self) -> u8 {
        (self.0 & 0xFF) as u8
    }
}
