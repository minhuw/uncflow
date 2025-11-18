use crate::error::Result;

#[cfg(target_arch = "x86_64")]
pub fn cpuid(eax: u32, ecx: u32) -> (u32, u32, u32, u32) {
    let mut ebx: u32;
    let mut edx: u32;
    let mut eax_out = eax;
    let mut ecx_out = ecx;

    unsafe {
        std::arch::asm!(
            "mov {0:r}, rbx",
            "cpuid",
            "xchg {0:r}, rbx",
            out(reg) ebx,
            inout("eax") eax_out,
            inout("ecx") ecx_out,
            out("edx") edx,
            options(nostack, preserves_flags)
        );
    }

    (eax_out, ebx, ecx_out, edx)
}

#[cfg(not(target_arch = "x86_64"))]
pub fn cpuid(_eax: u32, _ecx: u32) -> (u32, u32, u32, u32) {
    (0, 0, 0, 0)
}

pub fn get_mbm_scaling_factor() -> Result<u32> {
    let (_eax, ebx, _ecx, _edx) = cpuid(0x0F, 0x1);
    let scaling_factor = ebx;

    if scaling_factor == 0 {
        tracing::warn!("MBM scaling factor is 0, defaulting to 1");
        Ok(1)
    } else {
        tracing::info!("MBM scaling factor: {}", scaling_factor);
        Ok(scaling_factor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::similar_names)] // CPU register names are standard
    fn test_cpuid() {
        let (eax, ebx, ecx, edx) = cpuid(0, 0);
        println!("CPUID(0,0): EAX={eax:08X} EBX={ebx:08X} ECX={ecx:08X} EDX={edx:08X}");
    }
}
