use std::cell::{Cell, RefCell, RefMut};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};

use super::ErrorReport;
use neovim_lib::{Neovim, NeovimApi};

#[derive(Clone, Copy, PartialEq)]
enum NeovimClientState {
    Uninitialized,
    InitInProgress,
    Initialized,
    Error,
}

pub enum NeovimRef<'a> {
    SingleThreaded(RefMut<'a, Neovim>),
    MultiThreaded(MutexGuard<'a, Option<Neovim>>),
}

impl<'a> NeovimRef<'a> {
    fn from_nvim(nvim: RefMut<'a, Neovim>) -> Self {
        NeovimRef::SingleThreaded(nvim)
    }

    fn try_nvim_async(nvim_async: &'a NeovimClientAsync) -> Option<NeovimRef<'a>> {
        let guard = nvim_async.nvim.try_lock();

        if let Ok(guard) = guard {
            if guard.is_some() {
                return Some(NeovimRef::MultiThreaded(guard));
            }
        }

        None
    }

    fn from_nvim_async(nvim_async: &'a NeovimClientAsync) -> Option<NeovimRef<'a>> {
        let guard = nvim_async.nvim.lock().unwrap();

        if guard.is_some() {
            Some(NeovimRef::MultiThreaded(guard))
        } else {
            None
        }
    }

    pub fn non_blocked(mut self) -> Option<Self> {
        self.get_mode().ok_and_report().and_then(|mode| {
            mode.iter()
                .find(|kv| kv.0.as_str().map(|key| key == "blocking").unwrap_or(false))
                .map(|kv| kv.1.as_bool().unwrap_or(false))
                .and_then(|block| if block { None } else { Some(self) })
        })
    }
}

impl<'a> Deref for NeovimRef<'a> {
    type Target = Neovim;

    fn deref(&self) -> &Neovim {
        match *self {
            NeovimRef::SingleThreaded(ref nvim) => &*nvim,
            NeovimRef::MultiThreaded(ref nvim) => (&*nvim).as_ref().unwrap(),
        }
    }
}

impl<'a> DerefMut for NeovimRef<'a> {
    fn deref_mut(&mut self) -> &mut Neovim {
        match *self {
            NeovimRef::SingleThreaded(ref mut nvim) => &mut *nvim,
            NeovimRef::MultiThreaded(ref mut nvim) => (&mut *nvim).as_mut().unwrap(),
        }
    }
}

pub struct NeovimClientAsync {
    nvim: Arc<Mutex<Option<Neovim>>>,
}

impl NeovimClientAsync {
    fn new() -> Self {
        NeovimClientAsync {
            nvim: Arc::new(Mutex::new(None)),
        }
    }

    pub fn borrow(&self) -> Option<NeovimRef> {
        NeovimRef::from_nvim_async(self)
    }

    pub fn try_borrow(&self) -> Option<NeovimRef> {
        NeovimRef::try_nvim_async(self)
    }
}

impl Clone for NeovimClientAsync {
    fn clone(&self) -> Self {
        NeovimClientAsync {
            nvim: self.nvim.clone(),
        }
    }
}

pub struct NeovimClient {
    state: Cell<NeovimClientState>,
    nvim: RefCell<Option<Neovim>>,
    nvim_async: NeovimClientAsync,
}

impl NeovimClient {
    pub fn new() -> Self {
        NeovimClient {
            state: Cell::new(NeovimClientState::Uninitialized),
            nvim: RefCell::new(None),
            nvim_async: NeovimClientAsync::new(),
        }
    }

    pub fn clear(&self) {
        let mut nvim = self.nvim.borrow_mut();
        if nvim.is_some() {
            nvim.take();
        } else {
            self.nvim_async.nvim.lock().unwrap().take();
        }
    }

    pub fn async_to_sync(&self) {
        let mut lock = self.nvim_async.nvim.lock().unwrap();
        let nvim = lock.take().unwrap();
        *self.nvim.borrow_mut() = Some(nvim);
    }

    pub fn set_nvim_async(&self, nvim: Neovim) -> NeovimClientAsync {
        *self.nvim_async.nvim.lock().unwrap() = Some(nvim);
        self.nvim_async.clone()
    }

    pub fn set_initialized(&self) {
        self.state.set(NeovimClientState::Initialized);
    }

    pub fn set_error(&self) {
        self.state.set(NeovimClientState::Error);
    }

    pub fn set_in_progress(&self) {
        self.state.set(NeovimClientState::InitInProgress);
    }

    pub fn is_initialized(&self) -> bool {
        self.state.get() == NeovimClientState::Initialized
    }

    pub fn is_uninitialized(&self) -> bool {
        self.state.get() == NeovimClientState::Uninitialized
    }

    pub fn is_initializing(&self) -> bool {
        self.state.get() == NeovimClientState::InitInProgress
    }

    /// In case neovimref locked in another thread
    /// this method can return None
    pub fn try_nvim(&self) -> Option<NeovimRef> {
        let nvim = self.nvim.borrow_mut();
        if nvim.is_some() {
            Some(NeovimRef::from_nvim(RefMut::map(nvim, |n| {
                n.as_mut().unwrap()
            })))
        } else {
            self.nvim_async.try_borrow()
        }
    }

    pub fn nvim(&self) -> Option<NeovimRef> {
        let nvim = self.nvim.borrow_mut();
        if nvim.is_some() {
            Some(NeovimRef::from_nvim(RefMut::map(nvim, |n| {
                n.as_mut().unwrap()
            })))
        } else {
            self.nvim_async.borrow()
        }
    }
}
