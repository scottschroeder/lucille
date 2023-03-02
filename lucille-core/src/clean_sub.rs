use std::fmt;

use subrip::Subtitle;

pub struct CleanSubs<'a>(pub &'a [Subtitle]);

impl<'a> fmt::Display for CleanSubs<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (idx, sub) in self.0.iter().enumerate() {
            if idx != 0 {
                f.write_str(" ")?;
            }
            write!(f, "{}", CleanSub(sub))?;
        }
        Ok(())
    }
}

pub struct CleanSub<'a>(pub &'a Subtitle);

impl<'a> fmt::Display for CleanSub<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (idx, line) in self.0.text.lines().enumerate() {
            if idx != 0 {
                f.write_str(" ")?;
            }
            let text = line.trim().trim_start_matches('-').trim();
            f.write_str(text)?;
        }
        Ok(())
    }
}
