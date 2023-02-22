use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

use tokio::process::Command;

#[derive(Debug)]
pub(crate) enum StdIo {
    Null,
    Piped,
    Inherit,
}

impl StdIo {
    fn into_exec(self) -> std::process::Stdio {
        match self {
            StdIo::Null => std::process::Stdio::null(),
            StdIo::Piped => std::process::Stdio::piped(),
            StdIo::Inherit => std::process::Stdio::inherit(),
        }
    }
}

struct TestArgFormat<'a>(&'a FFmpegArg);

impl<'a> std::fmt::Debug for TestArgFormat<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0.as_test())
    }
}

#[derive(Debug)]
pub(crate) enum FFmpegArg {
    Plain(OsString),
    Replaced(OsString, OsString),
}

impl FFmpegArg {
    pub(crate) fn plain<S: Into<OsString>>(s: S) -> FFmpegArg {
        FFmpegArg::Plain(s.into())
    }
    pub(crate) fn replaced<U: Into<OsString>, S: Into<OsString>>(u: U, s: S) -> FFmpegArg {
        FFmpegArg::Replaced(u.into(), s.into())
    }
    fn into_exec(self) -> OsString {
        match self {
            FFmpegArg::Plain(s) => s,
            FFmpegArg::Replaced(_, s) => s,
        }
    }
    fn as_test(&self) -> &OsStr {
        match self {
            FFmpegArg::Plain(s) => s.as_os_str(),
            FFmpegArg::Replaced(s, _) => s.as_os_str(),
        }
    }
}

impl Default for StdIo {
    fn default() -> Self {
        StdIo::Inherit
    }
}

#[derive(Debug, Clone, Default)]
pub struct FFMpegBinary {
    path: Option<PathBuf>,
}

impl From<Option<PathBuf>> for FFMpegBinary {
    fn from(path: Option<PathBuf>) -> Self {
        FFMpegBinary { path }
    }
}

impl FFMpegBinary {
    pub fn new<P: Into<PathBuf>>(p: P) -> FFMpegBinary {
        FFMpegBinary {
            path: Some(p.into()),
        }
    }
    fn executable_path(&self) -> &std::path::Path {
        if let Some(p) = &self.path {
            p.as_path()
        } else {
            std::path::Path::new("ffmpeg")
        }
    }
    pub(crate) fn build_command(self) -> FFmpegCommand {
        FFmpegCommand {
            bin: self,
            ..Default::default()
        }
    }
}

/// Low level interface over calling ffmpeg
#[derive(Debug, Default)]
pub(crate) struct FFmpegCommand {
    pub(crate) bin: FFMpegBinary,
    pub(crate) args: Vec<FFmpegArg>,
    pub(crate) cwd: Option<FFmpegArg>,
    pub(crate) stdin: Option<StdIo>,
    pub(crate) stdout: Option<StdIo>,
    debug: bool,
}

pub(crate) struct TestFormat<'a>(&'a FFmpegCommand);

impl<'a> std::fmt::Debug for TestFormat<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let args = self.0.args.iter().map(TestArgFormat).collect::<Vec<_>>();
        f.debug_struct("FFMpegTestFormat")
            .field("bin", &self.0.bin.executable_path())
            .field("args", &args)
            .field("cwd", &self.0.cwd.as_ref().map(TestArgFormat))
            .field("stdin", &self.0.stdin)
            .field("stdout", &self.0.stdout)
            .finish()
    }
}

impl FFmpegCommand {
    pub(crate) fn test_display(&self) -> TestFormat<'_> {
        TestFormat(self)
    }
    pub(crate) async fn spawn(self) -> Result<tokio::process::Child, std::io::Error> {
        log::trace!("spawn {:?}", &self);
        let mut st = Command::new(self.bin.executable_path());
        for arg in self.args {
            st.arg(arg.into_exec());
        }
        if let Some(cwd) = self.cwd {
            st.current_dir(cwd.into_exec());
        }

        if let Some(stdin) = self.stdin {
            st.stdin(stdin.into_exec());
        }
        if let Some(stdout) = self.stdout {
            st.stdout(stdout.into_exec());
        } else {
            st.stdout(StdIo::Null.into_exec());
        }

        if self.debug || cfg!(feature = "ffmpeg-debug") {
            st.stderr(StdIo::Inherit.into_exec());
        } else {
            st.stderr(StdIo::Null.into_exec());
        }

        st.spawn()
    }
}
