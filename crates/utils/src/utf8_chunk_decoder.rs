use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Bytes;
use futures::Stream;

/// Incrementally decode UTF-8 from arbitrary byte chunks without splitting
/// multibyte characters at chunk boundaries.
#[derive(Debug, Default)]
pub struct Utf8ChunkDecoder {
    pending: Vec<u8>,
}

impl Utf8ChunkDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append bytes and return newly decoded UTF-8 text (may be empty).
    pub fn push(&mut self, chunk: &[u8]) -> String {
        if chunk.is_empty() {
            return String::new();
        }

        self.pending.extend_from_slice(chunk);
        let valid_len = match std::str::from_utf8(&self.pending) {
            Ok(_) => self.pending.len(),
            Err(error) => error.valid_up_to(),
        };

        if valid_len == 0 {
            return String::new();
        }

        String::from_utf8(self.pending.drain(..valid_len).collect())
            .expect("valid_up_to guarantees valid UTF-8")
    }

    /// Flush any trailing bytes when the stream ends.
    pub fn finish(&mut self) -> String {
        if self.pending.is_empty() {
            return String::new();
        }

        String::from_utf8_lossy(&self.pending).into_owned()
    }
}

/// Stream adapter that decodes UTF-8 across chunk boundaries and flushes on EOF.
pub struct Utf8DecodeStream<S> {
    inner: S,
    decoder: Utf8ChunkDecoder,
    eof: bool,
    flushed: bool,
}

impl<S> Utf8DecodeStream<S>
where
    S: Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
{
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            decoder: Utf8ChunkDecoder::new(),
            eof: false,
            flushed: false,
        }
    }
}

impl<S> Stream for Utf8DecodeStream<S>
where
    S: Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
{
    type Item = Result<String, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if self.flushed {
                return Poll::Ready(None);
            }

            if self.eof {
                self.flushed = true;
                let remaining = self.decoder.finish();
                return if remaining.is_empty() {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Ok(remaining)))
                };
            }

            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    let decoded = self.decoder.push(&bytes);
                    if decoded.is_empty() {
                        continue;
                    }
                    return Poll::Ready(Some(Ok(decoded)));
                }
                Poll::Ready(Some(Err(error))) => return Poll::Ready(Some(Err(error))),
                Poll::Ready(None) => self.eof = true,
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_single_chunk() {
        let mut decoder = Utf8ChunkDecoder::new();
        assert_eq!(decoder.push("第①条流水线".as_bytes()), "第①条流水线");
        assert_eq!(decoder.finish(), "");
    }

    #[test]
    fn decodes_multibyte_split_across_chunks() {
        let text = "第①条流水线";
        let bytes = text.as_bytes();

        for split_at in 1..bytes.len() {
            let (first, second) = bytes.split_at(split_at);
            let mut decoder = Utf8ChunkDecoder::new();
            let part1 = decoder.push(first);
            let part2 = decoder.push(second);
            let part3 = decoder.finish();
            let decoded = format!("{part1}{part2}{part3}");
            assert_eq!(decoded, text, "split at byte {split_at}");
            assert!(
                !decoded.contains('\u{FFFD}'),
                "replacement char at split {split_at}: {decoded:?}"
            );
        }
    }

    #[test]
    fn finish_uses_lossy_for_trailing_partial_bytes() {
        let mut decoder = Utf8ChunkDecoder::new();
        assert_eq!(decoder.push(&[0xE6, 0x9D]), "");
        assert_eq!(decoder.finish(), "\u{FFFD}");
    }
}
