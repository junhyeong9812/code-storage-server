// =============================================================================
// 해싱 모듈 (hash.rs)
// =============================================================================
//
// SHA-256 해싱 기능 제공
// 
// Git은 SHA-1을 사용하지만, CTS는 더 안전한 SHA-256 사용
// - SHA-1: 160비트 (40자 hex) - 충돌 공격 가능
// - SHA-256: 256비트 (64자 hex) - 현재 안전
//
// 파일 위치: crates/core/src/hash.rs
//
// 사용 예시:
//   use core::hash::Hasher;
//   
//   let hasher = Hasher::new();
//   let hash = hasher.hash_bytes(b"hello world");
//   println!("{}", hash);  // 64자 hex 문자열
// =============================================================================

use sha2::{Sha256, Digest};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

// -----------------------------------------------------------------------------
// 상수
// -----------------------------------------------------------------------------

/// 해시 출력 길이 (바이트)
/// SHA-256 = 256비트 = 32바이트
pub const HASH_LENGTH: usize = 32;

/// 해시 hex 문자열 길이
/// 32바이트 * 2 = 64자
pub const HASH_HEX_LENGTH: usize = 64;

/// 파일 읽기 버퍼 크기 (8KB)
/// 큰 파일을 청크 단위로 읽어서 메모리 효율적으로 해싱
const BUFFER_SIZE: usize = 8 * 1024;

// =============================================================================
// Hasher 구조체
// =============================================================================

/// SHA-256 해셔
///
/// 바이트 배열, 문자열, 파일 등을 해싱
///
/// # Example
/// ```
/// use core::hash::Hasher;
///
/// let hasher = Hasher::new();
///
/// // 바이트 배열 해싱
/// let hash = hasher.hash_bytes(b"hello");
///
/// // 문자열 해싱
/// let hash = hasher.hash_str("hello");
///
/// // 파일 해싱
/// let hash = hasher.hash_file("path/to/file").unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct Hasher;

impl Hasher {
    /// 새 Hasher 생성
    ///
    /// Hasher는 상태가 없으므로 여러 번 재사용 가능
    pub fn new() -> Self {
        Self
    }

    // -------------------------------------------------------------------------
    // 기본 해싱 메서드
    // -------------------------------------------------------------------------

    /// 바이트 배열 해싱
    ///
    /// # Arguments
    /// * `data` - 해싱할 바이트 슬라이스
    ///
    /// # Returns
    /// 64자 hex 문자열 (소문자)
    ///
    /// # Example
    /// ```
    /// let hash = hasher.hash_bytes(b"hello world");
    /// assert_eq!(hash.len(), 64);
    /// ```
    pub fn hash_bytes(&self, data: &[u8]) -> String {
        // Sha256 해시 계산
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();

        // 바이트 배열을 hex 문자열로 변환
        hex::encode(result)
    }

    /// 문자열 해싱
    ///
    /// 내부적으로 hash_bytes 호출
    ///
    /// # Arguments
    /// * `s` - 해싱할 문자열
    ///
    /// # Returns
    /// 64자 hex 문자열
    pub fn hash_str(&self, s: &str) -> String {
        self.hash_bytes(s.as_bytes())
    }

    /// 파일 해싱
    ///
    /// 파일을 청크 단위로 읽어서 메모리 효율적으로 해싱
    /// 큰 파일도 적은 메모리로 처리 가능
    ///
    /// # Arguments
    /// * `path` - 파일 경로
    ///
    /// # Returns
    /// * `Ok(String)` - 64자 hex 문자열
    /// * `Err` - 파일 읽기 실패
    ///
    /// # Example
    /// ```
    /// let hash = hasher.hash_file("large_file.bin")?;
    /// ```
    pub fn hash_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<String> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; BUFFER_SIZE];

        // 파일을 청크 단위로 읽으면서 해싱
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;  // EOF
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let result = hasher.finalize();
        Ok(hex::encode(result))
    }

    // -------------------------------------------------------------------------
    // 검증 메서드
    // -------------------------------------------------------------------------

    /// 해시 검증
    ///
    /// 데이터의 해시가 기대값과 일치하는지 확인
    /// 데이터 무결성 검증에 사용
    ///
    /// # Arguments
    /// * `data` - 검증할 데이터
    /// * `expected_hash` - 기대하는 해시값 (hex 문자열)
    ///
    /// # Returns
    /// * `true` - 해시 일치
    /// * `false` - 해시 불일치 (데이터 손상 또는 변조)
    ///
    /// # Example
    /// ```
    /// let data = b"hello";
    /// let hash = hasher.hash_bytes(data);
    ///
    /// assert!(hasher.verify(data, &hash));
    /// assert!(!hasher.verify(b"world", &hash));
    /// ```
    pub fn verify(&self, data: &[u8], expected_hash: &str) -> bool {
        let actual_hash = self.hash_bytes(data);
        // 타이밍 공격 방지를 위해 상수 시간 비교가 이상적이지만,
        // 여기서는 간단히 문자열 비교 사용
        actual_hash == expected_hash.to_lowercase()
    }

    /// 파일 해시 검증
    ///
    /// # Arguments
    /// * `path` - 파일 경로
    /// * `expected_hash` - 기대하는 해시값
    ///
    /// # Returns
    /// * `Ok(true)` - 해시 일치
    /// * `Ok(false)` - 해시 불일치
    /// * `Err` - 파일 읽기 실패
    pub fn verify_file<P: AsRef<Path>>(
        &self,
        path: P,
        expected_hash: &str,
    ) -> std::io::Result<bool> {
        let actual_hash = self.hash_file(path)?;
        Ok(actual_hash == expected_hash.to_lowercase())
    }
}

// =============================================================================
// 편의 함수 (Convenience Functions)
// =============================================================================
// Hasher 인스턴스 없이 바로 사용 가능

/// 바이트 배열 해싱 (편의 함수)
///
/// # Example
/// ```
/// use core::hash::hash_bytes;
/// let hash = hash_bytes(b"hello");
/// ```
pub fn hash_bytes(data: &[u8]) -> String {
    Hasher::new().hash_bytes(data)
}

/// 문자열 해싱 (편의 함수)
pub fn hash_str(s: &str) -> String {
    Hasher::new().hash_str(s)
}

/// 파일 해싱 (편의 함수)
pub fn hash_file<P: AsRef<Path>>(path: P) -> std::io::Result<String> {
    Hasher::new().hash_file(path)
}

// =============================================================================
// 테스트
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_bytes() {
        let hasher = Hasher::new();

        // "hello world"의 SHA-256 해시 (알려진 값)
        let hash = hasher.hash_bytes(b"hello world");
        assert_eq!(hash.len(), HASH_HEX_LENGTH);
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_hash_str() {
        let hasher = Hasher::new();

        let hash1 = hasher.hash_str("hello");
        let hash2 = hasher.hash_bytes(b"hello");

        // 같은 입력은 같은 해시
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_empty_input() {
        let hasher = Hasher::new();

        // 빈 입력의 SHA-256 해시
        let hash = hasher.hash_bytes(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_verify() {
        let hasher = Hasher::new();
        let data = b"test data";
        let hash = hasher.hash_bytes(data);

        assert!(hasher.verify(data, &hash));
        assert!(!hasher.verify(b"different data", &hash));
    }

    #[test]
    fn test_verify_case_insensitive() {
        let hasher = Hasher::new();
        let data = b"test";
        let hash = hasher.hash_bytes(data);

        // 대문자로 변환해도 검증 성공
        assert!(hasher.verify(data, &hash.to_uppercase()));
    }

    #[test]
    fn test_deterministic() {
        let hasher = Hasher::new();

        // 같은 입력은 항상 같은 해시
        let hash1 = hasher.hash_str("deterministic");
        let hash2 = hasher.hash_str("deterministic");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_different_inputs() {
        let hasher = Hasher::new();

        // 다른 입력은 다른 해시
        let hash1 = hasher.hash_str("input1");
        let hash2 = hasher.hash_str("input2");
        assert_ne!(hash1, hash2);
    }
}