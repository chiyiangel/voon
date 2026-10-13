use rtrb::Consumer;

/// Result of filling one HAL callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FillResult {
    pub underrun: bool,
    pub complete: bool,
    pub bytes_consumed: usize,
}

/// One planar channel buffer described by a raw pointer.
///
/// The callback uses this so the same function can be tested without CoreAudio.
pub struct Plane {
    pub ptr: *mut u8,
    pub len: usize,
}

/// Copy interleaved PCM from the ring into one device buffer.
///
/// A short read is padded with silence. That is an underrun only when the
/// decoder has not finished. Once the decoder is finished and the ring is
/// empty, the callback is complete and the tail is silence, not an underrun.
pub fn fill_interleaved(
    consumer: &mut Consumer<u8>,
    dest: &mut [u8],
    frame_bytes: usize,
    producer_done: bool,
) -> FillResult {
    if frame_bytes == 0 || dest.is_empty() {
        return FillResult {
            underrun: false,
            complete: producer_done && consumer.slots() == 0,
            bytes_consumed: 0,
        };
    }
    let usable = dest.len() - (dest.len() % frame_bytes);
    let mut filled = 0;
    while filled < usable {
        let (_, remainder) = consumer.pop_partial_slice(&mut dest[filled..usable]);
        let got = (usable - filled) - remainder.len();
        if got == 0 {
            break;
        }
        filled += got;
    }
    if filled < dest.len() {
        dest[filled..].fill(0);
    }
    finish(consumer, filled, filled < usable, producer_done)
}

/// Copy interleaved frames from the ring into non-interleaved channel planes.
///
/// # Safety
///
/// Each plane must reference `len` writable bytes for the duration of the call,
/// and the planes must not overlap.
pub unsafe fn fill_planes(
    consumer: &mut Consumer<u8>,
    planes: &mut [Plane],
    bytes_per_sample: usize,
    producer_done: bool,
) -> FillResult {
    if planes.is_empty() || bytes_per_sample == 0 || bytes_per_sample > 4 || planes.len() > 8 {
        return FillResult {
            underrun: false,
            complete: producer_done,
            bytes_consumed: 0,
        };
    }
    let channels = planes.len();
    let frame_bytes = bytes_per_sample * channels;
    let frames = planes
        .iter()
        .map(|plane| plane.len / bytes_per_sample)
        .min()
        .unwrap_or(0);
    for plane in planes.iter() {
        if !plane.ptr.is_null() && plane.len > 0 {
            // SAFETY: caller guarantees the plane is writable for `len` bytes.
            unsafe { std::ptr::write_bytes(plane.ptr, 0, plane.len) };
        }
    }
    let mut frame = [0u8; 32];
    let mut filled_frames = 0usize;
    while filled_frames < frames {
        let (_, remainder) = consumer.pop_partial_slice(&mut frame[..frame_bytes]);
        let got = frame_bytes - remainder.len();
        if got < frame_bytes {
            break;
        }
        for (channel, plane) in planes.iter().enumerate() {
            if plane.ptr.is_null() {
                continue;
            }
            let src = frame.as_ptr().add(channel * bytes_per_sample);
            let dst = plane.ptr.add(filled_frames * bytes_per_sample);
            // SAFETY: frame holds `bytes_per_sample` for this channel, and the
            // plane has room for this frame because `filled_frames < frames`.
            unsafe {
                std::ptr::copy_nonoverlapping(src, dst, bytes_per_sample);
            }
        }
        filled_frames += 1;
    }
    let bytes = filled_frames * frame_bytes;
    finish(consumer, bytes, filled_frames < frames, producer_done)
}

fn finish(
    consumer: &Consumer<u8>,
    bytes_consumed: usize,
    short: bool,
    producer_done: bool,
) -> FillResult {
    let empty = consumer.slots() == 0;
    FillResult {
        underrun: short && !producer_done,
        complete: producer_done && empty,
        bytes_consumed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtrb::RingBuffer;

    #[test]
    fn interleaved_copy_pads_and_counts_one_underrun() {
        let (mut producer, mut consumer) = RingBuffer::<u8>::new(8);
        let (_, rest) = producer.push_partial_slice(&[1, 2, 3, 4]);
        assert!(rest.is_empty());
        let mut dest = [0u8; 8];
        let result = fill_interleaved(&mut consumer, &mut dest, 4, false);
        assert_eq!(&dest[..4], &[1, 2, 3, 4]);
        assert_eq!(&dest[4..], &[0, 0, 0, 0]);
        assert!(result.underrun);
        assert!(!result.complete);
        assert_eq!(result.bytes_consumed, 4);
    }

    #[test]
    fn end_of_stream_silence_is_not_an_underrun() {
        let (_producer, mut consumer) = RingBuffer::<u8>::new(4);
        let mut dest = [7u8; 4];
        let result = fill_interleaved(&mut consumer, &mut dest, 2, true);
        assert_eq!(dest, [0, 0, 0, 0]);
        assert!(!result.underrun);
        assert!(result.complete);
    }

    #[test]
    fn planar_deinterleave_matches_channel_bytes() {
        let (mut producer, mut consumer) = RingBuffer::<u8>::new(16);
        // Two stereo frames, 2 bytes per sample: L0 L1 R0 R1, L0 L1 R0 R1.
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let (_, rest) = producer.push_partial_slice(&data);
        assert!(rest.is_empty());
        let mut left = [0u8; 4];
        let mut right = [0u8; 4];
        let mut planes = [
            Plane {
                ptr: left.as_mut_ptr(),
                len: left.len(),
            },
            Plane {
                ptr: right.as_mut_ptr(),
                len: right.len(),
            },
        ];
        let result = unsafe { fill_planes(&mut consumer, &mut planes, 2, true) };
        assert_eq!(left, [1, 2, 5, 6]);
        assert_eq!(right, [3, 4, 7, 8]);
        assert!(result.complete);
        assert!(!result.underrun);
        assert_eq!(result.bytes_consumed, 8);
    }
}
