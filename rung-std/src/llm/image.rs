//! Images a tool hands back to the model: format sniffing, the size cap,
//! and the explicit note left in place of an image that is not sent.
//!
//! One check serves every producer (`read_file`, MCP tool results), so an
//! image that reaches a provider has passed the same gate wherever it came
//! from.

use super::ImageSource;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

/// Largest image a tool result carries, in raw bytes. Base64 grows it by
/// 4/3, so this stays under Anthropic's 5 MB limit on the encoded image
/// and matches Bedrock's 3.75 MB raw limit. OpenAI accepts more.
pub const MAX_IMAGE_BYTES: usize = 3_750_000;

/// Media types every image-capable provider rung speaks to accepts.
pub const IMAGE_MEDIA_TYPES: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/webp"];

/// The media type of an image from its leading bytes, or `None` when the
/// bytes are not one of [`IMAGE_MEDIA_TYPES`]. Content decides, not the
/// file name.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

impl ImageSource {
    /// A base64 image source. No checks: use [`ImageSource::from_bytes`] or
    /// [`ImageSource::from_base64`] for anything a tool produced.
    pub fn base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            source_type: "base64".into(),
            media_type: media_type.into(),
            data: data.into(),
        }
    }

    /// Encode raw image bytes. Refuses a format no provider takes and an
    /// image over [`MAX_IMAGE_BYTES`], with words a model can act on.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let Some(media_type) = sniff(bytes) else {
            return Err(format!(
                "not a supported image (supported: {})",
                IMAGE_MEDIA_TYPES.join(", ")
            ));
        };
        check_size(media_type, bytes.len())?;
        Ok(Self::base64(media_type, STANDARD.encode(bytes)))
    }

    /// An image that arrived already encoded (an MCP `image` content item).
    /// The bytes are decoded so the format is sniffed, not trusted from the
    /// declared type, and the size is the real one.
    pub fn from_base64(declared_type: &str, data: &str) -> Result<Self, String> {
        // Reject on the encoded length first so a huge payload is not decoded.
        if data.len() / 4 * 3 > MAX_IMAGE_BYTES + 3 {
            return Err(too_large(declared_type, data.len() / 4 * 3));
        }
        let bytes = STANDARD
            .decode(data.trim())
            .map_err(|e| format!("{declared_type} image is not valid base64: {e}"))?;
        Self::from_bytes(&bytes)
    }

    /// Size of the decoded image in bytes.
    pub fn byte_len(&self) -> usize {
        let pad = self.data.bytes().rev().take_while(|b| *b == b'=').count();
        (self.data.len() / 4 * 3).saturating_sub(pad)
    }

    /// The text left where this image is not sent, saying why. Never the
    /// image data itself.
    pub fn omitted_note(&self, why: &str) -> String {
        format!(
            "[image omitted: {}, {} bytes; {why}]",
            self.media_type,
            self.byte_len()
        )
    }
}

/// Refuse an image over [`MAX_IMAGE_BYTES`], in words a model can act on.
pub fn check_size(media_type: &str, len: usize) -> Result<(), String> {
    if len > MAX_IMAGE_BYTES {
        Err(too_large(media_type, len))
    } else {
        Ok(())
    }
}

fn too_large(media_type: &str, len: usize) -> String {
    format!(
        "{media_type} image is {len} bytes, over the {MAX_IMAGE_BYTES}-byte limit for an image \
         sent to the model; downscale or recompress it (for example a smaller PNG or a JPEG) \
         and look again"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    #[test]
    fn sniff_reads_content_not_name() {
        assert_eq!(sniff(PNG), Some("image/png"));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff(b"GIF89a.."), Some("image/gif"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"<svg xmlns="), None);
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVE"), None);
    }

    #[test]
    fn from_bytes_encodes_and_reports_size() {
        let img = ImageSource::from_bytes(PNG).unwrap();
        assert_eq!(img.source_type, "base64");
        assert_eq!(img.media_type, "image/png");
        assert_eq!(STANDARD.decode(&img.data).unwrap(), PNG);
        assert_eq!(img.byte_len(), PNG.len());
    }

    #[test]
    fn from_bytes_refuses_unsupported_format() {
        let err = ImageSource::from_bytes(b"BM\0\0 bitmap").unwrap_err();
        assert!(err.contains("not a supported image"), "{err}");
    }

    #[test]
    fn from_bytes_refuses_an_image_over_the_cap() {
        let mut big = PNG.to_vec();
        big.resize(MAX_IMAGE_BYTES + 1, 0);
        let err = ImageSource::from_bytes(&big).unwrap_err();
        assert!(err.contains("over the 3750000-byte limit"), "{err}");
        assert!(err.contains("downscale"), "{err}");
    }

    #[test]
    fn from_base64_sniffs_instead_of_trusting_the_declared_type() {
        let data = STANDARD.encode(PNG);
        let img = ImageSource::from_base64("image/jpeg", &data).unwrap();
        assert_eq!(img.media_type, "image/png");
        let err = ImageSource::from_base64("image/png", &STANDARD.encode(b"plain")).unwrap_err();
        assert!(err.contains("not a supported image"), "{err}");
        let err = ImageSource::from_base64("image/png", "!!not base64!!").unwrap_err();
        assert!(err.contains("not valid base64"), "{err}");
    }

    #[test]
    fn from_base64_refuses_oversize_before_decoding() {
        let data = "A".repeat((MAX_IMAGE_BYTES + 16) / 3 * 4);
        let err = ImageSource::from_base64("image/png", &data).unwrap_err();
        assert!(err.contains("byte limit"), "{err}");
    }

    #[test]
    fn omitted_note_names_the_image_and_never_its_data() {
        let img = ImageSource::from_bytes(PNG).unwrap();
        let note = img.omitted_note("this model takes text only");
        assert_eq!(
            note,
            format!(
                "[image omitted: image/png, {} bytes; this model takes text only]",
                PNG.len()
            )
        );
        assert!(!note.contains(&img.data));
    }
}
