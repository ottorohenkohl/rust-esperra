use core::sync::atomic::{AtomicUsize, Ordering};

const UNLOCKED: usize = 0;

pub struct MailboxLock {
    owner: AtomicUsize,
}

impl MailboxLock {
    pub const fn new() -> Self {
        Self { owner: AtomicUsize::new(UNLOCKED) }
    }

    pub fn allows(&self, caller: Option<usize>) -> bool {
        let owner = self.owner.load(Ordering::SeqCst);

        owner == UNLOCKED || caller == Some(owner)
    }

    pub fn lock(&self, owner: usize) -> bool {
        match self.owner.compare_exchange(UNLOCKED, owner, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => true,
            Err(current_owner) => current_owner == owner,
        }
    }

    pub fn unlock(&self, owner: usize) -> bool {
        self.owner.compare_exchange(owner, UNLOCKED, Ordering::SeqCst, Ordering::SeqCst).is_ok()
    }
}
