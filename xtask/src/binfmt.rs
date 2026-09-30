//! Binary architecture detection from Mach-O / ELF headers (no external tools).

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Arch {
    Aarch64,
    X86_64,
}

const CPU_X86_64: u32 = 0x0100_0007;
const CPU_ARM64: u32 = 0x0100_000c;

fn cpu(t: u32) -> Option<Arch> {
    match t {
        CPU_X86_64 => Some(Arch::X86_64),
        CPU_ARM64 => Some(Arch::Aarch64),
        _ => None,
    }
}

/// (format, sorted architectures). Format is "macho" or "elf".
pub fn detect(b: &[u8]) -> Result<(&'static str, Vec<Arch>), String> {
    let u32le = |o: usize| b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
    let u32be = |o: usize| b.get(o..o + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]));
    let short = || "file too short for a header".to_string();
    if b.starts_with(&[0x7f, b'E', b'L', b'F']) {
        let m = b.get(18..20).ok_or_else(short)?;
        let arch = match u16::from_le_bytes([m[0], m[1]]) {
            0xb7 => Arch::Aarch64,
            0x3e => Arch::X86_64,
            other => return Err(format!("unknown ELF e_machine {other:#x}")),
        };
        return Ok(("elf", vec![arch]));
    }
    if u32le(0) == Some(0xfeed_facf) {
        let t = u32le(4).ok_or_else(short)?;
        let arch = cpu(t).ok_or_else(|| format!("unknown Mach-O cputype {t:#x}"))?;
        return Ok(("macho", vec![arch]));
    }
    if u32be(0) == Some(0xcafe_babe) {
        let n = u32be(4).ok_or_else(short)? as usize;
        let mut archs = Vec::new();
        for i in 0..n.min(16) {
            let t = u32be(8 + i * 20).ok_or_else(short)?;
            archs.push(cpu(t).ok_or_else(|| format!("unknown fat cputype {t:#x}"))?);
        }
        archs.sort();
        return Ok(("macho", archs));
    }
    Err("not a Mach-O or ELF file".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elf_aarch64() {
        let mut h = vec![0u8; 20];
        h[..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
        h[18] = 0xb7;
        assert_eq!(detect(&h), Ok(("elf", vec![Arch::Aarch64])));
    }

    #[test]
    fn macho_x86_64() {
        let mut h = 0xfeed_facfu32.to_le_bytes().to_vec();
        h.extend(CPU_X86_64.to_le_bytes());
        assert_eq!(detect(&h), Ok(("macho", vec![Arch::X86_64])));
    }

    #[test]
    fn fat_both() {
        let mut h = 0xcafe_babeu32.to_be_bytes().to_vec();
        h.extend(2u32.to_be_bytes());
        for c in [CPU_X86_64, CPU_ARM64] {
            h.extend(c.to_be_bytes());
            h.extend([0u8; 16]);
        }
        assert_eq!(detect(&h), Ok(("macho", vec![Arch::Aarch64, Arch::X86_64])));
    }

    #[test]
    fn garbage() {
        assert!(detect(b"hello").is_err());
    }
}
