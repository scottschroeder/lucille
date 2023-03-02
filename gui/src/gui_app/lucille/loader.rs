pub enum LoadManager<T> {
    Init,
    Unloaded,
    Ready(T),
}

pub enum LoadResult<T> {
    Ready(T),
    NotReady(Option<anyhow::Error>),
}

impl<T> LoadResult<T> {
    pub fn take(self) -> anyhow::Result<Option<T>> {
        match self {
            LoadResult::Ready(t) => Ok(Some(t)),
            LoadResult::NotReady(e) => {
                if let Some(e) = e {
                    Err(e)
                } else {
                    Ok(None)
                }
            }
        }
    }
    pub fn err(&mut self) -> Option<anyhow::Error> {
        match self {
            LoadResult::Ready(_) => None,
            LoadResult::NotReady(e) => e.take(),
        }
    }
}

impl<T> Default for LoadManager<T> {
    fn default() -> Self {
        LoadManager::Init
    }
}

impl<T> LoadManager<T> {
    pub fn reset(&mut self) {
        *self = LoadManager::Init
    }

    pub fn unload(&mut self) {
        *self = LoadManager::Unloaded
    }

    /// Get the ready value. Panics if not ready.
    fn get_ready(&self) -> &T {
        match self {
            LoadManager::Ready(t) => t,
            _ => panic!("not ready"),
        }
    }
    // pub fn aquire<F>(&mut self, f: F) -> LoadResult<&T>
    // where
    //     F: FnOnce() -> anyhow::Result<T>,
    // {
    //     match self {
    //         LoadManager::Init => match f() {
    //             Ok(t) => {
    //                 *self = LoadManager::Ready(t);
    //                 LoadResult::Ready(self.get_ready())
    //             }
    //             Err(e) => {
    //                 *self = LoadManager::Unloaded;
    //                 LoadResult::NotReady(Some(e))
    //             }
    //         },
    //         LoadManager::Unloaded => LoadResult::NotReady(None),
    //         LoadManager::Ready(t) => LoadResult::Ready(t),
    //     }
    // }

    // pub fn take(&mut self) -> Option<T> {
    //     let mut swp = LoadManager::<T>::Unloaded;
    //     std::mem::swap(self, &mut swp);
    //     match swp {
    //         LoadManager::Init => None,
    //         LoadManager::Unloaded => None,
    //         LoadManager::Ready(t) => Some(t),
    //     }
    // }

    pub fn aquire_owned<F>(&mut self, f: F) -> LoadResult<T>
    where
        F: FnOnce() -> anyhow::Result<T>,
    {
        let mut swp = LoadManager::<T>::Unloaded;
        std::mem::swap(self, &mut swp);
        match swp {
            LoadManager::Init => match f() {
                Ok(t) => LoadResult::Ready(t),
                Err(e) => LoadResult::NotReady(Some(e)),
            },
            LoadManager::Unloaded => LoadResult::NotReady(None),
            LoadManager::Ready(t) => LoadResult::Ready(t),
        }
    }
}
