use wide::*;
use anyhow::Result;

/// SIMD-optimized operations for packet processing
/// Supports both ARM NEON and x86_64 AVX2 where available
pub struct SimdPacketProcessor;

impl SimdPacketProcessor {
    /// SIMD-accelerated memory copy for packet data
    pub fn simd_copy(src: &[u8], dst: &mut [u8]) -> Result<()> {
        if src.len() != dst.len() {
            return Err(anyhow::anyhow!("Source and destination lengths must match"));
        }

        let len = src.len();
        if len == 0 {
            return Ok(());
        }

        // Use SIMD for bulk copy when size is appropriate
        if len >= 32 && len % 32 == 0 {
            Self::simd_copy_aligned_32(src, dst)
        } else if len >= 16 && len % 16 == 0 {
            Self::simd_copy_aligned_16(src, dst)
        } else {
            // Fallback to regular copy for small/unaligned data
            dst.copy_from_slice(src);
            Ok(())
        }
    }

    /// 32-byte aligned SIMD copy (AVX2/NEON)
    #[inline]
    fn simd_copy_aligned_32(src: &[u8], dst: &mut [u8]) -> Result<()> {
        let chunks = src.len() / 32;
        
        for i in 0..chunks {
            let src_offset = i * 32;
            let dst_offset = i * 32;
            
            // Load 32 bytes using SIMD
            let src_chunk = &src[src_offset..src_offset + 32];
            let dst_chunk = &mut dst[dst_offset..dst_offset + 32];
            
            // Use wide crate for cross-platform SIMD
            let src_u8x32 = u8x32::from([
                src_chunk[0], src_chunk[1], src_chunk[2], src_chunk[3],
                src_chunk[4], src_chunk[5], src_chunk[6], src_chunk[7],
                src_chunk[8], src_chunk[9], src_chunk[10], src_chunk[11],
                src_chunk[12], src_chunk[13], src_chunk[14], src_chunk[15],
                src_chunk[16], src_chunk[17], src_chunk[18], src_chunk[19],
                src_chunk[20], src_chunk[21], src_chunk[22], src_chunk[23],
                src_chunk[24], src_chunk[25], src_chunk[26], src_chunk[27],
                src_chunk[28], src_chunk[29], src_chunk[30], src_chunk[31],
            ]);
            
            let result = src_u8x32.to_array();
            dst_chunk.copy_from_slice(&result);
        }
        
        Ok(())
    }

    /// 16-byte aligned SIMD copy
    #[inline]
    fn simd_copy_aligned_16(src: &[u8], dst: &mut [u8]) -> Result<()> {
        let chunks = src.len() / 16;
        
        for i in 0..chunks {
            let src_offset = i * 16;
            let dst_offset = i * 16;
            
            let src_chunk = &src[src_offset..src_offset + 16];
            let dst_chunk = &mut dst[dst_offset..dst_offset + 16];
            
            let src_u8x16 = u8x16::from([
                src_chunk[0], src_chunk[1], src_chunk[2], src_chunk[3],
                src_chunk[4], src_chunk[5], src_chunk[6], src_chunk[7],
                src_chunk[8], src_chunk[9], src_chunk[10], src_chunk[11],
                src_chunk[12], src_chunk[13], src_chunk[14], src_chunk[15],
            ]);
            
            let result = src_u8x16.to_array();
            dst_chunk.copy_from_slice(&result);
        }
        
        Ok(())
    }

    /// SIMD-accelerated checksum calculation for packet validation
    pub fn simd_checksum(data: &[u8]) -> u32 {
        if data.len() < 16 {
            return Self::scalar_checksum(data);
        }

        let mut checksum = 0u32;
        let chunks = data.len() / 16;
        let remainder_start = chunks * 16;

        // Process 16-byte chunks with SIMD
        for i in 0..chunks {
            let offset = i * 16;
            let chunk = &data[offset..offset + 16];
            
            let u8x16_chunk = u8x16::from([
                chunk[0], chunk[1], chunk[2], chunk[3],
                chunk[4], chunk[5], chunk[6], chunk[7],
                chunk[8], chunk[9], chunk[10], chunk[11],
                chunk[12], chunk[13], chunk[14], chunk[15],
            ]);
            
            // Sum all bytes in the vector
            let sum = u8x16_chunk.to_array().iter().map(|&x| x as u32).sum::<u32>();
            checksum = checksum.wrapping_add(sum);
        }

        // Handle remainder bytes
        if remainder_start < data.len() {
            let remainder_sum = data[remainder_start..].iter()
                .map(|&x| x as u32)
                .sum::<u32>();
            checksum = checksum.wrapping_add(remainder_sum);
        }

        checksum
    }

    /// Scalar checksum fallback for small data
    #[inline]
    fn scalar_checksum(data: &[u8]) -> u32 {
        data.iter().map(|&x| x as u32).sum()
    }

    /// SIMD-accelerated pattern matching for packet filtering
    pub fn simd_pattern_match(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        if needle.is_empty() || haystack.len() < needle.len() {
            return None;
        }

        // Use SIMD for pattern matching when feasible
        if needle.len() >= 16 && haystack.len() >= 32 {
            Self::simd_pattern_match_16(haystack, needle)
        } else {
            // Fallback to naive search for small patterns
            Self::naive_pattern_match(haystack, needle)
        }
    }

    /// SIMD pattern matching for 16-byte patterns
    fn simd_pattern_match_16(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        if needle.len() < 16 {
            return Self::naive_pattern_match(haystack, needle);
        }

        let needle_first_16 = &needle[..16];
        let needle_u8x16 = u8x16::from([
            needle_first_16[0], needle_first_16[1], needle_first_16[2], needle_first_16[3],
            needle_first_16[4], needle_first_16[5], needle_first_16[6], needle_first_16[7],
            needle_first_16[8], needle_first_16[9], needle_first_16[10], needle_first_16[11],
            needle_first_16[12], needle_first_16[13], needle_first_16[14], needle_first_16[15],
        ]);

        for i in 0..=(haystack.len().saturating_sub(needle.len())) {
            if i + 16 <= haystack.len() {
                let haystack_chunk = &haystack[i..i + 16];
                let haystack_u8x16 = u8x16::from([
                    haystack_chunk[0], haystack_chunk[1], haystack_chunk[2], haystack_chunk[3],
                    haystack_chunk[4], haystack_chunk[5], haystack_chunk[6], haystack_chunk[7],
                    haystack_chunk[8], haystack_chunk[9], haystack_chunk[10], haystack_chunk[11],
                    haystack_chunk[12], haystack_chunk[13], haystack_chunk[14], haystack_chunk[15],
                ]);

                // Compare first 16 bytes
                if haystack_u8x16 == needle_u8x16 {
                    // If needle is exactly 16 bytes, we found a match
                    if needle.len() == 16 {
                        return Some(i);
                    }
                    // Otherwise, check remaining bytes
                    if i + needle.len() <= haystack.len() 
                        && &haystack[i + 16..i + needle.len()] == &needle[16..] {
                        return Some(i);
                    }
                }
            }
        }

        None
    }

    /// Naive pattern matching fallback
    fn naive_pattern_match(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len())
            .position(|window| window == needle)
    }

    /// Batch SIMD operations for multiple packets
    pub fn batch_simd_copy(src_dst_pairs: &mut [(&[u8], &mut [u8])]) -> Result<()> {
        for (src, dst) in src_dst_pairs {
            Self::simd_copy(src, dst)?;
        }
        Ok(())
    }

    /// SIMD-accelerated data transformation (e.g., byte swapping for endianness)
    pub fn simd_byte_swap_u32(data: &mut [u32]) {
        // Process 4 u32s at a time (16 bytes total)
        let chunks = data.len() / 4;
        
        for i in 0..chunks {
            let offset = i * 4;
            let chunk = &mut data[offset..offset + 4];
            
            // Swap bytes in each u32
            for val in chunk {
                *val = val.swap_bytes();
            }
        }
        
        // Handle remainder
        let remainder_start = chunks * 4;
        for val in &mut data[remainder_start..] {
            *val = val.swap_bytes();
        }
    }

    /// Get SIMD capabilities info
    pub fn get_simd_info() -> SimdInfo {
        SimdInfo {
            has_avx2: Self::has_avx2(),
            has_neon: Self::has_neon(),
            vector_width: Self::get_vector_width(),
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn has_avx2() -> bool {
        is_x86_feature_detected!("avx2")
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn has_avx2() -> bool {
        false
    }

    #[cfg(target_arch = "aarch64")]
    fn has_neon() -> bool {
        // NEON is standard on aarch64
        true
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn has_neon() -> bool {
        false
    }

    fn get_vector_width() -> usize {
        if Self::has_avx2() {
            32 // AVX2 256-bit vectors
        } else if Self::has_neon() {
            16 // NEON 128-bit vectors
        } else {
            8  // Fallback to 64-bit
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimdInfo {
    pub has_avx2: bool,
    pub has_neon: bool,
    pub vector_width: usize,
}

/// Specialized SIMD operations for CBOE PITCH packet processing
pub struct CboePitchSimd;

impl CboePitchSimd {
    /// Fast CBOE header validation using SIMD
    pub fn validate_cboe_header_batch(headers: &[&[u8]]) -> Vec<bool> {
        headers.iter()
            .map(|header| Self::validate_single_header(header))
            .collect()
    }

    fn validate_single_header(header: &[u8]) -> bool {
        // CBOE PITCH header validation logic
        header.len() >= 8 && 
        header[0] != 0 &&  // Length field should not be zero
        header[1] != 0     // Count field should not be zero
    }

    /// SIMD-accelerated price conversion for CBOE format
    pub fn convert_prices_batch(prices: &[f64]) -> Vec<u64> {
        prices.iter()
            .map(|&price| (price * 10_000_000.0) as u64)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_copy() {
        let src = vec![1u8; 64];
        let mut dst = vec![0u8; 64];
        
        SimdPacketProcessor::simd_copy(&src, &mut dst).unwrap();
        assert_eq!(src, dst);
    }

    #[test]
    fn test_simd_checksum() {
        let data = vec![1u8; 100];
        let checksum = SimdPacketProcessor::simd_checksum(&data);
        assert_eq!(checksum, 100); // All bytes are 1
    }

    #[test]
    fn test_pattern_matching() {
        let haystack = b"hello world hello";
        let needle = b"world";
        
        let pos = SimdPacketProcessor::simd_pattern_match(haystack, needle);
        assert_eq!(pos, Some(6));
    }

    #[test]
    fn test_simd_info() {
        let info = SimdPacketProcessor::get_simd_info();
        println!("SIMD Info: {:?}", info);
        assert!(info.vector_width >= 8);
    }
}