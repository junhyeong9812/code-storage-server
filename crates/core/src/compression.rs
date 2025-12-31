// =============================================================================
// 압축 모듈 (compression.rs)
// =============================================================================
//
// zlib/deflate 압축 알고리즘 제공
// 
// Git과 동일한 압축 방식 사용
// - Blob 저장 시 파일 크기 감소 (보통 50-70% 절약)
// - 네트워크 전송 시 대역폭 절약
//
// 파일 위치: crates/core/src/compression.rs
//
// 사용 예시:
//   use core::compression::{compress, decompress};
//   
//   let original = b"hello world hello world hello world";
//   let compressed = compress(original)?;
//   let decompressed = decompress(&compressed)?;
//   assert_eq!(original.as_slice(), decompressed.as_slice());
// =============================================================================

use flate2::read::{ZlibDecoder, ZlibEncoder};
use flate2::Compression;
use std::io::{Read, Result};

// -----------------------------------------------------------------------------
// 상수
// -----------------------------------------------------------------------------

/// 기본 압축 레벨
///
/// 레벨 6 (기본값)
/// - 0: 압축 없음 (가장 빠름)
/// - 1-3: 빠른 압축 (낮은 압축률)
/// - 4-6: 균형 잡힌 압축 (기본값)
/// - 7-9: 최대 압축 (느리지만 높은 압축률)
fn default_compression() -> Compression {
    Compression::new(6)
}

// =============================================================================
// 압축 함수
// =============================================================================

/// 데이터 압축 (zlib)
///
/// zlib 형식으로 압축 (2바이트 헤더 + deflate 데이터 + 4바이트 체크섬)
/// Git과 동일한 형식
///
/// # Arguments
/// * `data` - 압축할 원본 데이터
///
/// # Returns
/// * `Ok(Vec<u8>)` - 압축된 데이터
/// * `Err` - 압축 실패 (메모리 부족 등)
///
/// # Example
/// ```
/// use core::compression::compress;
///
/// let original = b"hello world hello world";
/// let compressed = compress(original).unwrap();
///
/// // 반복되는 데이터는 압축률이 높음
/// assert!(compressed.len() < original.len());
/// ```
///
/// # 압축률 예시
/// - 텍스트 파일: 60-80% 감소
/// - 소스 코드: 70-85% 감소
/// - 이미 압축된 파일 (jpg, zip): 거의 변화 없음
pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    compress_with_level(data, default_compression())
}

/// 압축 레벨 지정 압축
///
/// # Arguments
/// * `data` - 압축할 원본 데이터
/// * `level` - 압축 레벨 (Compression::none/fast/default/best)
///
/// # Example
/// ```
/// use core::compression::compress_with_level;
/// use flate2::Compression;
///
/// // 빠른 압축 (압축률 낮음)
/// let fast = compress_with_level(data, Compression::fast())?;
///
/// // 최대 압축 (느리지만 작음)
/// let best = compress_with_level(data, Compression::best())?;
/// ```
pub fn compress_with_level(data: &[u8], level: Compression) -> Result<Vec<u8>> {
    // ZlibEncoder: zlib 형식 압축기
    // Read trait을 구현하므로 read_to_end로 모든 압축 데이터 읽기
    let mut encoder = ZlibEncoder::new(data, level);
    let mut compressed = Vec::new();
    encoder.read_to_end(&mut compressed)?;
    Ok(compressed)
}

// =============================================================================
// 해제 함수
// =============================================================================

/// 데이터 압축 해제 (zlib)
///
/// compress()로 압축된 데이터를 원본으로 복원
///
/// # Arguments
/// * `data` - 압축된 데이터
///
/// # Returns
/// * `Ok(Vec<u8>)` - 압축 해제된 원본 데이터
/// * `Err` - 압축 해제 실패 (잘못된 형식 등)
///
/// # Example
/// ```
/// use core::compression::{compress, decompress};
///
/// let original = b"hello world";
/// let compressed = compress(original)?;
/// let restored = decompress(&compressed)?;
///
/// assert_eq!(original.as_slice(), restored.as_slice());
/// ```
///
/// # 에러 케이스
/// - 잘못된 zlib 헤더
/// - 손상된 데이터
/// - 잘못된 체크섬
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    // ZlibDecoder: zlib 형식 압축 해제기
    let mut decoder = ZlibDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

/// 최대 크기 제한이 있는 압축 해제
///
/// 악의적인 압축 폭탄(zip bomb) 방지
/// 압축 해제 결과가 max_size를 초과하면 에러
///
/// # Arguments
/// * `data` - 압축된 데이터
/// * `max_size` - 최대 허용 크기 (바이트)
///
/// # Example
/// ```
/// // 최대 10MB로 제한
/// let result = decompress_with_limit(&compressed, 10 * 1024 * 1024)?;
/// ```
pub fn decompress_with_limit(data: &[u8], max_size: usize) -> Result<Vec<u8>> {
    let decoder = ZlibDecoder::new(data);
    let mut decompressed = Vec::new();

    // take(): 최대 max_size + 1 바이트까지 읽기 시도
    // +1은 제한 초과 여부 확인용
    let mut limited_reader = decoder.take((max_size + 1) as u64);
    limited_reader.read_to_end(&mut decompressed)?;

    // 제한 초과 확인
    if decompressed.len() > max_size {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Decompressed data exceeds {} bytes limit", max_size),
        ));
    }

    Ok(decompressed)
}

// =============================================================================
// 유틸리티 함수
// =============================================================================

/// 압축률 계산
///
/// # Returns
/// 압축률 (0.0 ~ 1.0)
/// - 0.0: 압축 안 됨 (또는 더 커짐)
/// - 0.5: 50% 감소
/// - 1.0: 100% 감소 (이론상 불가능)
///
/// # Example
/// ```
/// let ratio = compression_ratio(original_size, compressed_size);
/// println!("압축률: {:.1}%", ratio * 100.0);
/// ```
pub fn compression_ratio(original_size: usize, compressed_size: usize) -> f64 {
    if original_size == 0 {
        return 0.0;
    }
    1.0 - (compressed_size as f64 / original_size as f64)
}

/// 압축이 효과적인지 확인
///
/// 이미 압축된 파일(jpg, zip 등)은 압축해도 효과 없음
/// 오히려 약간 커질 수 있음
///
/// # Arguments
/// * `original_size` - 원본 크기
/// * `compressed_size` - 압축 후 크기
///
/// # Returns
/// 압축 크기가 원본보다 작으면 true
pub fn is_compression_effective(original_size: usize, compressed_size: usize) -> bool {
    compressed_size < original_size
}

// =============================================================================
// 테스트
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress() {
        let original = b"hello world hello world hello world";

        let compressed = compress(original).unwrap();
        let decompressed = decompress(&compressed).unwrap();

        assert_eq!(original.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_compression_reduces_size() {
        // 반복되는 데이터는 압축률이 높음
        let original = "hello ".repeat(1000);
        let compressed = compress(original.as_bytes()).unwrap();

        println!(
            "Original: {} bytes, Compressed: {} bytes, Ratio: {:.1}%",
            original.len(),
            compressed.len(),
            compression_ratio(original.len(), compressed.len()) * 100.0
        );

        assert!(compressed.len() < original.len());
    }

    #[test]
    fn test_empty_data() {
        let original = b"";

        let compressed = compress(original).unwrap();
        let decompressed = decompress(&compressed).unwrap();

        assert_eq!(original.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_single_byte() {
        let original = b"a";

        let compressed = compress(original).unwrap();
        let decompressed = decompress(&compressed).unwrap();

        assert_eq!(original.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_binary_data() {
        // 바이너리 데이터 (모든 바이트 값)
        let original: Vec<u8> = (0..=255).collect();

        let compressed = compress(&original).unwrap();
        let decompressed = decompress(&compressed).unwrap();

        assert_eq!(original, decompressed);
    }

    #[test]
    fn test_compression_levels() {
        let data = "test data ".repeat(100);

        let none = compress_with_level(data.as_bytes(), Compression::none()).unwrap();
        let fast = compress_with_level(data.as_bytes(), Compression::fast()).unwrap();
        let best = compress_with_level(data.as_bytes(), Compression::best()).unwrap();

        // 모두 정상 압축 해제 되어야 함
        assert_eq!(data.as_bytes(), decompress(&none).unwrap().as_slice());
        assert_eq!(data.as_bytes(), decompress(&fast).unwrap().as_slice());
        assert_eq!(data.as_bytes(), decompress(&best).unwrap().as_slice());

        // best가 가장 작아야 함 (보통)
        println!("None: {}, Fast: {}, Best: {}", none.len(), fast.len(), best.len());
    }

    #[test]
    fn test_decompress_with_limit() {
        let original = "x".repeat(1000);
        let compressed = compress(original.as_bytes()).unwrap();

        // 충분한 제한: 성공
        let result = decompress_with_limit(&compressed, 2000);
        assert!(result.is_ok());

        // 부족한 제한: 실패
        let result = decompress_with_limit(&compressed, 500);
        assert!(result.is_err());
    }

    #[test]
    fn test_compression_ratio() {
        assert_eq!(compression_ratio(100, 50), 0.5);   // 50% 감소
        assert_eq!(compression_ratio(100, 100), 0.0);  // 변화 없음
        assert_eq!(compression_ratio(100, 0), 1.0);    // 100% 감소
        assert_eq!(compression_ratio(0, 0), 0.0);      // 빈 데이터
    }

    #[test]
    fn test_invalid_compressed_data() {
        // 잘못된 데이터 압축 해제 시도
        let invalid = b"not valid zlib data";
        let result = decompress(invalid);
        assert!(result.is_err());
    }
}