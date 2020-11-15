use crate::srt::Subtitle;
use nom::{
    bytes::complete::is_not,
    bytes::complete::tag,
    bytes::complete::take_till,
    character::complete::line_ending,
    character::complete::{char, one_of},
    combinator::{map, map_res, recognize},
    dbg_basic,
    multi::{many0, many1},
    sequence::{pair, preceded, terminated, tuple},
    IResult,
};
use std::time::Duration;

fn decimal(input: &str) -> IResult<&str, &str> {
    recognize(many1(one_of("0123456789")))(input)
}

fn ascii_u32(input: &str) -> IResult<&str, u32> {
    map_res(decimal, |s| s.parse::<u32>())(input)
}

fn timestamp(input: &str) -> IResult<&str, Duration> {
    map(
        tuple((
            ascii_u32,
            tag(":"),
            ascii_u32,
            tag(":"),
            ascii_u32,
            tag(","),
            ascii_u32,
        )),
        |(h, _, m, _, s, _, mi)| {
            let mut t = h;
            t = t * 60 + m;
            t = t * 60 + s;
            t = t * 1000 + mi;
            Duration::from_millis(t as u64)
        },
    )(input)
}

fn timespan(input: &str) -> IResult<&str, (Duration, Duration)> {
    map(
        tuple((
            timestamp,
            many1(tag(" ")),
            tag("-->"),
            many1(tag(" ")),
            timestamp,
        )),
        |t| (t.0, t.4),
    )(input)
}

fn text(input: &str) -> IResult<&str, &str> {
    recognize(many0(terminated(is_not("\n\r"), line_ending)))(input)
}

fn subtitle(input: &str) -> IResult<&str, Subtitle> {
    map(
        tuple((
            ascii_u32,
            line_ending,
            timespan,
            many0(line_ending),
            text,
            many0(line_ending),
        )),
        |(idx, _, (start, end), _, text, _)| {
            let better_newlines = text.replace("\r\n", "\n");
            Subtitle {
                idx,
                start,
                end,
                text: better_newlines,
            }
        },
    )(input)
}

fn srt_file(input: &str) -> IResult<&str, Vec<Subtitle>> {
    preceded(many0(line_ending), many0(subtitle))(input)
}

pub fn parse(input: &str) -> anyhow::Result<Vec<Subtitle>> {
    let (leftover, results) =
        srt_file(input).map_err(|e| anyhow::anyhow!("could not parse srt file: {}", e))?;
    if !leftover.is_empty() {
        log::warn!("unparsed data at end of file: {:?}", leftover)
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_numberline() {
        let input = "231";
        let (r, n) = ascii_u32(input).unwrap();
        assert_eq!(n, 231);
        assert_eq!(r, "");
    }

    #[test]
    fn timestamp_zero() {
        let input = "00:00:00,000";
        let (r, t) = timestamp(input).unwrap();
        assert_eq!(t, Duration::from_millis(0));
    }

    #[test]
    fn timestamp_millis() {
        let input = "00:00:00,050";
        let (r, t) = timestamp(input).unwrap();
        assert_eq!(t, Duration::from_millis(50));
    }
    #[test]
    fn timestamp_secs() {
        let input = "00:00:08,000";
        let (r, t) = timestamp(input).unwrap();
        assert_eq!(t, Duration::from_secs(8));
    }
    #[test]
    fn timestamp_min() {
        let input = "00:14:00,000";
        let (r, t) = timestamp(input).unwrap();
        assert_eq!(t, Duration::from_secs(14 * 60));
    }
    #[test]
    fn timestamp_hrs() {
        let input = "02:00:00,000";
        let (r, t) = timestamp(input).unwrap();
        assert_eq!(t, Duration::from_secs(2 * 60 * 60));
    }
    #[test]
    fn timestamp_random() {
        let input = "02:14:08,050";
        let (r, t) = timestamp(input).unwrap();
        assert_eq!(
            t,
            Duration::from_millis(((2 * 60 + 14) * 60 + 8) * 1000 + 50)
        );
    }

    struct TimeSpanTestCase {
        unit: u32,
        raw: &'static str,
        start: Duration,
        end: Duration,
        lines: &'static [&'static str],
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum LineEnding {
        Unix,
        Windows,
    }
    impl LineEnding {
        fn as_str(&self) -> &'static str {
            match self {
                LineEnding::Unix => "\n",
                LineEnding::Windows => "\r\n",
            }
        }
    }

    impl TimeSpanTestCase {
        fn input(&self, ending: LineEnding, text_newline: bool, final_newline: bool) -> String {
            let newline = ending.as_str();
            let mut res = format!("{}", self.unit);
            res.push_str(newline);
            res.push_str(self.raw);
            res.push_str(newline);
            if text_newline {
                res.push_str(newline);
            }
            for line in self.lines {
                res.push_str(line);
                res.push_str(newline);
            }
            if final_newline {
                res.push_str(newline);
            }
            res
        }
        fn sub(&self) -> Subtitle {
            let mut text = self.lines.join("\n");
            text.push_str("\n");
            Subtitle {
                idx: self.unit,
                start: self.start,
                end: self.end,
                text,
            }
        }
    }

    const EX_TS_1: TimeSpanTestCase = TimeSpanTestCase {
        unit: 1,
        raw: "00:00:02,002 --> 00:00:05,403",
        start: Duration::from_millis(02 * 1000 + 2),
        end: Duration::from_millis(05 * 1000 + 403),
        lines: &[
            "<i>Now the story of a wealthy family</i>",
            "<i>who lost everything...</i>",
        ],
    };

    const EX_TS_2: TimeSpanTestCase = TimeSpanTestCase {
        unit: 2,
        raw: "00:00:05,505 --> 00:00:07,496",
        start: Duration::from_millis(05 * 1000 + 505),
        end: Duration::from_millis(07 * 1000 + 496),
        lines: &["<i>and the one son</i>", "<i>who had no choice...</i>"],
    };

    const EX_TS_3: TimeSpanTestCase = TimeSpanTestCase {
        unit: 3,
        raw: "00:00:07,607 --> 00:00:09,598",
        start: Duration::from_millis(07 * 1000 + 607),
        end: Duration::from_millis(09 * 1000 + 598),
        lines: &["<i>but to keep them all together.</i>"],
    };

    fn test_srt(ending: LineEnding, extra_text_line: bool, short_ending: bool) {
        let mut input = String::new();
        input.push_str(EX_TS_1.input(ending, extra_text_line, true).as_str());
        input.push_str(EX_TS_2.input(ending, extra_text_line, true).as_str());
        input.push_str(EX_TS_3.input(ending, extra_text_line, !short_ending).as_str());
        let expected = vec![EX_TS_1.sub(), EX_TS_2.sub(), EX_TS_3.sub()];

        let (subt_rem, res) = srt_file(input.as_str()).unwrap();
        assert_eq!(subt_rem, "");
        assert_eq!(res, expected);
    }

    fn test_each_sub(ending: LineEnding, extra_text_line: bool, final_newline: bool) {
        for tc in &[EX_TS_1, EX_TS_2, EX_TS_3] {
            let input = tc.input(ending, extra_text_line, final_newline);
            let (subt_rem, sub) = subtitle(input.as_str()).unwrap();
            assert_eq!(subt_rem, "");
            assert_eq!(sub, tc.sub());
        }
    }

    #[test]
    fn parse_sub() {
        test_each_sub(LineEnding::Unix, false, true);
    }
    #[test]
    fn parse_sub_windows() {
        test_each_sub(LineEnding::Windows, false, true);
    }
    #[test]
    fn parse_sub_last() {
        test_each_sub(LineEnding::Unix, false, false);
    }
    #[test]
    fn parse_sub_windows_last() {
        test_each_sub(LineEnding::Windows, false, false);
    }
    #[test]
    fn parse_sub_text_newline() {
        test_each_sub(LineEnding::Unix, true, false);
    }
    #[test]
    fn parse_sub_windows_text_newline() {
        test_each_sub(LineEnding::Windows, true, false);
    }

    #[test]
    fn parse_srt_unix() {
        test_srt(LineEnding::Unix, false, false)
    }
    #[test]
    fn parse_srt_windows() {
        test_srt(LineEnding::Windows, false, false)
    }
    #[test]
    fn parse_srt_unix_text_newline() {
        test_srt(LineEnding::Unix, true, false)
    }
    #[test]
    fn parse_srt_windows_text_newline() {
        test_srt(LineEnding::Windows, true, false)
    }
    #[test]
    fn parse_srt_unix_short() {
        test_srt(LineEnding::Unix, false, true)
    }
    #[test]
    fn parse_srt_windows_short() {
        test_srt(LineEnding::Windows, false, true)
    }
    #[test]
    fn parse_srt_unix_text_newline_short() {
        test_srt(LineEnding::Unix, true, true)
    }
    #[test]
    fn parse_srt_windows_text_newline_short() {
        test_srt(LineEnding::Windows, true, true)
    }
}
