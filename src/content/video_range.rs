use super::index::MediaTimestamp;
pub fn extract_range<T>(
    start: MediaTimestamp,
    end: MediaTimestamp,
    data: &[(T, MediaTimestamp)],
) -> impl Iterator<Item = &T> {
    if start.0 > end.0 {
        panic!("media start time must be before end time");
    }

    let sres = data.binary_search_by_key(&start, |(_, k)| *k);
    let sidx = match sres {
        Ok(i) => i,
        Err(i) => i - 1,
    };

    let eres = data.binary_search_by_key(&end, |(_, k)| *k);
    let eidx = match eres {
        Ok(i) => i,
        Err(i) => i,
    };

    data[sidx..eidx].iter().map(|(t, _)| t)
}
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn ts(s: f32) -> MediaTimestamp {
        MediaTimestamp(Duration::from_secs_f32(s))
    }

    fn build_media(time: f32, segments: usize) -> Vec<(usize, MediaTimestamp)> {
        (0..segments)
            .map(|i| {
                let start = MediaTimestamp(Duration::from_secs_f32(time * i as f32));
                (i, start)
            })
            .collect()
    }

    fn extract_to_vec(
        start: MediaTimestamp,
        end: MediaTimestamp,
        data: &[(usize, MediaTimestamp)],
    ) -> Vec<usize> {
        extract_range(start, end, data).cloned().collect()
    }

    #[test]
    fn inside_first_segment() {
        let e = build_media(30.0, 4);
        let v = extract_to_vec(ts(1.0), ts(2.0), e.as_slice());
        assert_eq!(v, vec![0]);
    }
    #[test]
    fn exact_match_middle() {
        let e = build_media(30.0, 4);
        let v = extract_to_vec(ts(30.0), ts(60.0), e.as_slice());
        assert_eq!(v, vec![1]);
    }
    #[test]
    fn span_two_segments() {
        let e = build_media(30.0, 4);
        let v = extract_to_vec(ts(59.0), ts(61.0), e.as_slice());
        assert_eq!(v, vec![1, 2]);
    }
    #[test]
    fn last_segment() {
        let e = build_media(30.0, 4);
        let v = extract_to_vec(ts(100.0), ts(600.0), e.as_slice());
        assert_eq!(v, vec![3]);
    }
    #[test]
    #[should_panic]
    fn invalid_range() {
        let e = build_media(30.0, 4);
        let _v = extract_to_vec(ts(100.0), ts(90.0), e.as_slice());
    }
}
