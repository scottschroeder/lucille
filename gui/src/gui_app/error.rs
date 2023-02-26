use std::fmt;

pub struct ErrorChainLogLine {
    inner: anyhow::Error,
}

impl From<anyhow::Error> for ErrorChainLogLine {
    fn from(e: anyhow::Error) -> Self {
        ErrorChainLogLine { inner: e }
    }
}

impl fmt::Debug for ErrorChainLogLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;

        for ec in self.inner.chain() {
            if first {
                first = false;
            } else {
                write!(f, " -> ")?;
            }
            write!(f, "{}", ec)?
        }
        Ok(())
    }
}
