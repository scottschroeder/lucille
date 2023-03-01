use tokio::sync::oneshot;

type Tx<O> = oneshot::Sender<anyhow::Result<O>>;
type Rx<O> = oneshot::Receiver<anyhow::Result<O>>;

pub enum OneshotManager<I, O> {
    Request(Option<I>),
    Waiting(Option<Rx<O>>),
}

#[derive(Debug, PartialEq)]
pub enum OneshotState {
    Init,
    Request,
    Wait,
    Complete,
}

impl<I, O> Default for OneshotManager<I, O> {
    fn default() -> Self {
        Self::Request(None)
    }
}

impl<I, O> OneshotManager<I, O> {
    pub fn state(&self) -> OneshotState {
        match self {
            OneshotManager::Request(None) => OneshotState::Init,
            OneshotManager::Request(Some(_)) => OneshotState::Request,
            OneshotManager::Waiting(Some(_)) => OneshotState::Wait,
            OneshotManager::Waiting(None) => OneshotState::Complete,
        }
    }
    pub fn make_request(input: I) -> OneshotManager<I, O> {
        OneshotManager::Request(Some(input))
    }
    pub fn reset(&mut self) {
        let mut swp = Self::default();
        std::mem::swap(&mut swp, self)
    }
    pub fn set_request(&mut self, input: I) {
        let mut swp = Self::make_request(input);
        std::mem::swap(&mut swp, self)
    }
    pub fn send_request(&mut self, f: impl FnOnce(I, Tx<O>)) {
        if let OneshotManager::Request(opt) = self {
            if let Some(input) = opt.take() {
                let (tx, rx) = oneshot::channel();
                f(input, tx);
                *self = OneshotManager::Waiting(Some(rx));
            }
        }
    }
    pub fn get_response(&mut self) -> Option<anyhow::Result<O>> {
        if let OneshotManager::Waiting(opt) = self {
            if let Some(mut rx) = opt.take() {
                return match rx.try_recv() {
                    Ok(Ok(obj)) => Some(Ok(obj)),
                    Ok(Err(e)) => Some(Err(e)),
                    Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                        *opt = Some(rx);
                        None
                    }
                    Err(e) => Some(Err(
                        anyhow::Error::from(e).context("did not recieve a message from thread")
                    )),
                };
            }
        }

        None
    }
}
